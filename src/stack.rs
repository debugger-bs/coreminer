//! # Stack Module
//!
//! Provides functionality for representing and manipulating the debuggee's stack.
//!
//! This module contains the [`Stack`] struct, which represents the current state
//! of a debuggee's stack memory. It allows for inspection of stack contents and
//! provides methods for navigating and manipulating the stack during debugging.
//!
//! The stack is a crucial part of understanding program execution, as it contains
//! local variables, function call information, and other runtime data.

use std::fmt::Display;

use serde::Serialize;

use crate::{Addr, Word, WORD_BYTES};

/// Represents the stack of a debugged process
///
/// [`Stack`] encapsulates the current state of a debuggee's stack, including
/// the starting address and the sequence of words stored on the stack.
///
/// # Examples
///
/// ```
/// use coreminer::stack::Stack;
/// use coreminer::addr::Addr;
///
/// // Create a new stack starting at address 0x7fffffffe000
/// let mut stack = Stack::new(Addr::from(0x7fffffffe000usize));
///
/// // Push some values onto the stack
/// stack.push(0x123456789);
/// stack.push(0x42);
///
/// // Pop a value from the stack
/// let value = stack.pop();
/// assert_eq!(value, Some(0x42));
///
/// // Access all values on the stack
/// for word in stack.words() {
///     println!("{:#x}", word);
/// }
/// ```
#[derive(Clone, Debug, Serialize)]
pub struct Stack {
    start_addr: Addr,
    words: Vec<Word>,
}

impl Stack {
    /// Creates a new empty stack with the specified starting address
    ///
    /// # Parameters
    ///
    /// * `start_addr` - The starting address of the stack
    ///
    /// # Returns
    ///
    /// A new empty [`Stack`] instance with the specified starting address
    pub fn new(start_addr: Addr) -> Self {
        Self {
            start_addr,
            words: Vec::new(),
        }
    }

    /// Pushes a word onto the stack
    ///
    /// # Parameters
    ///
    /// * `word` - The word to push onto the stack
    pub fn push(&mut self, word: Word) {
        self.words.push(word);
    }

    /// Pops a word from the stack
    ///
    /// # Returns
    ///
    /// * `Some(Word)` - The popped word, if the stack is not empty
    /// * `None` - If the stack is empty
    pub fn pop(&mut self) -> Option<Word> {
        self.words.pop()
    }

    /// Gets all words stored on the stack
    ///
    /// # Returns
    ///
    /// A slice containing all words stored on the stack
    pub fn words(&self) -> &[Word] {
        &self.words
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (idx, w) in self.words().iter().enumerate() {
            writeln!(
                f,
                "{:<24}\t{:016x}",
                self.start_addr - (idx * WORD_BYTES),
                w
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_stack_operations() {
        let mut stack = Stack::new(Addr::from(1000usize));
        stack.push(42);
        stack.push(43);
        assert_eq!(stack.pop(), Some(43));
        assert_eq!(stack.words(), &[42]);
    }
}
