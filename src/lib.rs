use std::fmt::Display;
use std::io::{Read, Seek, Write};
use std::ops::{Add, Sub};

use nix::sys::ptrace;
use nix::unistd::Pid;

use crate::errors::Result;

pub mod breakpoint;
pub mod consts;
pub mod dbginfo;
pub mod debugger;
pub mod disassemble;
pub mod dwarf_parse;
pub mod errors;
pub mod feedback;
pub mod ui;
pub mod unwind;

pub type Word = i64;
pub type RawPointer = *mut std::ffi::c_void;

#[derive(Hash, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Addr(pub RawPointer);

impl Addr {
    pub fn from_relative(base: Addr, raw: usize) -> Addr {
        Self::from(base.usize() + raw)
    }

    pub fn relative(&self, base: Addr) -> Addr {
        *self - base
    }

    pub fn usize(&self) -> usize {
        self.0 as usize
    }
    pub fn u64(&self) -> u64 {
        self.0 as u64
    }
    pub fn raw_pointer(&self) -> RawPointer {
        self.0
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#018x}", self.0 as usize)
    }
}

impl Add for Addr {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self((self.0 as usize + rhs.0 as usize) as RawPointer)
    }
}

impl Add<usize> for Addr {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        Self((self.0 as usize + rhs) as RawPointer)
    }
}

impl Sub for Addr {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self((self.0 as usize - rhs.0 as usize) as RawPointer)
    }
}

impl Sub<usize> for Addr {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self((self.0 as usize - rhs) as RawPointer)
    }
}

impl From<RawPointer> for Addr {
    fn from(value: RawPointer) -> Self {
        Addr(value)
    }
}

impl From<Addr> for RawPointer {
    fn from(value: Addr) -> Self {
        value.0
    }
}

impl From<usize> for Addr {
    fn from(value: usize) -> Self {
        Addr(value as RawPointer)
    }
}

impl From<Word> for Addr {
    fn from(value: Word) -> Self {
        Addr(value as RawPointer)
    }
}

impl From<u64> for Addr {
    fn from(value: u64) -> Self {
        Addr(value as RawPointer)
    }
}

impl From<Addr> for Word {
    fn from(value: Addr) -> Self {
        value.0 as Word
    }
}

impl From<Addr> for u64 {
    fn from(value: Addr) -> Self {
        value.0 as u64
    }
}

pub(crate) fn mem_write_word(pid: Pid, addr: Addr, value: Word) -> Result<()> {
    Ok(ptrace::write(pid, addr.into(), value)?)
}

pub(crate) fn mem_read_word(pid: Pid, addr: Addr) -> Result<Word> {
    Ok(ptrace::read(pid, addr.into())?)
}

pub(crate) fn mem_read(data_raw: &mut [u8], pid: Pid, addr: Addr) -> Result<usize> {
    let mut file = std::fs::File::options()
        .read(true)
        .write(false)
        .open(format!("/proc/{pid}/mem"))?;
    file.seek(std::io::SeekFrom::Start(addr.into()))?;
    let len = file.read(data_raw)?;

    Ok(len)
}

pub(crate) fn mem_write(data_raw: &[u8], pid: Pid, addr: Addr) -> Result<usize> {
    let mut file = std::fs::File::options()
        .read(false)
        .write(true)
        .open(format!("/proc/{pid}/mem"))?;
    file.seek(std::io::SeekFrom::Start(addr.into()))?;
    let len = file.write(data_raw)?;

    Ok(len)
}
