use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use gimli::{DW_AT_high_pc, DW_AT_low_pc, DW_AT_name, Unit};
use iced_x86::FormatterTextKind;
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{execv, Pid};
use proc_maps::MapRange;
use tracing::{debug, error, info, trace, warn};

use crate::breakpoint::Breakpoint;
use crate::consts::{SI_KERNEL, TRAP_BRKPT, TRAP_TRACE};
use crate::dbginfo::{CMDebugInfo, OwnedSymbol, SymbolKind};
use crate::disassemble::Disassembly;
use crate::dwarf_parse::GimliReaderThing;
use crate::errors::{DebuggerError, Result};
use crate::feedback::Feedback;
use crate::ui::{DebuggerUI, Register, Status};
use crate::{mem_read, mem_read_word, mem_write_word, unwind, Addr, Word};

pub type VariableExpression = String;

pub struct Debugger<'executable, UI: DebuggerUI> {
    pub(crate) executable_path: PathBuf,
    pub(crate) debuggee: Option<Debuggee<'executable>>,
    pub(crate) ui: UI,
}

pub struct Debuggee<'executable> {
    pub(crate) pid: Pid,
    pub(crate) breakpoints: HashMap<Addr, Breakpoint>,
    #[allow(dead_code)] // this is important stuff
    pub(crate) dbginfo: CMDebugInfo<'executable>,
    pub(crate) symbols: Vec<OwnedSymbol>,
}

impl<'executable> Debuggee<'executable> {
    pub(crate) fn build(
        pid: Pid,
        dbginfo: CMDebugInfo<'executable>,
        breakpoints: HashMap<Addr, Breakpoint>,
    ) -> Result<Self> {
        let mut symbols = Vec::new();
        let dwarf = &dbginfo.dwarf;
        let mut iter = dwarf.units();

        while let Some(header) = iter.next()? {
            let unit = dwarf.unit(header)?;
            let mut tree = unit.entries_tree(None)?;
            symbols.push(Self::process_tree(pid, dwarf, &unit, tree.root()?)?);
        }

        Ok(Self {
            pid,
            breakpoints,
            dbginfo,
            symbols,
        })
    }

    pub fn kill(&self) -> Result<()> {
        ptrace::kill(self.pid)?;
        Ok(())
    }

    fn get_process_map_by_pid(pid: Pid) -> Result<Vec<MapRange>> {
        Ok(proc_maps::get_process_maps(pid.into())?)
    }

    pub fn get_base_addr_by_pid(pid: Pid) -> Result<Addr> {
        Ok(Self::get_process_map_by_pid(pid)?[0].start().into())
    }

    #[inline]
    pub fn get_process_map(&self) -> Result<Vec<MapRange>> {
        Self::get_process_map_by_pid(self.pid)
    }

    pub fn get_base_addr(&self) -> Result<Addr> {
        Self::get_base_addr_by_pid(self.pid)
    }

    pub fn disassemble(&self, addr: Addr, len: usize) -> Result<Disassembly> {
        let mut data_raw: Vec<u8> = vec![0; len];
        mem_read(&mut data_raw, self.pid, addr)?;
        let out: Disassembly = Disassembly::disassemble(&data_raw, addr)?;
        Ok(out)
    }

    fn entry_from_gimli(
        pid: Pid,
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        entry: &gimli::DebuggingInformationEntry<'_, '_, GimliReaderThing>,
    ) -> Result<OwnedSymbol> {
        let base_addr = Self::get_base_addr_by_pid(pid)?;

        #[allow(clippy::single_match)]
        match entry.tag() {
            gimli::DW_TAG_subprogram => {
                let low = Self::parse_addr_low(dwarf, unit, entry.attr(DW_AT_low_pc)?, base_addr)?;
                let high = Self::parse_addr_high(entry.attr(DW_AT_high_pc)?, low)?;
                let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
                let kind = SymbolKind::try_from(entry.tag())?;
                Ok(OwnedSymbol::new(name, low, high, kind, &[]))
            }
            gimli::DW_TAG_compile_unit => {
                // Example values
                // DW_AT_producer              GNU C17 14.2.0 -mtune=generic -march=x86-64 -g -fasynchronous-unwind-tables
                // DW_AT_language              DW_LANG_C11
                // DW_AT_name                  ./examples/dummy.c
                // DW_AT_comp_dir              /home/plex/Dokumente/code/rs/coreminer
                // DW_AT_low_pc                0x00001139
                // DW_AT_high_pc               <offset-from-lowpc> 83 <highpc: 0x0000118c>
                // DW_AT_stmt_list             0x00000000
                let low = Self::parse_addr_low(dwarf, unit, entry.attr(DW_AT_low_pc)?, base_addr)?;
                let high = Self::parse_addr_high(entry.attr(DW_AT_high_pc)?, low)?;
                let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
                let kind = SymbolKind::try_from(entry.tag())?;
                Ok(OwnedSymbol::new(name, low, high, kind, &[]))
            }
            gimli::DW_TAG_base_type => {
                let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
                let kind = SymbolKind::try_from(entry.tag())?;
                Ok(OwnedSymbol::new(name, None, None, kind, &[]))
            }
            // gimli::DW_TAG_constant => {
            //     todo!()
            // }
            gimli::DW_TAG_variable => {
                let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
                let kind = SymbolKind::try_from(entry.tag())?;
                Ok(OwnedSymbol::new(name, None, None, kind, &[]))
            }
            _ => {
                debug!("unknown tag type, parsing as Other: {}", entry.tag());
                let name = Self::parse_string(dwarf, unit, entry.attr(DW_AT_name)?)?;
                let kind = SymbolKind::try_from(entry.tag())?;
                Ok(OwnedSymbol::new(name, None, None, kind, &[]))
            }
        }
    }

    // RETURNS ALL SYMBOLS!
    //
    // those symbols have references to their children
    fn process_tree(
        pid: Pid,
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        node: gimli::EntriesTreeNode<GimliReaderThing>,
    ) -> Result<OwnedSymbol> {
        let mut children: Vec<OwnedSymbol> = Vec::new();
        let mut parent = Self::entry_from_gimli(pid, dwarf, unit, node.entry())?;

        // then process it's children
        let mut children_tree = node.children();
        while let Some(child) = children_tree.next()? {
            // Recursively process a child.
            children.push(match Self::process_tree(pid, dwarf, unit, child) {
                Err(e) => {
                    debug!("could not parse a leaf of the debug symbol tree: {e}");
                    continue;
                }
                Ok(s) => s,
            });
        }

        parent.children = children;
        Ok(parent)
    }

    pub fn get_symbol_by_name(&self, name: impl Display) -> Result<Vec<OwnedSymbol>> {
        let all: Vec<OwnedSymbol> = self
            .symbols_query(|a| a.name() == Some(&name.to_string()))
            .to_vec();

        Ok(all)
    }

    pub fn get_function_by_addr(&self, addr: Addr) -> Result<Option<OwnedSymbol>> {
        debug!("get function for addr {addr}");
        for sym in self
            .symbols_query(|a| a.kind() == SymbolKind::Function)
            .iter()
            .cloned()
        {
            if sym.low_addr.is_some_and(|a| a <= addr) && sym.high_addr.is_some_and(|a| addr < a) {
                return Ok(Some(sym));
            }
        }

        Ok(None)
    }

    pub fn get_local_variables(&self, addr: Addr) -> Result<Vec<OwnedSymbol>> {
        debug!("get locals of function {addr}");
        for sym in self.symbols_query(|a| a.kind() == SymbolKind::Function) {
            if sym.low_addr.is_some_and(|a| a <= addr) && sym.high_addr.is_some_and(|a| addr < a) {
                return Ok(sym.children().to_vec());
            } else {
                trace!("it's not {:#?}", sym);
            }
        }

        Ok(Vec::new())
    }

    pub fn symbols(&self) -> &[OwnedSymbol] {
        &self.symbols
    }

    /// like [Self::symbols] but includes all children somehow
    pub fn symbols_query<F>(&self, fil: F) -> Vec<OwnedSymbol>
    where
        F: Fn(&OwnedSymbol) -> bool,
    {
        let mut relevant = Vec::new();

        fn finder<F>(buf: &mut Vec<OwnedSymbol>, s: &OwnedSymbol, fil: &F)
        where
            F: Fn(&OwnedSymbol) -> bool,
        {
            for c in s.children() {
                finder(buf, c, fil);
            }
            if fil(s) {
                buf.push(s.clone());
            }
        }

        for s in self.symbols() {
            finder(&mut relevant, s, &fil);
        }

        relevant
    }
}

impl<'executable, UI: DebuggerUI> Debugger<'executable, UI> {
    pub fn build(executable_path: impl AsRef<Path>, ui: UI) -> Result<Self> {
        Ok(Debugger {
            debuggee: None,
            ui,
            executable_path: executable_path.as_ref().to_owned(),
        })
    }

    pub fn launch_debuggee(
        &mut self,
        args: &[CString],
        executable_obj_data: object::File<'executable>,
    ) -> Result<()> {
        let path: &Path = self.executable_path.as_ref();
        if !path.exists() {
            let err = DebuggerError::ExecutableDoesNotExist(path.to_string_lossy().to_string());
            error!("{err}");
            return Err(err);
        }
        if !path.is_file() {
            let err = DebuggerError::ExecutableIsNotAFile(path.to_string_lossy().to_string());
            error!("{err}");
            return Err(err);
        }

        let dbginfo: CMDebugInfo = CMDebugInfo::build(executable_obj_data)?;

        let fork_res = unsafe { nix::unistd::fork() };
        match fork_res {
            Err(e) => {
                error!("could not start executable: {e}");
                Err(e.into())
            }
            Ok(fr) => match fr {
                nix::unistd::ForkResult::Parent { child: pid } => {
                    let dbge = Debuggee::build(pid, dbginfo, HashMap::new())?;
                    self.debuggee = Some(dbge);
                    Ok(())
                }
                nix::unistd::ForkResult::Child => {
                    info!("starting the debuggee process");
                    let cpath = CString::new(path.to_string_lossy().to_string().as_str())?;
                    ptrace::traceme()
                        .inspect_err(|e| eprintln!("error while doing traceme: {e}"))?;
                    execv(&cpath, args)?; // NOTE: unsure if args[0] is set to the executable
                    unreachable!()
                }
            },
        }
    }

    pub fn wait_signal(&self) -> Result<nix::libc::siginfo_t> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let mut _status;
        let mut siginfo;
        let mut sig;

        loop {
            _status = self.wait(&[])?;
            siginfo = ptrace::getsiginfo(dbge.pid)?;
            sig = Signal::try_from(siginfo.si_signo)?;
            match sig {
                Signal::SIGTRAP => {
                    self.handle_sigtrap(sig, siginfo)?;
                    break;
                }
                Signal::SIGSEGV
                | Signal::SIGINT
                | Signal::SIGPIPE
                | Signal::SIGSTOP
                | Signal::SIGILL => {
                    self.handle_important_signal(sig, siginfo)?;
                    break;
                }
                _ => {
                    trace!("ignoring signal {sig}");
                    continue;
                }
            }
        }

        Ok(siginfo)
    }

    pub fn wait(&self, options: &[WaitPidFlag]) -> Result<WaitStatus> {
        self.err_if_no_debuggee()?;
        let mut flags = WaitPidFlag::empty();
        for f in options {
            flags |= *f;
        }
        Ok(waitpid(
            self.debuggee.as_ref().unwrap().pid,
            if flags.is_empty() { None } else { Some(flags) },
        )?)
    }

    pub fn parse_exec_data(
        &mut self,
        data: &'executable [u8],
    ) -> Result<object::File<'executable>> {
        use object::File;
        let file = File::parse(data)?;
        Ok(file)
    }

    pub fn run_debugger(&mut self) -> Result<()> {
        self.wait(&[])?; // wait until the debuggee is stopped

        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        let fun = dbge.get_function_by_addr(Addr::from_relative(dbge.get_base_addr()?, 0x1140))?;
        debug!("function at 0x1140: {fun:#?}");
        let root_syms = dbge.symbols();
        debug!("root symbols:\n{root_syms:#?}");

        info!("PID: {}", dbge.pid);
        info!("base addr: {}", dbge.get_base_addr()?);

        let mut feedback: Feedback = Feedback::Ok;
        loop {
            let ui_res = self.ui.process(feedback);
            feedback = {
                match ui_res {
                    Err(e) => {
                        error!("{e}");
                        return Err(e);
                    }
                    Ok(s) => match s {
                        Status::Infos => self.infos(),
                        Status::DebuggerQuit => break,
                        Status::Continue => self.cont(None),
                        Status::SetBreakpoint(addr) => self.set_bp(addr),
                        Status::DelBreakpoint(addr) => self.del_bp(addr),
                        Status::DumpRegisters => self.dump_regs(),
                        Status::SetRegister(r, v) => self.set_reg(r, v),
                        Status::WriteMem(a, v) => self.write_mem(a, v),
                        Status::ReadMem(a) => self.read_mem(a),
                        Status::DisassembleAt(a, l) => self.disassemble_at(a, l),
                        Status::GetSymbolsByName(s) => self.get_symbol_by_name(s),
                        Status::StepSingle => self.single_step(),
                        Status::StepOut => self.step_out(),
                        Status::StepInto => self.step_into(),
                        Status::StepOver => self.step_over(),
                        Status::Backtrace => self.backtrace(),
                    },
                }
            }
            .into();
        }

        Ok(())
    }

    pub fn cont(&mut self, sig: Option<Signal>) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        self.go_back_step_over_bp()?;
        ptrace::cont(self.debuggee.as_ref().unwrap().pid, sig)?;

        self.wait_signal()?; // wait until the debuggee is stopped again!!!
        Ok(Feedback::Ok)
    }

    pub fn dump_regs(&self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        let regs = ptrace::getregs(dbge.pid)?;
        Ok(Feedback::Registers(regs))
    }

    fn err_if_no_debuggee(&self) -> Result<()> {
        if self.debuggee.is_none() {
            let err = DebuggerError::NoDebugee;
            error!("{err}");
            Err(err)
        } else {
            Ok(())
        }
    }

    pub fn cleanup(&self) -> Result<()> {
        if let Some(dbge) = &self.debuggee {
            dbge.kill()?;
        }
        Ok(())
    }

    pub fn set_bp(&mut self, addr: Addr) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_mut().unwrap();

        let mut bp = Breakpoint::new(dbge.pid, addr);
        bp.enable()?;
        dbge.breakpoints.insert(addr, bp);

        Ok(Feedback::Ok)
    }

    pub fn del_bp(&mut self, addr: Addr) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_mut().unwrap();

        if let Some(_bp) = dbge.breakpoints.get_mut(&addr) {
            dbge.breakpoints.remove(&addr); // gets disabled on dropping
        } else {
            warn!("removed a breakpoint at {addr:x?} that did not exist");
        }

        Ok(Feedback::Ok)
    }

    pub fn get_reg(&self, r: Register) -> Result<u64> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        let regs = ptrace::getregs(dbge.pid)?;

        let v = match r {
            Register::r9 => regs.r9,
            Register::r8 => regs.r8,
            Register::r10 => regs.r10,
            Register::r11 => regs.r11,
            Register::r12 => regs.r12,
            Register::r13 => regs.r13,
            Register::r14 => regs.r14,
            Register::r15 => regs.r15,
            Register::rip => regs.rip,
            Register::rbp => regs.rbp,
            Register::rax => regs.rax,
            Register::rcx => regs.rcx,
            Register::rbx => regs.rbx,
            Register::rdx => regs.rdx,
            Register::rsi => regs.rsi,
            Register::rsp => regs.rsp,
            Register::rdi => regs.rdi,
            Register::orig_rax => regs.orig_rax,
            Register::eflags => regs.eflags,
            Register::es => regs.es,
            Register::cs => regs.cs,
            Register::ss => regs.ss,
            Register::fs_base => regs.fs_base,
            Register::fs => regs.fs,
            Register::gs_base => regs.gs_base,
            Register::gs => regs.gs,
            Register::ds => regs.ds,
        };

        Ok(v)
    }

    pub fn set_reg(&self, r: Register, v: u64) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        let mut regs = ptrace::getregs(dbge.pid)?;

        match r {
            Register::r9 => regs.r9 = v,
            Register::r8 => regs.r8 = v,
            Register::r10 => regs.r10 = v,
            Register::r11 => regs.r11 = v,
            Register::r12 => regs.r12 = v,
            Register::r13 => regs.r13 = v,
            Register::r14 => regs.r14 = v,
            Register::r15 => regs.r15 = v,
            Register::rip => regs.rip = v,
            Register::rbp => regs.rbp = v,
            Register::rax => regs.rax = v,
            Register::rcx => regs.rcx = v,
            Register::rbx => regs.rbx = v,
            Register::rdx => regs.rdx = v,
            Register::rsi => regs.rsi = v,
            Register::rsp => regs.rsp = v,
            Register::rdi => regs.rdi = v,
            Register::orig_rax => regs.orig_rax = v,
            Register::eflags => regs.eflags = v,
            Register::es => regs.es = v,
            Register::cs => regs.cs = v,
            Register::ss => regs.ss = v,
            Register::fs_base => regs.fs_base = v,
            Register::fs => regs.fs = v,
            Register::gs_base => regs.gs_base = v,
            Register::gs => regs.gs = v,
            Register::ds => regs.ds = v,
        }

        ptrace::setregs(dbge.pid, regs)?;

        Ok(Feedback::Ok)
    }

    pub fn read_mem(&self, addr: Addr) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let w = mem_read_word(dbge.pid, addr)?;

        Ok(Feedback::Word(w))
    }

    pub fn write_mem(&self, addr: Addr, value: Word) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        mem_write_word(dbge.pid, addr, value)?;

        Ok(Feedback::Ok)
    }

    fn atomic_single_step(&self) -> Result<()> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        // FIXME: this is probably noticeable
        if let Err(e) = ptrace::step(dbge.pid, None) {
            error!("could not do atomic step: {e}");
            return Err(e.into());
        }

        Ok(())
    }

    pub fn single_step(&mut self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        if self.go_back_step_over_bp()? {
            info!("breakpoint before, caught up and continueing with single step")
        }
        let dbge = self.debuggee.as_ref().unwrap();

        let maybe_bp_addr: Addr = (self.get_reg(Register::rip)?).into();
        if dbge.breakpoints.contains_key(&maybe_bp_addr) {
            trace!("step over instruction with breakpoint");
            self.dse(maybe_bp_addr)?;
        } else {
            trace!("step regular instruction");
            self.atomic_single_step()?;
            self.wait_signal()?;
        }
        trace!("now at {:018x}", self.get_reg(Register::rip)?);

        Ok(Feedback::Ok)
    }

    pub fn step_out(&mut self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        {
            let a = self
                .debuggee
                .as_ref()
                .unwrap()
                .get_function_by_addr(self.get_reg(Register::rip)?.into())?;
            if let Some(s) = a {
                debug!("step out in following function: {s:#?}");
                if s.name() == Some("main") {
                    error!("you're about to do something stupid: no stepping out of the earliest stack frame allowed");
                    return Err(DebuggerError::StepOutMain);
                }
            } else {
                warn!("did not find debug symbol for current address");
            }
        }

        let stack_frame_pointer: Addr = self.get_reg(Register::rbp)?.into();
        let return_addr: Addr =
            mem_read_word(self.debuggee.as_ref().unwrap().pid, stack_frame_pointer + 8)?.into();
        trace!("rsb: {stack_frame_pointer}");
        trace!("ret_addr: {return_addr}");

        let should_remove_breakpoint = if !self
            .debuggee
            .as_ref()
            .unwrap()
            .breakpoints
            .contains_key(&return_addr)
        {
            self.set_bp(return_addr)?;
            true
        } else {
            false
        };

        self.cont(None)?;

        if should_remove_breakpoint {
            self.del_bp(return_addr)?;
            self.set_reg(Register::rip, self.get_reg(Register::rip)? - 1)?; // we need to go back
                                                                            // else we skip an instruction
        }
        Ok(Feedback::Ok)
    }

    fn dse(&mut self, here: Addr) -> Result<()> {
        trace!("disabling the breakpoint");
        self.debuggee
            .as_mut()
            .unwrap()
            .breakpoints
            .get_mut(&here)
            .unwrap()
            .disable()?;

        trace!("atomic step");
        self.atomic_single_step()?;
        trace!("waiting");
        self.wait_signal()
            .inspect_err(|e| warn!("weird wait_signal error: {e}"))?;
        trace!("enable stepped over bp again");
        self.debuggee
            .as_mut()
            .unwrap()
            .breakpoints
            .get_mut(&here)
            .unwrap()
            .enable()?;
        trace!("dse done");

        Ok(())
    }

    pub fn go_back_step_over_bp(&mut self) -> Result<bool> {
        // This function is hell with the borrow checker.
        // You can only have a single mutable refence OR n immutable references
        // Thus, you cannot simply `let bp = ...` at the start and later use things like
        // `self.atomic_single_step`
        self.err_if_no_debuggee()?;
        let maybe_bp_addr: Addr = (self.get_reg(Register::rip)? - 1).into();
        trace!("Checkinf if {maybe_bp_addr} had a breakpoint");

        if self
            .debuggee
            .as_mut()
            .unwrap()
            .breakpoints
            .get_mut(&maybe_bp_addr)
            .is_some_and(|a| a.is_enabled())
        {
            let here = maybe_bp_addr;
            trace!("set register to {here}");
            self.set_reg(Register::rip, here.into())?;

            self.dse(here)?;
            Ok(true)
        } else {
            trace!("breakpoint is disabled or does not exist, doing nothing");
            Ok(false)
        }
    }

    pub fn disassemble_at(&self, addr: Addr, len: usize) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let t = dbge.disassemble(addr, len)?;

        Ok(Feedback::Disassembly(t))
    }

    pub fn get_symbol_by_name(&self, name: impl Display) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let symbols: Vec<OwnedSymbol> = dbge.get_symbol_by_name(name)?;
        Ok(Feedback::Symbols(symbols))
    }

    pub fn handle_sigtrap(
        &self,
        sig: nix::sys::signal::Signal,
        siginfo: nix::libc::siginfo_t,
    ) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);

        match siginfo.si_code {
            SI_KERNEL => trace!("SI_KERNEL"), // we don't know what do do?
            TRAP_BRKPT => {
                trace!("TRAP_BRKPT")
            }
            TRAP_TRACE => trace!("TRAP_TRACE"), // single stepping
            _ => warn!("Strange SIGTRAP code: {}", siginfo.si_code),
        }
        Ok(())
    }

    pub fn handle_important_signal(
        &self,
        sig: Signal,
        siginfo: nix::libc::siginfo_t,
    ) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);
        Ok(())
    }

    pub fn handle_other_signal(&self, sig: Signal, siginfo: nix::libc::siginfo_t) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);
        Ok(())
    }

    fn infos(&self) -> std::result::Result<Feedback, DebuggerError> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        info!("Breakpoints:\n{:#?}", dbge.breakpoints);
        Ok(Feedback::Ok)
    }

    pub fn step_into(&mut self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        self.go_back_step_over_bp()?;

        loop {
            let rip: Addr = (self.get_reg(Register::rip)?).into();
            let disassembly: Disassembly = self.debuggee.as_ref().unwrap().disassemble(rip, 8)?;
            let next_instruction = &disassembly.inner()[0];
            let operator = next_instruction.2[0].clone();

            if operator.1 != FormatterTextKind::Mnemonic {
                error!("could not read operator from disassembly");
            }
            if operator.0.trim() == "call" {
                self.single_step()?;

                let rip: Addr = (self.get_reg(Register::rip)?).into();
                let disassembly: Disassembly =
                    self.debuggee.as_ref().unwrap().disassemble(rip, 8)?;

                let mut normal_prolog = true;
                // NOTE: the magic numbers are the machine code for the normal prologue
                // 55                      push            rbp
                // 48 89 e5                mov             rbp,rsp
                // 48 83 ec 10             sub             rsp,10 ; 10 is flexible (stack size)
                if disassembly.inner().len() != 3 {
                    normal_prolog = false;
                }
                if normal_prolog && disassembly.inner()[0].1 != [0x55] {
                    normal_prolog = false;
                }
                if normal_prolog && disassembly.inner()[1].1 != [0x48, 0x89, 0xe5] {
                    normal_prolog = false;
                }
                if normal_prolog && disassembly.inner()[2].1.starts_with(&[0x48, 0x89, 0xec]) {
                    normal_prolog = false;
                }

                if normal_prolog {
                    self.single_step()?;
                    self.single_step()?;
                    self.single_step()?;
                } else {
                    warn!("weird prolog, not stepping to the end of the prolog:\n{disassembly}")
                }

                break;
            } else {
                self.single_step()?; // PERF: this is very inefficient :/ maybe remove the autostepper
            }
        }

        Ok(Feedback::Ok)
    }

    pub fn step_over(&mut self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        self.go_back_step_over_bp()?;

        self.step_into()?;
        self.step_out()
    }

    pub fn backtrace(&self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let backtrace = unwind::unwind(dbge.pid)?;

        Ok(Feedback::Backtrace(backtrace))
    }
}
