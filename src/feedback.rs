//! # Feedback Module
//!
//! Provides types for communicating between the debugger and user interface.
//!
//! This module defines the [`Feedback`] enum, which is used to represent
//! the results of debugging operations in a structured way that can be
//! presented to the user. It serves as the primary communication channel
//! between the debugger core and the user interface.
//!
//! The different variants of the [`Feedback`] enum represent different types
//! of information that might be returned from debugging operations, such as
//! register values, memory contents, disassembly, and error conditions.

use std::fmt::Display;

use nix::libc::user_regs_struct;
use serde::Serialize;

use crate::dbginfo::OwnedSymbol;
use crate::disassemble::Disassembly;
use crate::errors::DebuggerError;
use crate::memorymap::ProcessMemoryMap;
use crate::unwind::Backtrace;
use crate::variable::VariableValue;
use crate::{Addr, Word};

/// Represents the result of a debugging operation
///
/// [`Feedback`] is used to communicate the results of debugging operations
/// between the debugger core and the user interface. Each variant represents
/// a different type of result that might be returned from a debugging operation.
///
/// # Examples
///
/// ```no_run
/// use coreminer::feedback::Feedback;
/// use coreminer::addr::Addr;
///
/// // Function that might return different types of feedback
/// fn example_operation(operation: &str) -> Feedback {
///     match operation {
///         "read_mem" => Feedback::Word(42),
///         "get_addr" => Feedback::Addr(Addr::from(0x1000usize)),
///         "error" => Feedback::Error(coreminer::errors::DebuggerError::NoDebugee),
///         _ => Feedback::Text(format!("Unknown operation: {}", operation)),
///     }
/// }
///
/// // Processing feedback in a UI
/// fn display_feedback(feedback: Feedback) {
///     match feedback {
///         Feedback::Word(word) => println!("Word value: {:#x}", word),
///         Feedback::Addr(addr) => println!("Address: {}", addr),
///         Feedback::Error(err) => println!("Error: {}", err),
///         Feedback::Text(text) => println!("{}", text),
///         _ => println!("Other feedback type: {}", feedback),
///     }
/// }
/// ```
#[derive(Debug, Serialize)]
pub enum Feedback {
    /// Simple text message
    Text(String),

    /// Memory word value
    Word(Word),

    /// Memory address
    Addr(Addr),

    /// Register values
    Registers(UserRegs),

    /// Error condition
    Error(DebuggerError),

    /// Success with no specific data
    Ok,

    /// Disassembled code
    Disassembly(Disassembly),

    /// Call stack backtrace
    Backtrace(Backtrace),

    /// Debug symbols
    Symbols(Vec<OwnedSymbol>),

    /// Variable value
    Variable(VariableValue),

    /// Stack contents
    Stack(crate::stack::Stack),

    /// Process memory map
    ProcessMap(ProcessMemoryMap),

    /// Debuggee process exit
    Exit(i32),
}

impl Display for Feedback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Feedback::Ok => write!(f, "Ok")?,
            Feedback::Error(e) => write!(f, "Error: {e}")?,
            Feedback::Registers(regs) => write!(f, "Registers: {regs:#x?}")?,
            Feedback::Word(w) => write!(f, "Word: {w:#018x?}")?,
            Feedback::Addr(w) => write!(f, "Address: {w}")?,
            Feedback::Text(t) => write!(f, "{t}")?,
            Feedback::Disassembly(t) => write!(f, "{t:#?}")?,
            Feedback::Symbols(t) => write!(f, "Symbols: {t:#?}")?,
            Feedback::Backtrace(t) => write!(f, "Backtrace: {t:#?}")?,
            Feedback::Variable(t) => write!(f, "Variable: {t:#?}")?,
            Feedback::Stack(t) => write!(f, "Stack:\n{t}")?,
            Feedback::ProcessMap(pm) => write!(f, "Process Map:\n{pm:#x?}")?,
            Feedback::Exit(code) => write!(f, "Debugee exited with code {code}")?,
        }

        Ok(())
    }
}

impl From<Result<Feedback, DebuggerError>> for Feedback {
    fn from(value: Result<Feedback, DebuggerError>) -> Self {
        match value {
            Ok(f) => f,
            Err(e) => Feedback::Error(e),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UserRegs {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rax: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub orig_rax: u64,
    pub rip: u64,
    pub cs: u64,
    pub eflags: u64,
    pub rsp: u64,
    pub ss: u64,
    pub fs_base: u64,
    pub gs_base: u64,
    pub ds: u64,
    pub es: u64,
    pub fs: u64,
    pub gs: u64,
}

impl From<user_regs_struct> for UserRegs {
    fn from(regs: user_regs_struct) -> Self {
        Self {
            r15: regs.r15,
            r14: regs.r14,
            r13: regs.r13,
            r12: regs.r12,
            rbp: regs.rbp,
            rbx: regs.rbx,
            r11: regs.r11,
            r10: regs.r10,
            r9: regs.r9,
            r8: regs.r8,
            rax: regs.rax,
            rcx: regs.rcx,
            rdx: regs.rdx,
            rsi: regs.rsi,
            rdi: regs.rdi,
            orig_rax: regs.orig_rax,
            rip: regs.rip,
            cs: regs.cs,
            eflags: regs.eflags,
            rsp: regs.rsp,
            ss: regs.ss,
            fs_base: regs.fs_base,
            gs_base: regs.gs_base,
            ds: regs.ds,
            es: regs.es,
            fs: regs.fs,
            gs: regs.gs,
        }
    }
}
