use std::collections::HashMap;
use std::ffi::CString;
use std::path::{Path, PathBuf};

use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::{execv, Pid};
use proc_maps::MapRange;
use tracing::{debug, error, info, warn};

use crate::breakpoint::Breakpoint;
use crate::dbginfo::CMDebugInfo;
use crate::disassemble::Disassembly;
use crate::errors::{DebuggerError, Result};
use crate::feedback::Feedback;
use crate::ui::{DebuggerUI, Register, Status};
use crate::{mem_read, mem_read_word, mem_write_word, Addr, Word};

pub struct Debugger<'executable, UI: DebuggerUI> {
    executable_path: PathBuf,
    debuggee: Option<Debuggee<'executable>>,
    ui: UI,
}

pub struct Debuggee<'executable> {
    pid: Pid,
    breakpoints: HashMap<Addr, Breakpoint>,
    dbginfo: CMDebugInfo<'executable>,
}

impl Debuggee<'_> {
    pub fn kill(&self) -> Result<()> {
        ptrace::kill(self.pid)?;
        Ok(())
    }

    #[inline]
    pub fn get_process_map(&self) -> Result<Vec<MapRange>> {
        Ok(proc_maps::get_process_maps(self.pid.into())?)
    }

    pub fn get_base_addr(&self) -> Result<Addr> {
        Ok(self.get_process_map()?[0].start().into())
    }

    pub fn disassemble(&self, addr: Addr, len: usize) -> Result<Disassembly> {
        let mut data_raw: Vec<u8> = vec![0; len];
        mem_read(&mut data_raw, self.pid, addr)?;
        let out: Disassembly = Disassembly::disassemble(&data_raw, addr)?;
        Ok(out)
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
                    self.debuggee = Some(Debuggee {
                        pid,
                        dbginfo,
                        breakpoints: HashMap::new(),
                    });
                    Ok(())
                }
                nix::unistd::ForkResult::Child => {
                    info!("starting the debuggee process");
                    let cpath = CString::new(path.to_string_lossy().to_string().as_str())?;
                    eprintln!("DOING THE TRACEME");
                    ptrace::traceme()
                        .inspect_err(|e| eprintln!("error while doing traceme: {e}"))?;
                    eprintln!("DOING THE EXECV");
                    execv(&cpath, args)?; // NOTE: unsure if args[0] is set to the executable
                    unreachable!()
                }
            },
        }
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

    pub fn run_debugger(&mut self) -> Result<()> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();
        self.wait(&[])?; // wait until the debuggee is stopped

        info!("PID: {}", dbge.pid);
        info!("base addr: {}", dbge.get_base_addr()?);

        let mut feedback: Feedback = Feedback::Ok;
        loop {
            let ui_res = self.ui.process(&feedback);
            feedback = {
                match ui_res {
                    Err(e) => {
                        error!("{e}");
                        return Err(e);
                    }
                    Ok(s) => match s {
                        Status::DebuggerQuit => break,
                        Status::Continue => self.cont(None),
                        Status::SetBreakpoint(addr) => self.set_bp(addr),
                        Status::DelBreakpoint(addr) => self.del_bp(addr),
                        Status::DumpRegisters => self.dump_regs(),
                        Status::SetRegister(r, v) => self.set_reg(r, v),
                        Status::WriteMem(a, v) => self.write_mem(a, v),
                        Status::ReadMem(a) => self.read_mem(a),
                        Status::DisassembleAt(a, l) => self.disassemble_at(a, l),
                    },
                }
            }
            .into();
        }

        Ok(())
    }

    pub fn cont(&mut self, sig: Option<Signal>) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        self.step_over_bp()?;
        ptrace::cont(self.debuggee.as_ref().unwrap().pid, sig)?;

        self.wait(&[])?; // wait until the debuggee is stopped again!!!
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
        debug!("{:#x?}", dbge.breakpoints);

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

    pub fn single_step(&self) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        // FIXME: this is probably noticeable
        ptrace::step(dbge.pid, None)?;

        Ok(Feedback::Ok)
    }

    pub fn step_over_bp(&mut self) -> Result<()> {
        // This function is hell with the borrow checker.
        // You can only have a single mutable refence OR n immutable references
        // Thus, you cannot simply `let bp = ...` at the start and later use things like
        // `self.single_step`
        self.err_if_no_debuggee()?;
        let maybe_bp_addr: Addr = (self.get_reg(Register::rip)? - 1).into();

        if self
            .debuggee
            .as_mut()
            .unwrap()
            .breakpoints
            .get_mut(&maybe_bp_addr)
            .is_some_and(|a| a.is_enabled())
        {
            let here_is_the_bp = maybe_bp_addr;
            self.set_reg(Register::rip, here_is_the_bp.into())?;
            self.debuggee
                .as_mut()
                .unwrap()
                .breakpoints
                .get_mut(&maybe_bp_addr)
                .unwrap()
                .disable()?;

            match self.single_step()? {
                Feedback::Ok => (),
                _ => panic!("single step returned a feedback other than Ok"),
            }
            self.wait(&[])?; // wait for it to stop again
            self.debuggee
                .as_mut()
                .unwrap()
                .breakpoints
                .get_mut(&maybe_bp_addr)
                .unwrap()
                .enable()?;
        }

        Ok(())
    }

    pub fn disassemble_at(&self, addr: Addr, len: usize) -> Result<Feedback> {
        self.err_if_no_debuggee()?;
        let dbge = self.debuggee.as_ref().unwrap();

        let t = dbge.disassemble(addr, len)?;

        Ok(Feedback::Disassembly(t))
    }
}
