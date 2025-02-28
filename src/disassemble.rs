//! # Disassembly Module
//!
//! Provides functionality for disassembling machine code into human-readable assembly instructions.
//!
//! This module contains the [`Disassembly`] struct and related functionality for converting
//! raw machine code bytes and some metadata into a structured representation of assembly instructions. It leverages
//! the iced-x86 library for the actual disassembly work and adds features specific to debugging,
//! such as tracking which instructions have breakpoints.
//!
//! Key features of this module include:
//!
//! - Disassembling a range of memory into instructions
//! - Tracking which instructions have breakpoints set
//! - Formatting disassembly for display

use std::fmt::{Display, Write};

use crate::errors::Result;
use crate::Addr;

const CODE_BITNESS: u32 = 64;

use iced_x86::{
    Decoder, DecoderOptions, Formatter, FormatterOutput, FormatterTextKind, Instruction,
    NasmFormatter,
};
use serde::{Serialize, Serializer};

/// Type alias for text content in disassembled code
///
/// Represents a piece of text and its kind (e.g., mnemonic, register, number)
/// in the disassembled instruction.
///
/// This can later be used by a [crate::ui::DebuggerUI] to color different parts of the
/// disassembly.
pub type TextContent = (String, FormatterTextKind);

#[derive(Serialize)]
struct SerializableTextContent {
    text: String,
    kind: String,
}

/// Custom output container for the disassembly formatter
///
/// This struct collects the formatted text pieces produced by the iced-x86
/// formatter during disassembly.
struct DisassemblyOutput(Vec<TextContent>);

/// Represents the result of disassembling a section of memory
///
/// [`Disassembly`] contains a structured representation of disassembled instructions,
/// including the address, raw bytes, formatted text content, and whether each
/// instruction has a breakpoint set.
///
/// This is best used from either [crate::debugger::Debugger::disassemble_at] or
/// [crate::debuggee::Debuggee::disassemble].
///
/// # Examples
///
/// ```
/// use std::fmt::Write;
/// use coreminer::disassemble::Disassembly;
/// use coreminer::addr::Addr;
/// use coreminer::debuggee::Debuggee;
/// use coreminer::breakpoint::INT3_BYTE;
///
/// let data_raw = vec![
///  0x48, 0x83, 0xec, 0x08,
///  0x48, 0x8b, 0x05, 0xbd, 0x1f, 0x02, 0x00,
///  0x48, 0x85, 0xc0,
///  0x74, 0x02,
///  0xff, 0xd0,
///  0x48, 0x83, 0xc4, 0x08,
///  0xc3,
///  0x00,0x00,
///  0x00,0x00,
///  0x00,0x00,
///  0x00,0x00,
///  0x00,                 
/// ];
/// let bp_indexes = vec![22, 16, 18];
/// let addr = Addr::from(0x000055dd73ea200busize);
/// let disassembly: Disassembly = Disassembly::disassemble(
///     &data_raw, addr, &bp_indexes).unwrap();
///
/// // Print the disassembly
/// println!("{}", disassembly);
///
/// // Iterate through individual instructions
/// // a simplified form
/// for (addr, raw_bytes, content, has_bp) in disassembly.inner() {
///     println!("{:<20}: {:<30} {} {}",
///         addr,
///         format!("{:02x?}", raw_bytes),
///         if *has_bp { "*" } else { " " },
///         content[0].0
///     );
/// }
///
/// println!();
///
/// // a more complicated form that prints more information
/// for (addr, raw, content, has_bp) in disassembly.inner() {
///     let mut buf = String::new();
///     print!("{addr}");
///     if *has_bp {
///         print!("{:<4}", "(*)");
///     } else {
///         print!("{:<4}", "");
///     }
///     for byte in raw {
///         write!(buf, "{byte:02x} ").unwrap();
///     }
///     print!("{:<20}\t", buf);
///     buf.clear();
///     print!("{:<20}\t", buf);
///     for (thing, _kind) in content {
///         print!("{thing}");
///     }
///     println!();
/// }
/// ```
#[derive(Debug, Clone, Hash, Serialize)]
pub struct Disassembly {
    // addres, raw data, interpreted data for display, is it a breakpoint?
    #[serde(serialize_with = "serialize_disassembly_vec")]
    vec: Vec<(Addr, Vec<u8>, Vec<TextContent>, bool)>,
}

impl DisassemblyOutput {
    fn new() -> Self {
        DisassemblyOutput(Vec::new())
    }
    fn inner(&mut self) -> &[TextContent] {
        &self.0
    }
    fn clear(&mut self) {
        self.0.clear();
    }
}

impl Disassembly {
    /// Creates a new empty disassembly
    ///
    /// # Returns
    ///
    /// A new empty [`Disassembly`] instance
    pub fn empty() -> Self {
        Self { vec: Vec::new() }
    }

    /// Disassembles a section of memory
    ///
    /// # Parameters
    ///
    /// * `data` - The raw memory bytes to disassemble
    /// * `first_addr` - The starting address of the memory section
    /// * `bp_indexes` - Indexes of bytes that have breakpoints set
    ///
    /// # Returns
    ///
    /// * `Ok(Disassembly)` - The disassembled code
    /// * `Err(DebuggerError)` - If disassembly failed
    ///
    /// # Errors
    ///
    /// This function can fail if the iced-x86 library encounters an error
    /// during disassembly.
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::disassemble::Disassembly;
    /// use coreminer::addr::Addr;
    ///
    /// // Disassemble some x86-64 code
    /// let code = [
    ///     0x48, 0x89, 0xe5,                           // this instruction has a bp
    ///     0x48, 0x83, 0xec, 0x20,                     // this one too
    ///     0x48, 0x8b, 0x05, 0xb8, 0x13, 0x00, 0x00    // this instruction has no bp
    /// ];
    /// let addr = Addr::from(0x000055dd73ea200busize);
    /// let breakpoints = vec![3,0];
    ///
    /// let disasm = Disassembly::disassemble(&code, addr, &breakpoints).unwrap();
    /// println!("{}", disasm);
    /// ```
    pub fn disassemble(data: &[u8], first_addr: Addr, bp_indexes: &[usize]) -> Result<Self> {
        let mut decoder =
            Decoder::with_ip(CODE_BITNESS, data, first_addr.into(), DecoderOptions::NONE);
        let mut formatter = NasmFormatter::new();

        // padding
        formatter.options_mut().set_first_operand_char_index(16);

        // numbers stuff
        formatter.options_mut().set_hex_suffix("");
        formatter.options_mut().set_hex_prefix("");
        formatter.options_mut().set_uppercase_hex(false);
        formatter.options_mut().set_decimal_suffix("");
        formatter.options_mut().set_decimal_prefix("0d");
        formatter.options_mut().set_octal_suffix("");
        formatter.options_mut().set_octal_prefix("0o");
        formatter.options_mut().set_binary_suffix("");
        formatter.options_mut().set_binary_prefix("0b");

        // memory stuff
        formatter.options_mut().set_show_symbol_address(true);
        formatter.options_mut().set_rip_relative_addresses(false);
        formatter
            .options_mut()
            .set_memory_size_options(iced_x86::MemorySizeOptions::Always);

        let mut disassembly = Self::empty();
        let mut instruction = Instruction::default();
        let mut text_contents: DisassemblyOutput = DisassemblyOutput::new();
        while decoder.can_decode() {
            decoder.decode_out(&mut instruction);
            text_contents.clear();
            formatter.format(&instruction, &mut text_contents);

            let start_index = (instruction.ip() - Into::<u64>::into(first_addr)) as usize;
            let instr_bytes = &data[start_index..start_index + instruction.len()];

            disassembly.write_to_line(
                instruction.ip().into(),
                instr_bytes,
                text_contents.inner(),
                bp_indexes.contains(&(instruction.ip() as usize - first_addr.usize())),
            );
        }

        Ok(disassembly)
    }

    /// Returns a reference to the inner data of this disassembly
    ///
    /// # Returns
    ///
    /// A slice containing tuples of (address, raw bytes, text content, has breakpoint?)
    /// for each disassembled instruction.
    pub fn inner(&self) -> &[(Addr, Vec<u8>, Vec<TextContent>, bool)] {
        &self.vec
    }

    /// Returns a mutable reference to the inner data of this disassembly
    ///
    /// # Returns
    ///
    /// A mutable vector containing tuples of (address, raw bytes, text content, has breakpoint?)
    /// for each disassembled instruction.
    pub fn inner_mut(&mut self) -> &mut Vec<(Addr, Vec<u8>, Vec<TextContent>, bool)> {
        &mut self.vec
    }

    /// Checks if this disassembly has an entry for the given address
    ///
    /// # Parameters
    ///
    /// * `addr` - The address to check
    ///
    /// # Returns
    ///
    /// `true` if the disassembly contains an instruction at the given address,
    /// `false` otherwise.
    pub fn has_entry_for(&self, addr: Addr) -> bool {
        self.vec.iter().any(|(a, _raw, _val, _bp)| *a == addr)
    }

    /// Adds a disassembled instruction to this disassembly
    ///
    /// # Parameters
    ///
    /// * `addr` - The address of the instruction
    /// * `raw` - The raw bytes of the instruction
    /// * `content` - The formatted text content of the instruction
    /// * `has_bp` - Whether the instruction has a breakpoint set
    ///
    /// # Panics
    ///
    /// This function will panic if an instruction at the given address already exists
    /// in the disassembly.
    pub fn write_to_line(&mut self, addr: Addr, raw: &[u8], content: &[TextContent], has_bp: bool) {
        if self.has_entry_for(addr) {
            panic!("tried to insert line which was already disassembled")
        }
        self.vec
            .push((addr, raw.to_vec(), content.to_vec(), has_bp));
    }
}

impl FormatterOutput for DisassemblyOutput {
    /// Writes a piece of text with its kind to this output
    ///
    /// This function is called by the iced-x86 formatter to add pieces of
    /// formatted text to the output.
    ///
    /// # Parameters
    ///
    /// * `text` - The text to add
    /// * `kind` - The kind of text (e.g., mnemonic, register, number)
    fn write(&mut self, text: &str, kind: FormatterTextKind) {
        self.0.push((text.to_string(), kind));
    }
}

impl Display for Disassembly {
    /// Formats the Disassembly into a fancy [String]
    ///
    /// # Example
    ///
    /// Will look like this:
    ///
    /// ```text
    /// 0x000055dd73ea2000    48 83 ec 08               sub             rsp,0d8
    /// 0x000055dd73ea2004    48 8b 05 bd 1f 02 00      mov             rax,qword [rel 55dd73ec3fc8]
    /// 0x000055dd73ea200b    48 85 c0                  test            rax,rax
    /// 0x000055dd73ea200e    74 02                     je              short 000055dd73ea2012
    /// 0x000055dd73ea2010    ff d0                     call            rax
    /// 0x000055dd73ea2012    48 83 c4 08               add             rsp,0d8
    /// 0x000055dd73ea2016    c3                        ret
    /// 0x000055dd73ea2017    00 00                     add             byte [rax],al
    /// 0x000055dd73ea2019    00 00                     add             byte [rax],al
    /// 0x000055dd73ea201b    00 00                     add             byte [rax],al
    /// 0x000055dd73ea201d    00 00                     add             byte [rax],al
    /// ```
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut buf2 = String::new();
        for (addr, raw, content, has_bp) in self.inner() {
            write!(f, "{addr}")?;
            for byte in raw {
                write!(buf2, "{byte:02x} ")?;
            }
            if *has_bp {
                write!(f, "{:<4}", "(*)")?;
            } else {
                write!(f, "{:<4}", "")?;
            }
            write!(f, "{:<20}\t", buf2)?;
            buf2.clear();
            for (thing, _kind) in content {
                write!(f, "{thing}")?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

impl From<&TextContent> for SerializableTextContent {
    fn from(content: &TextContent) -> Self {
        Self {
            text: content.0.clone(),
            kind: format!("{:?}", content.1),
        }
    }
}

fn serialize_disassembly_vec<S>(
    data: &Vec<(Addr, Vec<u8>, Vec<TextContent>, bool)>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let serializable_data: Vec<(Addr, Vec<u8>, Vec<SerializableTextContent>, bool)> = data
        .iter()
        .map(|(addr, raw, content, has_bp)| {
            (
                *addr,
                raw.clone(),
                content.iter().map(SerializableTextContent::from).collect(),
                *has_bp,
            )
        })
        .collect();

    serializable_data.serialize(serializer)
}
