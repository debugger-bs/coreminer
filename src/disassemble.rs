use crate::errors::Result;
use crate::Addr;

const CODE_BITNESS: u32 = 64;

use iced_x86::{
    Decoder, DecoderOptions, Formatter, FormatterOutput, FormatterTextKind, NasmFormatter,
};

type TextContent = (String, FormatterTextKind);

struct DisassemblyOutput(Vec<TextContent>);

// Custom formatter output that stores the output in a vector.
#[derive(Debug, Clone, Hash)]
pub struct Disassembly {
    vec: Vec<(Addr, Vec<TextContent>)>,
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
    pub fn empty() -> Self {
        Self { vec: Vec::new() }
    }

    pub fn disassemble(data: &[u8], first_addr: Addr) -> Result<Self> {
        let decoder = Decoder::with_ip(CODE_BITNESS, data, first_addr.into(), DecoderOptions::NONE);
        let mut formatter = NasmFormatter::new();

        // padding
        formatter.options_mut().set_first_operand_char_index(16);

        // numbers stuff
        formatter.options_mut().set_hex_suffix("");
        formatter.options_mut().set_hex_prefix("");
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
        let mut text_contents: DisassemblyOutput = DisassemblyOutput::new();
        for instruction in decoder {
            text_contents.clear();
            formatter.format(&instruction, &mut text_contents);
            disassembly.write_to_line(instruction.ip().into(), text_contents.inner());
        }

        Ok(disassembly)
    }

    pub fn inner(&self) -> &[(Addr, Vec<TextContent>)] {
        &self.vec
    }

    pub fn inner_mut(&mut self) -> &mut Vec<(Addr, Vec<TextContent>)> {
        &mut self.vec
    }

    pub fn has_entry_for(&self, addr: Addr) -> bool {
        self.vec.iter().any(|(a, _val)| *a == addr)
    }

    pub fn write_to_line(&mut self, addr: Addr, content: &[TextContent]) {
        if self.has_entry_for(addr) {
            panic!("tried to insert line which was already disassembled")
        }
        self.vec.push((addr, content.to_vec()));
    }
}

impl FormatterOutput for DisassemblyOutput {
    fn write(&mut self, text: &str, kind: FormatterTextKind) {
        self.0.push((text.to_string(), kind));
    }
}
