//! # Breakpoint Module
//!
//! Provides functionality for setting, enabling, and disabling breakpoints in a debugged process.
//!
//! This module implements the core breakpoint mechanism used by coreminer. Breakpoints work by
//! temporarily replacing an instruction in the target process with a special interrupt instruction
//! (INT3, `0xCC`), which causes the process to stop and signal the debugger when executed.
//!
//! When a breakpoint is hit, the debugger can then restore the original instruction, single-step
//! the process to execute that instruction, and then replace the breakpoint before continuing
//! execution.

use nix::unistd::Pid;
use tracing::trace;

use crate::errors::{DebuggerError, Result};
use crate::{mem_read_word, mem_write_word, Addr};

/// Mask to set all bits to 1 (using two's complement)
pub const MASK_ALL: i64 = -1; // yup for real, two's complement
/// The INT3 instruction byte (0xCC) used for software breakpoints
pub const INT3_BYTE: u8 = 0xcc;
/// `INT3_BYTE` represented as a [`crate::Word`]
pub const INT3: i64 = INT3_BYTE as i64;
/// Mask to isolate the lowest byte in a Word
pub const WORD_MASK: i64 = 0x0000_0000_0000_00ff;
/// Inverse of `WORD_MASK` (all bits set except the lowest byte)
pub const WORD_MASK_INV: i64 = MASK_ALL ^ WORD_MASK;

/// Represents a breakpoint in the debugged process
///
/// A [`Breakpoint`] maintains information about a location in the target process's
/// code where execution should be paused. It manages the original instruction byte
/// that was replaced with an INT3 instruction.
///
/// Breakpoints need to be enabled first. Enabling them means that the instruction at the
/// [address](crate::addr::Addr) is overwritten with `INT3`, and the old value of that byte is
/// stored in this datastructure.
///
/// Similarly, to execture the original code, breakpoints need to be disabled again, replacing the
/// artificial `INT3` with the original byte.
///
/// When a [Breakpoint] is dropped while still enabled, it is automatically disabled, see
/// [`Breakpoint::drop`].
///
/// # Examples
///
/// ```no_run
/// use coreminer::breakpoint::Breakpoint;
/// use coreminer::addr::Addr;
/// use nix::unistd::Pid;
///
/// // Create a new breakpoint at address 0x000055dd73ea3fb8 for process with PID 1234
/// let mut bp = Breakpoint::new(Pid::from_raw(1234), Addr::from(0x000055dd73ea3fb8usize));
///
/// // Enable the breakpoint (in a real program, this would modify the target process memory)
/// bp.enable().unwrap();
///
/// // Check if the breakpoint is enabled
/// assert!(bp.is_enabled());
///
/// // Later, disable the breakpoint
/// bp.disable().unwrap();
///
/// assert!(!bp.is_enabled());
/// ```
#[derive(Debug, Clone, Hash)]
pub struct Breakpoint {
    addr: Addr,
    pid: Pid,
    saved_data: Option<u8>,
}

impl Breakpoint {
    /// Creates a new, initially disabled breakpoint at the specified address
    ///
    /// # Parameters
    ///
    /// * `pid` - Process ID of the target process
    /// * `addr` - Address where the breakpoint should be set
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::breakpoint::Breakpoint;
    /// use coreminer::addr::Addr;
    /// use nix::unistd::Pid;
    ///
    /// // Create a new breakpoint at address 0x000055dd73ea3fb8 for process with PID 1234
    /// let mut bp = Breakpoint::new(Pid::from_raw(1234), Addr::from(0x000055dd73ea3fb8usize));
    /// assert!(!bp.is_enabled());
    /// ```
    #[must_use]
    pub fn new(pid: Pid, addr: Addr) -> Self {
        Self {
            pid,
            addr,
            saved_data: None,
        }
    }

    /// Checks if the breakpoint is currently enabled
    ///
    /// # Returns
    ///
    /// * `true` if the breakpoint is enabled (INT3 instruction is in place)
    /// * `false` if the breakpoint is disabled (original instruction is in place)
    ///
    #[inline]
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.saved_data.is_some()
    }

    /// Enables the breakpoint by replacing the original instruction with INT3
    ///
    /// This function:
    /// 1. Reads the current instruction byte from memory
    /// 2. Saves the original byte
    /// 3. Writes an INT3 instruction (0xCC) to the target address
    ///
    /// # Errors
    ///
    /// Will return [`DebuggerError::BreakpointIsAlreadyEnabled`] if the breakpoint
    /// is already enabled.
    ///
    /// This function can fail if:
    /// - Reading from the [Addr] of the [Breakpoint] failed
    /// - Writing to the [Addr] of the [Breakpoint] failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::breakpoint::Breakpoint;
    /// use coreminer::addr::Addr;
    /// use nix::unistd::Pid;
    ///
    /// let mut bp = Breakpoint::new(Pid::from_raw(1234), Addr::from(0x000055dd73ea3fb8usize));
    /// bp.enable().unwrap();
    /// assert!(bp.is_enabled());
    /// ```
    #[allow(clippy::missing_panics_doc)] // this cant panic
    pub fn enable(&mut self) -> Result<()> {
        if self.is_enabled() {
            return Err(DebuggerError::BreakpointIsAlreadyEnabled);
        }

        let data_word: i64 = mem_read_word(self.pid, self.addr)?;
        trace!("original word: {data_word:016x}");
        self.saved_data = Some((data_word & WORD_MASK) as u8);
        trace!("saved_byte: {:02x}", self.saved_data.as_ref().unwrap());
        let data_word_modified: i64 = (data_word & WORD_MASK_INV) | INT3;
        trace!("modified word: {data_word_modified:016x}");
        mem_write_word(self.pid, self.addr, data_word_modified)?;

        Ok(())
    }

    /// Disables the breakpoint by restoring the original instruction
    ///
    /// This function:
    /// 1. Reads the current word from memory (containing INT3)
    /// 2. Replaces the INT3 byte with the saved original byte
    /// 3. Writes the modified word back to memory
    ///
    /// # Errors
    ///
    /// Will return [`DebuggerError::BreakpointIsAlreadyDisabled`] if the breakpoint
    /// is already disabled.
    ///
    /// This function can fail if:
    /// - Reading from the [Addr] of the [Breakpoint] failed
    /// - Writing to the [Addr] of the [Breakpoint] failed
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::breakpoint::Breakpoint;
    /// use coreminer::addr::Addr;
    /// use nix::unistd::Pid;
    ///
    /// let mut bp = Breakpoint::new(Pid::from_raw(1234), Addr::from(0x000055dd73ea3fb8usize));
    /// bp.enable().unwrap();
    /// assert!(bp.is_enabled());
    ///
    /// bp.disable().unwrap();
    /// assert!(!bp.is_enabled());
    /// ```
    #[allow(clippy::missing_panics_doc)] // this cant panic
    pub fn disable(&mut self) -> Result<()> {
        if !self.is_enabled() {
            return Err(DebuggerError::BreakpointIsAlreadyDisabled);
        }

        let data_word: i64 = mem_read_word(self.pid, self.addr)?;
        trace!("breakpo: {data_word:016x}");
        let data_word_restored: i64 =
            (data_word & WORD_MASK_INV) | i64::from(self.saved_data.unwrap());
        trace!("restore: {data_word_restored:016x}");
        mem_write_word(self.pid, self.addr, data_word_restored)?;
        self.saved_data = None;

        Ok(())
    }

    /// Returns the saved original instruction byte, if the breakpoint is enabled
    ///
    /// # Returns
    ///
    /// * `Some(u8)` containing the original instruction byte if the breakpoint is enabled
    /// * `None` if the breakpoint is disabled (no saved data)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::breakpoint::Breakpoint;
    /// use coreminer::addr::Addr;
    /// use nix::unistd::Pid;
    ///
    /// let mut bp = Breakpoint::new(Pid::from_raw(1234), Addr::from(0x000055dd73ea3fb8usize));
    /// assert_eq!(bp.saved_data(), None);
    ///
    /// bp.enable().unwrap();
    /// assert!(bp.saved_data().is_some());
    /// ```
    #[must_use]
    pub fn saved_data(&self) -> Option<u8> {
        self.saved_data
    }
}

impl Drop for Breakpoint {
    /// Automatically disables the breakpoint when dropped to restore original code
    ///
    /// This ensures that breakpoints don't remain set when they go out of scope,
    /// which would leave the target program in an inconsistent state.
    ///
    /// # Panics
    ///
    /// Panics if the breakpoint cannot be disabled. This should only happen if
    /// the target process is no longer accessible.
    fn drop(&mut self) {
        if self.is_enabled() {
            self.disable()
                .expect("could not disable breakpoint while dropping");
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn test_minus_one_has_this_representaiton() {
        assert_eq!(
            &(-1i64).to_le_bytes(),
            &[0xffu8, 0xffu8, 0xffu8, 0xffu8, 0xffu8, 0xffu8, 0xffu8, 0xffu8,]
        );
    }
}
