use std::str::FromStr;

use crate::errors::{DebuggerError, Result};
use crate::feedback::Feedback;
use crate::{Addr, Word};

pub mod cli;

pub enum Status {
    Backtrace,
    StepOver,
    StepInto,
    StepOut,
    StepSingle,
    GetSymbolsByName(String),
    DisassembleAt(Addr, usize),
    DebuggerQuit,
    Continue,
    SetBreakpoint(Addr),
    DelBreakpoint(Addr),
    DumpRegisters,
    SetRegister(Register, u64),
    WriteMem(Addr, Word),
    ReadMem(Addr),
    Infos,
}

pub trait DebuggerUI {
    fn process(&mut self, feedback: Feedback) -> Result<Status>;
}

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
