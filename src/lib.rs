//! # Coreminer
//!
//! A debugger library and executable for Rust that provides low-level debugging capabilities,
//! particularly useful for debugging programs that may be resistant to standard
//! debugging approaches.
//!
//! ## Core Features
//!
//! - **Memory Access**: Read and write process memory
//! - **Register Control**: Access and modify CPU registers
//! - **Breakpoint Management**: Set, enable, disable, and remove breakpoints
//! - **Execution Control**: Step by step execution, continue execution, step in/out/over functions
//! - **Symbol Resolution**: Parse and use DWARF debug information for symbol lookup
//! - **Variable Inspection**: Access application variables through debug information
//! - **Stack Analysis**: Generate and inspect backtraces and stack frames
//! - **Disassembly**: Disassemble machine code to human readable assembly
//!
//! ## Architecture
//!
//! Coreminer is built around several core components:
//!
//! - **Debuggee**: Represents the debugged process, managed through ptrace
//! - **Debugger**: Coordinates the debugging session and UI communication
//! - **DWARF Parser**: Extracts debug information for symbol and variable resolution
//! - **Breakpoint Management**: Handles instruction replacement for breakpoints
//! - **UI Interface**: Accepts commands and provides feedback
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use coreminer::debugger::Debugger;
//! use coreminer::ui::cli::CliUi;
//!
//! fn main() -> Result<(), coreminer::errors::DebuggerError> {
//!     let ui = CliUi::build(None)?;
//!     let mut debugger = Debugger::build(ui)?;
//!     debugger.run_debugger()?;
//!     debugger.cleanup()?;
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::empty_docs)]

use std::array::TryFromSliceError;
use std::io::{Read, Seek, Write};
use std::str::FromStr;

use nix::sys::ptrace;
use nix::unistd::Pid;
use serde::Serialize;

use crate::errors::Result;

use self::addr::Addr;
use self::errors::DebuggerError;

pub mod addr;
pub mod breakpoint;
pub mod consts;
pub mod dbginfo;
pub mod debuggee;
pub mod debugger;
pub mod disassemble;
pub mod dwarf_parse;
pub mod errors;
pub mod feedback;
pub mod memorymap;
pub mod stack;
pub mod ui;
pub mod unwind;
pub mod variable;

/// Type alias for machine word-sized integers, used for register values and memory contents
pub type Word = i64;
/// Number of bytes in a [Word] (8 bytes on a 64-bit system)
pub const WORD_BYTES: usize = Word::BITS as usize / 8;

/// CPU register names for x86_64 architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[allow(non_camel_case_types)]
pub enum Register {
    r15,
    r14,
    r13,
    r12,
    rbp,
    rbx,
    r11,
    r10,
    r9,
    r8,
    rax,
    rcx,
    rdx,
    rsi,
    rdi,
    orig_rax,
    rip,
    cs,
    eflags,
    rsp,
    ss,
    fs_base,
    gs_base,
    ds,
    es,
    fs,
    gs,
}

impl FromStr for Register {
    type Err = DebuggerError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let s = s.to_lowercase();
        Ok(match s.as_str() {
            "r15" => Self::r15,
            "r14" => Self::r14,
            "r13" => Self::r13,
            "r12" => Self::r12,
            "rbp" => Self::rbp,
            "rbx" => Self::rbx,
            "r11" => Self::r11,
            "r10" => Self::r10,
            "r9" => Self::r9,
            "r8" => Self::r8,
            "rax" => Self::rax,
            "rcx" => Self::rcx,
            "rdx" => Self::rdx,
            "rsi" => Self::rsi,
            "rdi" => Self::rdi,
            "orig_rax" => Self::orig_rax,
            "rip" => Self::rip,
            "cs" => Self::cs,
            "eflags" => Self::eflags,
            "rsp" => Self::rsp,
            "ss" => Self::ss,
            "fs_base" => Self::fs_base,
            "gs_base" => Self::gs_base,
            "ds" => Self::ds,
            "es" => Self::es,
            "fs" => Self::fs,
            "gs" => Self::gs,
            _ => return Err(DebuggerError::ParseStr(s)),
        })
    }
}

impl TryFrom<gimli::Register> for Register {
    type Error = DebuggerError;
    /// Converts a DWARF register number to the corresponding Register enum value.
    ///
    /// The DWARF Register Number Mapping is defined in the amd64 ABI here:
    /// <https://refspecs.linuxbase.org/elf/x86_64-abi-0.99.pdf#figure.3.36>
    fn try_from(value: gimli::Register) -> Result<Self> {
        match value.0 {
            0 => Ok(Register::rax),
            1 => Ok(Register::rdx),
            2 => Ok(Register::rcx),
            3 => Ok(Register::rbx),
            4 => Ok(Register::rsi),
            5 => Ok(Register::rdi),
            6 => Ok(Register::rbp),
            7 => Ok(Register::rsp),
            8 => Ok(Register::r8),
            9 => Ok(Register::r9),
            10 => Ok(Register::r10),
            11 => Ok(Register::r11),
            12 => Ok(Register::r12),
            13 => Ok(Register::r13),
            14 => Ok(Register::r14),
            15 => Ok(Register::r15),
            16 => Ok(Register::rip),

            49 => Ok(Register::eflags),

            50 => Ok(Register::es),
            51 => Ok(Register::cs),
            52 => Ok(Register::ss),
            53 => Ok(Register::ds),
            54 => Ok(Register::fs),
            55 => Ok(Register::gs),

            56 => Ok(Register::fs_base),
            57 => Ok(Register::gs_base),

            // We skip 58..=62 because they correspond to tr, ldtr, mxcsr, fcw, fsw, etc.
            // which aren't in our enum.

            // No standard mapping for 63 or `orig_rax`
            // So we return None for those or anything else unrecognized.
            x => Err(DebuggerError::UnimplementedRegister(x)),
        }
    }
}

/// Writes a word-sized value to the specified process memory address
pub(crate) fn mem_write_word(pid: Pid, addr: Addr, value: Word) -> Result<()> {
    Ok(ptrace::write(pid, addr.into(), value)?)
}

/// Reads a word-sized value from the specified process memory address
pub(crate) fn mem_read_word(pid: Pid, addr: Addr) -> Result<Word> {
    Ok(ptrace::read(pid, addr.into())?)
}

/// Reads a slice of bytes from process memory at the specified address
pub(crate) fn mem_read(data_raw: &mut [u8], pid: Pid, addr: Addr) -> Result<usize> {
    let mut file = std::fs::File::options()
        .read(true)
        .write(false)
        .open(format!("/proc/{pid}/mem"))?;
    file.seek(std::io::SeekFrom::Start(addr.into()))?;
    let len = file.read(data_raw)?;

    Ok(len)
}

/// Writes a slice of bytes to process memory at the specified address
pub(crate) fn mem_write(data_raw: &[u8], pid: Pid, addr: Addr) -> Result<usize> {
    let mut file = std::fs::File::options()
        .read(false)
        .write(true)
        .open(format!("/proc/{pid}/mem"))?;
    file.seek(std::io::SeekFrom::Start(addr.into()))?;
    let len = file.write(data_raw)?;

    Ok(len)
}

/// Gets the value of a specified register for the target process
pub fn get_reg(pid: Pid, r: Register) -> Result<u64> {
    let regs = ptrace::getregs(pid)?;

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

/// Sets the value of a specified register for the target process
pub fn set_reg(pid: Pid, r: Register, v: u64) -> Result<()> {
    let mut regs = ptrace::getregs(pid)?;

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

    ptrace::setregs(pid, regs)?;

    Ok(())
}

/// Try to pad or truncate an array of [u8] into an array of constant size
pub(crate) fn fill_to_const_arr<const N: usize>(
    data: &[u8],
) -> std::result::Result<[u8; N], TryFromSliceError> {
    let mut d = data.to_vec();
    while d.len() < N {
        d.push(0);
    }
    let arr: [u8; N] = d.as_slice().try_into()?;
    Ok(arr)
}

/// Converts a byte slice to a [u64] value with proper padding
pub(crate) fn bytes_to_u64(bytes: &[u8]) -> std::result::Result<u64, TryFromSliceError> {
    const U64_BYTES: usize = u64::BITS as usize / 8;
    let data: [u8; U64_BYTES] = fill_to_const_arr(bytes)?;
    Ok(u64::from_ne_bytes(data))
}

#[cfg(test)]
mod test {
    use super::Register;
    #[test]
    fn test_dwarf_number_to_register() {
        assert_eq!(
            Register::try_from(gimli::Register(6)).expect("could not make register from valid num"),
            Register::rbp
        );
        assert_eq!(
            Register::try_from(gimli::Register(15))
                .expect("could not make register from valid num"),
            Register::r15
        );
        Register::try_from(gimli::Register(666)).expect_err("could make register from invalid num");
    }
}
