use std::rc::Rc;

use gimli::{Attribute, Encoding, EndianRcSlice, NativeEndian, Reader};
use object::{Object, ObjectSection};

use crate::dwarf_parse::GimliReaderThing;
use crate::errors::{DebuggerError, Result};
use crate::Addr;

// the gimli::Reader we use
type GimliRd = EndianRcSlice<NativeEndian>;
pub type GimliLocation = gimli::Location<GimliReaderThing, <GimliReaderThing as Reader>::Offset>;

pub struct CMDebugInfo<'executable> {
    pub object_info: object::File<'executable>,
    pub linedata: addr2line::Context<GimliRd>,
    pub dwarf: gimli::Dwarf<GimliReaderThing>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[non_exhaustive]
pub enum SymbolKind {
    Function,
    CompileUnit,
    Variable,
    Other,
    BaseType,
    Constant,
    Parameter,
    Block,
}

#[derive(Debug, Clone)]
pub struct OwnedSymbol {
    offset: usize,
    name: Option<String>,
    low_addr: Option<Addr>,
    high_addr: Option<Addr>,
    datatype: Option<usize>,
    kind: SymbolKind,
    children: Vec<Self>,
    location: Option<Attribute<GimliReaderThing>>,
    frame_base: Option<Attribute<GimliReaderThing>>,
    byte_size: Option<usize>,
    encoding: gimli::Encoding,
}

impl OwnedSymbol {
    pub fn new(
        code: usize,
        kind: SymbolKind,
        children: &[Self],
        encoding: gimli::Encoding,
    ) -> Self {
        Self {
            offset: code,
            name: None,
            low_addr: None,
            high_addr: None,
            kind,
            datatype: None,
            location: None,
            frame_base: None,
            children: children.to_vec(),
            byte_size: None,
            encoding,
        }
    }

    pub fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    pub fn set_name(&mut self, name: Option<String>) {
        self.name = name;
    }

    pub fn set_low_addr(&mut self, low_addr: Option<Addr>) {
        self.low_addr = low_addr;
    }

    pub fn set_high_addr(&mut self, high_addr: Option<Addr>) {
        self.high_addr = high_addr;
    }

    pub fn set_datatype(&mut self, datatype: Option<usize>) {
        self.datatype = datatype;
    }

    pub fn set_kind(&mut self, kind: SymbolKind) {
        self.kind = kind;
    }

    pub fn set_children(&mut self, children: Vec<Self>) {
        self.children = children;
    }

    pub fn set_location(&mut self, location: Option<Attribute<GimliReaderThing>>) {
        self.location = location;
    }

    pub fn set_frame_base(&mut self, frame_base: Option<Attribute<GimliReaderThing>>) {
        self.frame_base = frame_base;
    }

    pub fn set_byte_size(&mut self, byte_size: Option<usize>) {
        self.byte_size = byte_size;
    }

    pub fn set_encoding(&mut self, encoding: gimli::Encoding) {
        self.encoding = encoding;
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn low_addr(&self) -> Option<Addr> {
        self.low_addr
    }

    pub fn high_addr(&self) -> Option<Addr> {
        self.high_addr
    }

    pub fn datatype(&self) -> Option<usize> {
        self.datatype
    }

    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn children(&self) -> &[OwnedSymbol] {
        &self.children
    }

    pub fn location(&self) -> Option<&Attribute<GimliReaderThing>> {
        self.location.as_ref()
    }

    pub fn frame_base(&self) -> Option<&Attribute<GimliReaderThing>> {
        self.frame_base.as_ref()
    }

    pub fn byte_size(&self) -> Option<usize> {
        self.byte_size
    }

    pub fn encoding(&self) -> Encoding {
        self.encoding
    }
}

impl<'executable> CMDebugInfo<'executable> {
    pub fn build(object_info: object::File<'executable>) -> Result<Self> {
        let loader = |section: gimli::SectionId| -> std::result::Result<_, ()> {
            // does never fail surely
            let data = object_info
                .section_by_name(section.name())
                .map(|s| s.uncompressed_data().unwrap_or_default());

            Ok(GimliRd::new(
                Rc::from(data.unwrap_or_default().as_ref()),
                gimli::NativeEndian,
            ))
        };
        let dwarf = gimli::Dwarf::load(loader).unwrap();
        let dwarf2 = gimli::Dwarf::load(loader).unwrap();

        let linedata = addr2line::Context::from_dwarf(dwarf2)?;

        Ok(CMDebugInfo {
            object_info,
            linedata,
            dwarf,
        })
    }
}

impl TryFrom<gimli::DwTag> for SymbolKind {
    type Error = DebuggerError;
    fn try_from(value: gimli::DwTag) -> std::result::Result<Self, Self::Error> {
        Ok(match value {
            gimli::DW_TAG_compile_unit => SymbolKind::CompileUnit,
            gimli::DW_TAG_subprogram => SymbolKind::Function,
            gimli::DW_TAG_variable => SymbolKind::Variable,
            gimli::DW_TAG_constant => SymbolKind::Constant,
            gimli::DW_TAG_formal_parameter => SymbolKind::Parameter,
            gimli::DW_TAG_base_type => SymbolKind::BaseType,
            gimli::DW_TAG_try_block
            | gimli::DW_TAG_catch_block
            | gimli::DW_TAG_lexical_block
            | gimli::DW_TAG_common_block => SymbolKind::Block,
            _ => SymbolKind::Other,
        })
    }
}
