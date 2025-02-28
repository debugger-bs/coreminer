//! # Address Module
//!
//! Provides the core [`Addr`] type and related utilities for memory address manipulation in the debugger.
//!
//! This module encapsulates the representation of memory addresses, providing type safety and
//! convenience methods for working with memory addresses in various contexts. The primary type
//! is [`Addr`], which represents a memory address and includes operations for address arithmetic,
//! conversions, and formatting.
//!
//! ## Key Features
//!
//! - Type-safe address representation
//! - Address arithmetic operations
//! - Conversions between addresses and various numeric types
//! - Relative address calculations
//! - Debug and display formatting

use std::fmt::Display;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use serde::Serialize;

use crate::Word;

/// Raw pointer type used for interoperating with C functions
pub type RawPointer = *mut std::ffi::c_void;

/// Represents a memory address with type safety and convenience methods
///
/// [`Addr`] encapsulates a memory address as a [`usize`] and provides various
/// operations for address arithmetic, conversions, and formatted output.
///
/// # Examples
///
/// ```
/// use coreminer::addr::Addr;
///
/// // Create an address from a usize
/// let addr = Addr::from(0x1000usize);
///
/// // Perform address arithmetic
/// let offset_addr = addr + 0x100;
/// assert_eq!(offset_addr.usize(), 0x1100);
///
/// // Format the address for display
/// assert_eq!(format!("{}", addr), "0x0000000000001000");
/// ```
#[derive(Hash, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Addr(usize);

impl Addr {
    /// Returns the address as a `usize` value
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::addr::Addr;
    ///
    /// let addr = Addr::from(0x1234usize);
    /// assert_eq!(addr.usize(), 0x1234);
    /// ```
    pub fn usize(&self) -> usize {
        self.0
    }
    /// Returns the address as a `u64` value
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::addr::Addr;
    ///
    /// let addr = Addr::from(0x1234usize);
    /// assert_eq!(addr.u64(), 0x1234u64);
    /// ```
    pub fn u64(&self) -> u64 {
        self.0 as u64
    }
    /// Returns the address as a raw pointer
    ///
    /// This is useful for interoperating with C functions that expect a raw pointer.
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::addr::Addr;
    ///
    /// let addr = Addr::from(0x1234usize);
    /// let ptr = addr.raw_pointer();
    /// // Use ptr with FFI functions
    /// ```
    pub fn raw_pointer(&self) -> RawPointer {
        self.0 as RawPointer
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#018x}", { self.0 })
    }
}

impl Add for Addr {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<usize> for Addr {
    type Output = Self;
    fn add(self, rhs: usize) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign for Addr {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl AddAssign<usize> for Addr {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl SubAssign for Addr {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0
    }
}

impl SubAssign<usize> for Addr {
    fn sub_assign(&mut self, rhs: usize) {
        self.0 -= rhs
    }
}

impl Sub for Addr {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Sub<usize> for Addr {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl From<RawPointer> for Addr {
    fn from(value: RawPointer) -> Self {
        Addr(value as usize)
    }
}

impl From<Addr> for RawPointer {
    fn from(value: Addr) -> Self {
        value.0 as RawPointer
    }
}

impl From<usize> for Addr {
    fn from(value: usize) -> Self {
        Addr(value)
    }
}

impl From<Word> for Addr {
    fn from(value: Word) -> Self {
        Addr(value as usize)
    }
}

impl From<u64> for Addr {
    fn from(value: u64) -> Self {
        Addr(value as usize)
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

impl std::fmt::Debug for Addr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:#018x}", self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_addr_arithmetic() {
        let a = Addr::from(100usize);
        let b = Addr::from(50usize);
        assert_eq!((a + b).usize(), 150);
        assert_eq!((a - b).usize(), 50);
    }

    #[test]
    fn test_addr_conversions() {
        let a = Addr::from(0x1234usize);
        assert_eq!(a.u64(), 0x1234u64);
        assert_eq!(format!("{}", a), "0x0000000000001234");
    }
}
