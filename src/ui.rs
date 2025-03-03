//! # User Interface Module
//!
//! Provides interfaces and implementations for interacting with the debugger.
//!
//! This module defines the core user interface abstractions used by the debugger,
//! allowing for different interface implementations (such as CLI, JSON-RPC, etc.)
//! while maintaining a consistent API for the debugger core to interact with.
//!
//! The primary components are:
//! - The [`Status`] enum, which represents commands from the UI to the debugger
//! - The [`DebuggerUI`] trait, which defines the interface for UI implementations
//!
//! This module also includes submodules for specific UI implementations:
//! - [`cli`]: A command-line interface implementation

use std::ffi::CString;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::errors::Result;
use crate::feedback::Feedback;
use crate::{Addr, Register, Word};

pub mod cli;
pub mod json;

/// Represents a command from the UI to the debugger
///
/// [`Status`] encapsulates commands that can be sent from the user interface
/// to the debugger, such as setting breakpoints, stepping, continuing execution,
/// and inspecting memory or registers.
///
/// # Examples
///
/// ```
/// use coreminer::ui::Status;
/// use coreminer::addr::Addr;
/// use coreminer::Register;
///
/// // Command to set a breakpoint at address 0x1000
/// let status = Status::SetBreakpoint(Addr::from(0x1000usize));
///
/// // Command to continue execution
/// let status = Status::Continue;
///
/// // Command to set a register value
/// let status = Status::SetRegister(Register::rax, 0x42);
///
/// // Command to run a executable in the debugger
/// let status = Status::Run(Path::new("/bin/ls").into(), vec![]);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "PascalCase")]
pub enum Status {
    /// Generate a backtrace of the call stack
    Backtrace,

    /// Step over the current function call
    StepOver,

    /// Step into the current function call
    StepInto,

    /// Step out of the current function
    StepOut,

    /// Step a single instruction
    StepSingle,

    /// Look up symbols by name
    GetSymbolsByName(String),

    /// Disassemble memory at the specified address
    ///
    /// The boolean parameter indicates whether to show the literal bytes
    /// (including breakpoint instructions) instead of the original code.
    DisassembleAt(Addr, usize, bool),

    /// Exit the debugger
    DebuggerQuit,

    /// Continue execution
    Continue,

    /// Set a breakpoint at the specified address
    SetBreakpoint(Addr),

    /// Remove a breakpoint at the specified address
    DelBreakpoint(Addr),

    /// Get all register values
    DumpRegisters,

    /// Set a register value
    SetRegister(Register, u64),

    /// Write a value to memory
    WriteMem(Addr, Word),

    /// Read a value from memory
    ReadMem(Addr),

    /// Show debugger information
    Infos,

    /// Read a variable's value
    ReadVariable(String),

    /// Write a value to a variable
    WriteVariable(String, usize),

    /// Show the current stack
    GetStack,

    /// Show the process memory map
    ProcMap,

    /// Run a new program
    Run(PathBuf, Vec<CString>),
}

/// Interface for debugger user interfaces
///
/// [`DebuggerUI`] defines the interface that must be implemented by any user
/// interface that wants to interact with the debugger. It provides a way for
/// the debugger to send feedback to the UI and receive commands in return.
///
/// # Examples
///
/// ```no_run
/// use coreminer::ui::{DebuggerUI, Status};
/// use coreminer::feedback::Feedback;
/// use coreminer::errors::Result;
///
/// // A simple UI implementation that always returns Continue
/// struct SimpleUI;
///
/// impl DebuggerUI for SimpleUI {
///     fn process(&mut self, feedback: Feedback) -> Result<Status> {
///         println!("Received feedback: {}", feedback);
///         Ok(Status::Continue)
///     }
/// }
///
/// // Using the UI with a debugger
/// # fn run_example() -> Result<()> {
/// # use coreminer::debugger::Debugger;
/// let ui = SimpleUI;
/// let mut debugger = Debugger::build(ui)?;
/// debugger.run_debugger()?;
/// debugger.cleanup()?;
/// # Ok(())
/// # }
/// ```
pub trait DebuggerUI {
    /// Processes feedback from the debugger and returns a status command
    ///
    /// This method is called by the debugger to send feedback to the UI
    /// and receive a command in response. The UI implementation should
    /// present the feedback to the user in an appropriate way and then
    /// return a command based on user input or internal logic.
    ///
    /// # Parameters
    ///
    /// * `feedback` - The feedback from the debugger
    ///
    /// # Returns
    ///
    /// * `Ok(Status)` - The command to send to the debugger
    /// * `Err(DebuggerError)` - If an error occurred during processing
    ///
    /// # Errors
    ///
    /// This method can fail if there are issues with user input or other
    /// UI-specific errors.
    fn process(&mut self, feedback: Feedback) -> Result<Status>;
}
