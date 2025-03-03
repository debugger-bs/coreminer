//! # Debugger Module
//!
//! This module provides the core debugging functionality, coordinating between the user interface and the debugged process.
//!
//! The debugger is the central orchestrator in coreminer that:
//!
//! 1. **Controls process execution** - Manages starting, stopping, and stepping through the target program
//! 2. **Handles breakpoints** - Sets, removes, and manages hitting breakpoints during execution
//! 3. **Provides memory access** - Reads and writes to the target's memory space
//! 4. **Exposes registers** - Allows inspection and modification of CPU registers
//! 5. **Interprets debug info** - Uses DWARF debug information to resolve symbols, variables, and source locations
//! 6. **Manages signals** - Intercepts and processes signals from the target process
//! 7. **Handles user interaction** - Communicates with the UI to present information and receive commands
//!
//! ## Architecture
//!
//! The debugger uses a combination of:
//!
//! - **ptrace** - For low-level process control and memory access
//! - **DWARF debug information** - To understand program structure and variables
//! - **waitpid** - To synchronize with the debugged process's state changes
//! - **signal handling** - To interpret and respond to exceptions in the target
//! - **[Debuggee]** - Various methods of the [Debuggee] struct.
//!

use std::collections::HashMap;
use std::ffi::CString;
use std::fmt::Display;
use std::path::Path;

use iced_x86::FormatterTextKind;
use nix::sys::ptrace;
use nix::sys::signal::Signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::execv;
use tracing::{debug, error, info, trace, warn};

use crate::breakpoint::Breakpoint;
use crate::consts::{SI_KERNEL, TRAP_BRKPT, TRAP_TRACE};
use crate::dbginfo::{CMDebugInfo, OwnedSymbol};
use crate::debuggee::Debuggee;
use crate::disassemble::Disassembly;
use crate::dwarf_parse::FrameInfo;
use crate::errors::{DebuggerError, Result};
use crate::feedback::Feedback;
use crate::ui::{DebuggerUI, Status};
use crate::variable::{VariableExpression, VariableValue};
use crate::{mem_read_word, mem_write_word, unwind, Addr, Register, Word};

/// Manages the debugging session and coordinates between the UI and debuggee
///
/// The [`Debugger`] struct is the central component that ties together the user interface and
/// the debugged process. It holds state for the current debugging session, processes commands
/// from the UI, and controls the execution of the debuggee.
///
/// # Type Parameters
///
/// * `'executable` - Lifetime of the executable data
/// * `UI` - The user interface type, which must implement [`DebuggerUI`]
///
/// ## Examples
///
/// Basic usage of the debugger:
///
/// ```no_run
/// use coreminer::debugger::Debugger;
/// use coreminer::ui::cli::CliUi;
/// use coreminer::errors::Result;
/// use std::path::Path;
/// use std::ffi::CString;
///
/// fn main() -> Result<()> {
///     // Create a UI implementation, this may be antthing implementing the DebuggerUI trait
///     let ui = CliUi::build(None)?;
///
///     // Build a debugger with the UI
///     let mut debugger = Debugger::build(ui)?;
///
///     // Run the main interactive debugging loop
///     debugger.run_debugger()?;
///
///     // Clean up resources
///     debugger.cleanup()?;
///
///     Ok(())
/// }
/// ```
///
/// Theoretically, automated usage of the debugger functions is also possible:
///
/// ```no_run
/// use coreminer::debugger::Debugger;
/// use coreminer::ui::cli::CliUi;
/// use coreminer::errors::Result;
/// use coreminer::feedback::Feedback;
/// use std::path::Path;
/// use std::ffi::CString;
///
/// fn main() -> Result<()> {
///     // Create a UI implementation, this may be antthing implementing the DebuggerUI trait
///     // for automated, this is not really needed.
///     let ui = CliUi::build(None)?;
///
///     // Build a debugger with the UI
///     let mut debugger = Debugger::build(ui)?;
///
///     // Launch a program for debugging
///     let program_path = Path::new("./target/debug/my_program");
///     let args = vec![CString::new("my_program").unwrap(), CString::new("my_program").unwrap()];
///     // returns control shortly after forking off the debuggee as child process
///     debugger.run(program_path, &args)?;
///
///     if let Feedback::Registers(regs) = debugger.dump_regs()? {
///         println!("rip is here: {}", regs.rip)
///     } else {
///         eprintln!("something did not work!")
///     }
///
///     // Step over a single instruction
///     debugger.single_step()?;
///
///     // more automated debugging...
///
///     // Clean up resources
///     debugger.cleanup()?;
///
///     Ok(())
/// }
/// ```
pub struct Debugger<'executable, UI: DebuggerUI> {
    pub(crate) debuggee: Option<Debuggee>,
    pub(crate) ui: UI,
    stored_obj_data: Option<object::File<'executable>>,
    stored_obj_data_raw: Vec<u8>,
}

impl<'executable, UI: DebuggerUI> Debugger<'executable, UI> {
    /// Creates a new debugger with the provided user interface
    ///
    /// # Parameters
    ///
    /// * `ui` - The user interface implementation
    ///
    /// # Returns
    ///
    /// * `Ok(Debugger)` - A new debugger instance
    /// * `Err(DebuggerError)` - If the debugger could not be created
    ///
    /// # Errors
    ///
    /// Cannot fail.
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::debugger::Debugger;
    /// use coreminer::ui::cli::CliUi;
    ///
    /// let ui = CliUi::build(None).unwrap();
    /// let debugger = Debugger::build(ui).unwrap();
    /// ```
    pub fn build(ui: UI) -> Result<Self> {
        Ok(Debugger {
            debuggee: None,
            ui,
            stored_obj_data: None,
            stored_obj_data_raw: Vec::new(),
        })
    }

    /// Launches a new debuggee process
    ///
    /// This function loads an executable, parses its debug information, forks a new process,
    /// and sets up ptrace for debugging.
    ///
    /// # Parameters
    ///
    /// * `path` - Path to the executable
    /// * `args` - Command-line arguments for the executable
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the debuggee was successfully launched
    /// * `Err(DebuggerError)` - If the debuggee could not be launched
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The executable does not exist
    /// - The executable is not a valid file
    /// - Debug information cannot be parsed
    /// - The process cannot be forked
    /// - ptrace cannot be initialized
    fn launch_debuggee(&mut self, path: impl AsRef<Path>, args: &[CString]) -> Result<()> {
        let path: &Path = path.as_ref();
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

        let executable_obj_data: object::File<'_> = self.stored_obj_data.take().unwrap();

        let dbginfo: CMDebugInfo = CMDebugInfo::build(executable_obj_data)?;

        let fork_res = unsafe { nix::unistd::fork() };
        match fork_res {
            Err(e) => {
                error!("could not start executable: {e}");
                Err(e.into())
            }
            Ok(fr) => match fr {
                nix::unistd::ForkResult::Parent { child: pid } => {
                    let dbge = Debuggee::build(pid, &dbginfo, HashMap::new())?;
                    self.debuggee = Some(dbge);
                    Ok(())
                }
                nix::unistd::ForkResult::Child => {
                    let cpath = CString::new(path.to_string_lossy().to_string().as_str())?;
                    ptrace::traceme()
                        .inspect_err(|e| eprintln!("error while doing traceme: {e}"))?;
                    execv(&cpath, args)?; // NOTE: unsure if args[0] is set to the executable
                    unreachable!()
                }
            },
        }
    }

    /// Waits for a signal from the debuggee and processes it
    ///
    /// This function waits for signals from the debuggee, such as breakpoints, signals, or exits,
    /// and processes them appropriately.
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback)` - The result of the wait operation
    /// * `Err(DebuggerError)` - If there was an error during waiting
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - waitpid fails
    /// - Signal information cannot be retrieved
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// match debugger.wait_signal() {
    ///     Ok(Feedback::Exit(code)) => println!("Process exited with code {}", code),
    ///     Ok(Feedback::Ok) => println!("Process stopped"),
    ///     Ok(other) => println!("something else happened: {other}"), // impossible
    ///     Err(e) => eprintln!("Error: {}", e),
    /// }
    /// ```
    pub fn wait_signal(&self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        loop {
            match self.wait(&[])? {
                WaitStatus::Exited(_, exit_code) => {
                    return Ok(Feedback::Exit(exit_code));
                }
                WaitStatus::Signaled(_, signal, _) => {
                    debug!("Debuggee terminated by signal: {}", signal);
                    return Ok(Feedback::Exit(-1));
                }
                _ => {
                    // Get and handle other signals as before
                    let siginfo = ptrace::getsiginfo(dbge.pid)?;
                    let sig = Signal::try_from(siginfo.si_signo)?;
                    match sig {
                        Signal::SIGTRAP => {
                            self.handle_sigtrap(sig, siginfo)?;
                            return Ok(Feedback::Ok);
                        }
                        Signal::SIGSEGV
                        | Signal::SIGINT
                        | Signal::SIGPIPE
                        | Signal::SIGSTOP
                        | Signal::SIGWINCH
                        | Signal::SIGILL => {
                            self.handle_important_signal(sig, siginfo)?;
                            return Ok(Feedback::Ok);
                        }
                        _ => {
                            self.handle_other_signal(sig, siginfo)?;
                            continue;
                        }
                    }
                }
            }
        }
    }

    /// Low-level wait for a change in the debuggee's state
    ///
    /// # Parameters
    ///
    /// * `options` - Options to pass to waitpid, usually `&[]`
    ///
    /// # Returns
    ///
    /// * `Ok(WaitStatus)` - The status of the wait operation
    /// * `Err(DebuggerError)` - If there was an error during waiting
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - waitpid fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use nix::sys::wait::{WaitPidFlag, WaitStatus};
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Wait for any status change without options
    /// let status = debugger.wait(&[]).unwrap();
    ///
    /// // Wait with the WNOHANG option (non-blocking)
    /// let status = debugger.wait(&[WaitPidFlag::WNOHANG]).unwrap();
    ///
    /// match status {
    ///     WaitStatus::Exited(_, code) => println!("Process exited with code {}", code),
    ///     WaitStatus::Stopped(_, signal) => println!("Process stopped by signal {:?}", signal),
    ///     _ => println!("Other status change"),
    /// }
    /// ```
    pub fn wait(&self, options: &[WaitPidFlag]) -> Result<WaitStatus> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        let mut flags = WaitPidFlag::empty();
        for f in options {
            flags |= *f;
        }
        Ok(waitpid(
            dbge.pid,
            if flags.is_empty() { None } else { Some(flags) },
        )?)
    }

    /// Runs the main debugger loop
    ///
    /// This function forms the main execution loop of the debugger, processing
    /// UI commands and executing corresponding debugger actions until the user
    /// requests to quit.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the debugger loop completed successfully
    /// * `Err(DebuggerError)` - If there was an error during execution
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee already exists but can't be waited for.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// let ui = CliUi::build(None).unwrap();
    /// let mut debugger = Debugger::build(ui).unwrap();
    ///
    /// // Start the main debugger loop
    /// // This will run until the ui exits
    /// debugger.run_debugger().unwrap();
    ///
    /// debugger.cleanup().unwrap();
    /// ```
    pub fn run_debugger(&mut self) -> Result<()> {
        if self.debuggee.as_ref().is_some() {
            self.wait_signal()?; // wait until the debuggee is stopped
        } else {
            info!("debuggee not yet launched");
        }

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
                        Status::DisassembleAt(a, l, literal) => self.disassemble_at(a, l, literal),
                        Status::GetSymbolsByName(s) => self.get_symbol_by_name(s),
                        Status::StepSingle => self.single_step(),
                        Status::StepOut => self.step_out(),
                        Status::StepInto => self.step_into(),
                        Status::StepOver => self.step_over(),
                        Status::Backtrace => self.backtrace(),
                        Status::ReadVariable(va) => self.read_variable(&va),
                        Status::WriteVariable(va, val) => self.write_variable(&va, val),
                        Status::GetStack => self.get_stack(),
                        Status::ProcMap => self.get_process_map(),
                        Status::Run(exe, args) => self.run(&exe, &args),
                    },
                }
            }
            .into();

            // Clean up if process exited
            if let Feedback::Exit(_) = feedback {
                self.debuggee = None;
            }
        }

        Ok(())
    }

    /// Continues execution of the debuggee, optionally delivering a signal
    ///
    /// This function tells the debuggee to continue execution from its current state.
    /// If a signal is provided, it will be delivered to the debuggee.
    ///
    /// # Parameters
    ///
    /// * `sig` - An optional signal to deliver to the debuggee, usually [`None`]
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback)` - The result of the continuation
    /// * `Err(DebuggerError)` - If there was an error during continuation
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - ptrace's cont operation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use nix::sys::signal::Signal;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Continue execution normally
    /// debugger.cont(None).unwrap();
    ///
    /// // Continue execution and deliver a SIGINT signal
    /// debugger.cont(Some(Signal::SIGINT)).unwrap();
    /// ```
    pub fn cont(&mut self, sig: Option<Signal>) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        ptrace::cont(dbge.pid, sig)?;

        self.wait_signal() // wait until the debuggee is stopped again!!!
    }

    /// Gets the current registers of the debuggee
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Registers)` - The registers
    /// * `Err(DebuggerError)` - If there was an error retrieving registers
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - ptrace's getregs operation fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// if let Ok(Feedback::Registers(regs)) = debugger.dump_regs() {
    ///     println!("RIP: {:#x}", regs.rip);
    ///     println!("RSP: {:#x}", regs.rsp);
    ///     println!("RAX: {:#x}", regs.rax);
    /// }
    /// ```
    pub fn dump_regs(&self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        let regs = ptrace::getregs(dbge.pid)?;
        Ok(Feedback::Registers(regs.into()))
    }

    /// Cleans up resources used by the debugger
    ///
    /// This function terminates the debuggee if it's still running
    /// and releases any resources held by the debugger.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If cleanup was successful
    /// * `Err(DebuggerError)` - If there was an error during cleanup
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The exists but debuggee cannot be killed with [`Debuggee::kill`]
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// #
    /// // Clean up resources at the end of debugging
    /// debugger.cleanup().unwrap();
    /// ```
    pub fn cleanup(&mut self) -> Result<()> {
        if let Some(dbge) = &self.debuggee {
            dbge.kill()?;
            self.debuggee = None;
        }
        Ok(())
    }

    /// Sets a breakpoint at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to set the breakpoint at
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the breakpoint was set successfully
    /// * `Err(DebuggerError)` - If there was an error setting the breakpoint
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The breakpoint could not be enabled
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::addr::Addr;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Set a breakpoint at absolute address 0x1000
    /// debugger.set_bp(Addr::from(0x1000usize)).unwrap();
    ///
    /// // Set a breakpoint at the program's entry point
    /// let base = debugger.get_current_addr().unwrap();
    /// debugger.set_bp(base).unwrap();
    ///
    /// // Set a breakpoint at the relative address 0x1000
    /// let base = debugger.get_current_addr().unwrap();
    /// debugger.set_bp(base + 0x1000).unwrap();
    /// ```
    pub fn set_bp(&mut self, addr: Addr) -> Result<Feedback> {
        let dbge = self.debuggee.as_mut().ok_or(DebuggerError::NoDebugee)?;
        let mut bp = Breakpoint::new(dbge.pid, addr);
        bp.enable()?;
        dbge.breakpoints.insert(addr, bp);

        Ok(Feedback::Ok)
    }

    /// Removes a breakpoint at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to remove the breakpoint from
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the breakpoint was removed successfully
    /// * `Err(DebuggerError)` - If there was an error removing the breakpoint
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The breakpoint could not be disabled
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::addr::Addr;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Remove a breakpoint at address 0x1000
    /// debugger.del_bp(Addr::from(0x1000usize)).unwrap();
    ///
    /// // Remove a breakpoint at the relative address 0x1000
    /// let base = debugger.get_current_addr().unwrap();
    /// debugger.del_bp(base + 0x1000).unwrap();
    /// ```
    pub fn del_bp(&mut self, addr: Addr) -> Result<Feedback> {
        let dbge = self.debuggee.as_mut().ok_or(DebuggerError::NoDebugee)?;

        if let Some(_bp) = dbge.breakpoints.get_mut(&addr) {
            dbge.breakpoints.remove(&addr); // gets disabled on dropping
        } else {
            warn!("removed a breakpoint at {addr:x?} that did not exist");
        }

        Ok(Feedback::Ok)
    }

    /// Performs a single, atomic step of exactly one instruction through the debuggee
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the step was successful
    /// * `Err(DebuggerError)` - If there was an error during stepping
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - ptrace's step operation fails
    fn atomic_single_step(&self) -> Result<()> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        // FIXME: this is probably noticeable
        if let Err(e) = ptrace::step(dbge.pid, None) {
            error!("could not do atomic step: {e}");
            return Err(e.into());
        }

        Ok(())
    }

    /// Steps a single instruction in the debuggee
    ///
    /// This function advances execution by a single instruction, taking
    /// care to handle breakpoints appropriately.
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the step was successful
    /// * `Err(DebuggerError)` - If there was an error during stepping
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - ptrace operations fail
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Step through one instruction
    /// debugger.single_step().unwrap();
    ///
    /// // Step through multiple instructions
    /// for _ in 0..5 {
    ///     debugger.single_step().unwrap();
    /// }
    /// ```
    pub fn single_step(&mut self) -> Result<Feedback> {
        if self.go_back_step_over_bp()? {
            info!("breakpoint before, caught up and continueing with single step");
        }
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let maybe_bp_addr: Addr = self.get_current_addr()?;
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

    /// Steps out of the current function
    ///
    /// This function sets a temporary breakpoint at the return address
    /// and continues execution until that breakpoint is hit.
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the step-out was successful
    /// * `Err(DebuggerError)` - If there was an error during step-out
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - No valid return address of the current function can be found
    /// - The current function is main (cannot step out)
    /// - Could not read or write a [Register].
    /// - Could not read from memory.
    /// - Could not set or delete a [Breakpoint].
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running and in a function
    /// #
    /// // Step out of the current function
    /// debugger.step_out().unwrap();
    /// ```
    pub fn step_out(&mut self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        {
            let a = dbge.get_function_by_addr(self.get_reg(Register::rip)?.into())?;
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
        let return_addr: Addr = mem_read_word(dbge.pid, stack_frame_pointer + 8)?.into();
        trace!("rsb: {stack_frame_pointer}");
        trace!("ret_addr: {return_addr}");

        let should_remove_breakpoint = if dbge.breakpoints.contains_key(&return_addr) {
            false
        } else {
            self.set_bp(return_addr)?;
            true
        };

        self.cont(None)?;

        if should_remove_breakpoint {
            self.del_bp(return_addr)?;
            self.set_reg(Register::rip, self.get_reg(Register::rip)? - 1)?; // we need to go back
                                                                            // else we skip an instruction
        }
        Ok(Feedback::Ok)
    }

    /// Temporarily disables a breakpoint, steps over it, and then re-enables it
    ///
    /// # Parameters
    ///
    /// * `here` - The address of the breakpoint
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the operation was successful
    /// * `Err(DebuggerError)` - If there was an error
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Breakpoint operations fail
    /// - Step operations fail
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

    /// Checks if we need to restore an instruction pointer after hitting a breakpoint
    ///
    /// When a breakpoint is hit, the instruction pointer is just after the INT3 instruction.
    /// This function checks if we need to move the instruction pointer back to the breakpoint
    /// address and execute the original instruction. If so, it does that.
    ///
    /// # Returns
    ///
    /// * `Ok(bool)` - True if the IP was adjusted and a breakpoint was stepped over
    /// * `Err(DebuggerError)` - If there was an error
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Register operations fail
    /// - Breakpoint operations fail
    #[allow(clippy::missing_panics_doc)] // this function cant panic
    pub fn go_back_step_over_bp(&mut self) -> Result<bool> {
        if self.debuggee.is_none() {
            return Err(DebuggerError::NoDebugee);
        }

        let maybe_bp_addr: Addr = self.get_current_addr()? - 1;
        trace!("Checkinf if {maybe_bp_addr} had a breakpoint");

        if self
            .debuggee
            .as_mut()
            // safe because we check earlier, needed because we use a mutable reference
            // and can only drop in place this way
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

    /// Disassembles memory at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The starting address
    /// * `len` - The number of bytes to disassemble
    /// * `literal` - Whether to show literal bytes instead of original code (including breakpoint instructions)
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Disassembly)` - The disassembly result
    /// * `Err(DebuggerError)` - If there was an error during disassembly
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Memory cannot be read
    /// - Disassembly fails
    ///
    /// # Panics
    ///
    /// If a [Breakpoint] is enabled but has no saved data, this will panic.
    /// If a [Breakpoint] was found before making the [Disassembly], but the same breakpoint does
    /// not exist after the [Disassembly] was created, this will also panic.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::addr::Addr;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Disassemble 16 bytes at address 0x1000
    /// if let Ok(Feedback::Disassembly(disasm)) = debugger.disassemble_at(Addr::from(0x1000usize), 16, false) {
    ///     for (addr, raw, content, has_bp) in disasm.inner() {
    ///         println!("{}: {} {}", addr, if *has_bp { "*" } else { " " }, content[0].0);
    ///     }
    /// }
    /// ```
    pub fn disassemble_at(&self, addr: Addr, len: usize, literal: bool) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let t = dbge.disassemble(addr, len, literal)?;

        Ok(Feedback::Disassembly(t))
    }

    /// Searches for symbols by name
    ///
    /// # Parameters
    ///
    /// * `name` - The name to search for
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Symbols)` - The matching symbols
    /// * `Err(DebuggerError)` - If there was an error during search
    ///
    /// Note: If the executable that is being debugged has no DWARF information (was stripped), this will always
    /// return no symbols.
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debug information is not loaded
    /// - Symbol information is not available
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Find the "main" function
    /// if let Ok(Feedback::Symbols(symbols)) = debugger.get_symbol_by_name("main") {
    ///     for symbol in symbols {
    ///         println!("Found main at: {:?}", symbol.low_addr());
    ///     }
    /// }
    /// ```
    pub fn get_symbol_by_name(&self, name: impl Display) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let symbols: Vec<OwnedSymbol> = dbge.get_symbol_by_name(name)?;
        Ok(Feedback::Symbols(symbols))
    }

    /// Handles a SIGTRAP signal from the debuggee
    ///
    /// # Parameters
    ///
    /// * `sig` - The signal
    /// * `siginfo` - The signal information
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the signal was handled successfully
    /// * `Err(DebuggerError)` - If there was an error handling the signal
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    pub fn handle_sigtrap(
        &self,
        sig: nix::sys::signal::Signal,
        siginfo: nix::libc::siginfo_t,
    ) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);

        match siginfo.si_code {
            SI_KERNEL => trace!("SI_KERNEL"), // we don't know what do do?
            TRAP_BRKPT => {
                trace!("TRAP_BRKPT");
            }
            TRAP_TRACE => trace!("TRAP_TRACE"), // single stepping
            _ => warn!("Strange SIGTRAP code: {}", siginfo.si_code),
        }
        Ok(())
    }

    /// Handles important signals from the debuggee
    ///
    /// # Parameters
    ///
    /// * `sig` - The signal
    /// * `siginfo` - The signal information
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the signal was handled successfully
    /// * `Err(DebuggerError)` - If there was an error handling the signal
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    pub fn handle_important_signal(
        &self,
        sig: Signal,
        siginfo: nix::libc::siginfo_t,
    ) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);
        Ok(())
    }

    /// Handles other signals from the debuggee
    ///
    /// # Parameters
    ///
    /// * `sig` - The signal
    /// * `siginfo` - The signal information
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the signal was handled successfully
    /// * `Err(DebuggerError)` - If there was an error handling the signal
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    pub fn handle_other_signal(&self, sig: Signal, siginfo: nix::libc::siginfo_t) -> Result<()> {
        info!("debugee received {}: {}", sig.as_str(), siginfo.si_code);
        Ok(())
    }

    /// Logs information about the debugger state
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the information was logged successfully
    /// * `Err(DebuggerError)` - If there was an error
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    fn infos(&self) -> std::result::Result<Feedback, DebuggerError> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        info!("Breakpoints:\n{:#?}", dbge.breakpoints);
        Ok(Feedback::Ok)
    }

    /// Steps into a function call
    ///
    /// This function steps through instructions until a call instruction is found,
    /// then steps into that function.
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the step-into was successful
    /// * `Err(DebuggerError)` - If there was an error during step-into
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - No call instruction is found
    /// - Step operations fail
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Step into the next function call
    /// debugger.step_into().unwrap();
    /// ```
    #[allow(clippy::missing_panics_doc)] // this function cannot panic
    pub fn step_into(&mut self) -> Result<Feedback> {
        if self.debuggee.is_none() {
            return Err(DebuggerError::NoDebugee);
        }
        self.go_back_step_over_bp()?;

        loop {
            let rip: Addr = (self.get_reg(Register::rip)?).into();
            let disassembly: Disassembly =
                // unwrap is safe because we check earlier, needed because we need the mutable 
                // reference
                self.debuggee.as_ref().unwrap().disassemble(rip, 8, true)?;
            let next_instruction = &disassembly.inner()[0];
            let operator = next_instruction.2[0].clone();

            if operator.1 != FormatterTextKind::Mnemonic {
                error!("could not read operator from disassembly");
            }
            // PERF: this is very inefficient :/ maybe remove the autostepper or work with continue
            // somehow
            if operator.0.trim() == "call" {
                self.single_step()?;
                break;
            }
            self.single_step()?;
        }

        Ok(Feedback::Ok)
    }

    /// Steps over a function call
    ///
    /// This function combines [`Self::step_into`] and [`Self::step_out`] to step over a function call.
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the step-over was successful
    /// * `Err(DebuggerError)` - If there was an error during step-over
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Step operations fail
    pub fn step_over(&mut self) -> Result<Feedback> {
        self.go_back_step_over_bp()?;

        self.step_into()?;
        self.step_out()
    }

    /// Gets a backtrace of the current call stack
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Backtrace)` - The backtrace
    /// * `Err(DebuggerError)` - If there was an error generating the backtrace
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Stack unwinding fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Get a backtrace of the call stack
    /// if let Ok(Feedback::Backtrace(bt)) = debugger.backtrace() {
    ///     for (i, frame) in bt.frames.iter().enumerate() {
    ///         println!("#{} {} at {:?}", i, frame.name.as_deref().unwrap_or("??"), frame.addr);
    ///     }
    /// }
    /// ```
    pub fn backtrace(&self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let backtrace = unwind::unwind(dbge.pid)?;

        Ok(Feedback::Backtrace(backtrace))
    }

    /// Gets the current instruction pointer address
    ///
    /// # Returns
    ///
    /// * `Ok(Addr)` - The current address
    /// * `Err(DebuggerError)` - If there was an error getting the address
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - [Register] read fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Get the current instruction pointer
    /// let curr_addr = debugger.get_current_addr().unwrap();
    /// println!("Current IP: {}", curr_addr);
    /// ```
    pub fn get_current_addr(&self) -> Result<Addr> {
        Ok((self.get_reg(Register::rip)?).into())
    }

    /// Prepares for variable access by gathering necessary context
    ///
    /// # Parameters
    ///
    /// * `expression` - The variable expression to access
    ///
    /// # Returns
    ///
    /// * `Ok((OwnedSymbol, OwnedSymbol, FrameInfo))` - Function, variable symbols and frame info
    /// * `Err(DebuggerError)` - If preparation failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The current location is not in a function
    /// - The variable is not found
    /// - Frame information cannot be constructed
    fn prepare_variable_access(
        &self,
        expression: &VariableExpression,
    ) -> Result<(OwnedSymbol, OwnedSymbol, FrameInfo)> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        let rip: Addr = self.get_current_addr()?;

        // Get current function
        let current_function = match dbge.get_function_by_addr(rip)? {
            Some(f) if f.frame_base().is_some() => f,
            Some(_) => {
                return Err(DebuggerError::AttributeDoesNotExist(
                    gimli::DW_AT_frame_base,
                ))
            }
            None => return Err(DebuggerError::NotInFunction),
        };

        // Find variable
        let locals = dbge.get_local_variables(rip)?;
        let vars = dbge.filter_expressions(&locals, expression)?;
        let var = match vars.len() {
            0 => {
                return Err(DebuggerError::VarExprReturnedNothing(
                    expression.to_string(),
                ))
            }
            1 => vars[0].clone(),
            _ => return Err(DebuggerError::AmbiguousVarExpr(expression.to_string())),
        };

        // Build frame info
        let mut frame_info = FrameInfo::new(
            None,
            Some(Into::<Addr>::into(self.get_reg(Register::rbp)?) + 16usize),
        );

        let frame_base = dbge.parse_location(
            current_function.frame_base().unwrap(),
            &frame_info,
            current_function.encoding(),
        )?;

        let frame_base: Addr = match frame_base {
            gimli::Location::Address { address } => address.into(),
            other => unimplemented!(
                "frame base DWARF location was not an address as expected: is {other:?}"
            ),
        };

        frame_info.frame_base = Some(frame_base);

        Ok((current_function, var, frame_info))
    }

    /// Reads the value of a variable
    ///
    /// # Parameters
    ///
    /// * `expression` - The variable name to read
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Variable)` - The variable value
    /// * `Err(DebuggerError)` - If the variable could not be read
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The variable is not found
    /// - The variable cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Read the value of a variable named "count"
    /// if let Ok(Feedback::Variable(value)) = debugger.read_variable("count".to_string()) {
    ///     println!("count = {:?}", value);
    /// }
    /// ```
    pub fn read_variable(&self, expression: &VariableExpression) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let (_, symbol, frame_info) = self.prepare_variable_access(expression)?;

        let val = dbge.var_read(&symbol, &frame_info)?;

        Ok(Feedback::Variable(val))
    }

    /// Writes a value to a variable
    ///
    /// # Parameters
    ///
    /// * `expression` - The variable name to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the write was successful
    /// * `Err(DebuggerError)` - If the variable could not be written
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The variable is not found
    /// - The variable cannot be accessed
    /// - The value is incompatible with the variable type
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Set the value of a variable named "count" to 42
    /// debugger.write_variable("count".to_string(), 42).unwrap();
    /// ```
    pub fn write_variable(
        &self,
        expression: &VariableExpression,
        value: impl Into<VariableValue>,
    ) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let (_, var, frame_info) = self.prepare_variable_access(expression)?;

        dbge.var_write(&var, &frame_info, value.into())?;

        Ok(Feedback::Ok)
    }

    /// Reads a single [Word] from memory at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to read from
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Word)` - The value read from memory
    /// * `Err(DebuggerError)` - If the memory could not be read
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The memory address is invalid
    /// - The memory cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::addr::Addr;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Read a word from memory at address 0x1000
    /// if let Ok(Feedback::Word(value)) = debugger.read_mem(Addr::from(0x1000usize)) {
    ///     println!("Memory at 0x1000: {:#x}", value);
    /// }
    /// ```
    pub fn read_mem(&self, addr: Addr) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let w = mem_read_word(dbge.pid, addr)?;

        Ok(Feedback::Word(w))
    }

    /// Writes a [Word] to memory at the specified address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to write to
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the write was successful
    /// * `Err(DebuggerError)` - If the memory could not be written
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The memory address is invalid
    /// - The memory cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::addr::Addr;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Write the value 0x42 to memory at address 0x1000
    /// debugger.write_mem(Addr::from(0x1000usize), 0x42).unwrap();
    /// ```
    pub fn write_mem(&self, addr: Addr, value: Word) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        mem_write_word(dbge.pid, addr, value)?;

        Ok(Feedback::Ok)
    }

    /// Gets the value of a register
    ///
    /// # Parameters
    ///
    /// * `r` - The register to get
    ///
    /// # Returns
    ///
    /// * `Ok(u64)` - The register value
    /// * `Err(DebuggerError)` - If the register could not be read
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Register access fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::Register;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Get the value of the rax register
    /// let rax = debugger.get_reg(Register::rax).unwrap();
    /// println!("RAX: {:#x}", rax);
    /// ```
    pub fn get_reg(&self, r: Register) -> Result<u64> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        crate::get_reg(dbge.pid, r)
    }

    /// Sets the value of a register
    ///
    /// # Parameters
    ///
    /// * `r` - The register to set
    /// * `v` - The value to set
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the register was set successfully
    /// * `Err(DebuggerError)` - If the register could not be set
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Register access fails
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::Register;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Set the value of the rax register to 0x42
    /// debugger.set_reg(Register::rax, 0x42).unwrap();
    /// ```
    pub fn set_reg(&self, r: Register, v: u64) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;
        crate::set_reg(dbge.pid, r, v)?;
        Ok(Feedback::Ok)
    }

    /// Gets the current stack of the debugged process
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Stack)` - The stack
    /// * `Err(DebuggerError)` - If the stack could not be retrieved
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - Stack memory cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Get the current stack
    /// if let Ok(Feedback::Stack(stack)) = debugger.get_stack() {
    ///     println!("Stack: {}", stack);
    /// }
    /// ```
    pub fn get_stack(&self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let stack = dbge.get_stack()?;
        Ok(Feedback::Stack(stack))
    }

    /// Gets the process memory map
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::ProcessMap)` - The process memory map
    /// * `Err(DebuggerError)` - If the memory map could not be retrieved
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The debuggee is not running
    /// - The memory map cannot be accessed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use coreminer::feedback::Feedback;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let debugger = Debugger::build(ui).unwrap();
    /// # // Assume debuggee is already running
    /// #
    /// // Get the process memory map
    /// if let Ok(Feedback::ProcessMap(map)) = debugger.get_process_map() {
    ///     println!("{:?}", map);
    /// }
    /// ```
    pub fn get_process_map(&self) -> Result<Feedback> {
        let dbge = self.debuggee.as_ref().ok_or(DebuggerError::NoDebugee)?;

        let pm = dbge.get_process_map()?;

        Ok(Feedback::ProcessMap(pm))
    }

    /// Runs a program for debugging
    ///
    /// This function loads an executable, parses its debug information, and
    /// launches it under debugger control.
    ///
    /// # Parameters
    ///
    /// * `executable_path` - Path to the executable
    /// * `arguments` - Command-line arguments for the executable
    ///
    /// # Returns
    ///
    /// * `Ok(Feedback::Ok)` - If the program was launched successfully
    /// * `Err(DebuggerError)` - If the program could not be launched
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - A debuggee is already running
    /// - The executable does not exist
    /// - The executable is not a valid file
    /// - Debug information cannot be parsed
    /// - The process cannot be forked
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use coreminer::debugger::Debugger;
    /// # use coreminer::ui::cli::CliUi;
    /// # use std::path::Path;
    /// # use std::ffi::CString;
    /// #
    /// # let ui = CliUi::build(None).unwrap();
    /// # let mut debugger = Debugger::build(ui).unwrap();
    /// #
    /// // Run a program with arguments
    /// let program = Path::new("./target/debug/my_program");
    /// let args = vec![
    ///     CString::new("my_program").unwrap(),
    ///     CString::new("--arg1").unwrap(),
    ///     CString::new("value1").unwrap()
    /// ];
    ///
    /// debugger.run(program, &args).unwrap();
    /// ```
    pub fn run(
        &mut self,
        executable_path: impl AsRef<Path>,
        arguments: &[CString],
    ) -> Result<Feedback> {
        if self.debuggee.is_some() {
            return Err(DebuggerError::AlreadyRunning);
        }

        // NOTE: the lifetimes of the raw object data have given us many problems. It would be
        // possible to read the object data out in the main function and passing it to the
        // constructor of Debugger, but that would mean that we cannot debug a different program in
        // the same session.
        let exe: &Path = executable_path.as_ref();

        // First, read the file data
        self.stored_obj_data_raw = std::fs::read(exe)?;

        // Create a new scope to handle the borrow checker
        {
            // Create a reference to the raw data that matches the 'executable lifetime
            let raw_data: &'executable [u8] = unsafe {
                std::mem::transmute::<&[u8], &'executable [u8]>(&self.stored_obj_data_raw)
            };

            // Parse the object file
            let obj_data = object::File::parse(raw_data)?;
            self.stored_obj_data = Some(obj_data);
        }

        // Now launch the debuggee
        self.launch_debuggee(exe, arguments)?;

        Ok(Feedback::Ok)
    }
}
