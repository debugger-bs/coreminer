use std::fmt::Display;

use crate::{Addr, Word, WORD_BYTES};

#[derive(Clone, Debug)]
pub struct Stack {
    start_addr: Addr,
    words: Vec<Word>,
}

impl Stack {
    pub fn new(start_addr: Addr) -> Self {
        Self {
            start_addr,
            words: Vec::new(),
        }
    }
    pub fn push(&mut self, word: Word) {
        self.words.push(word);
    }
    pub fn pop(&mut self) -> Option<Word> {
        self.words.pop()
    }

    pub fn words(&self) -> &[i64] {
        &self.words
    }
}

impl Display for Stack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (idx, w) in self.words().iter().enumerate() {
            writeln!(
                f,
                "{:<24}\t{:018x}",
                self.start_addr + (idx * WORD_BYTES),
                w
            )?;
        }
        Ok(())
    }
}
