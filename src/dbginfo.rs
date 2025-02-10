use std::rc::Rc;

use gimli::{EndianRcSlice, NativeEndian};
use object::{Object, ObjectSection};

use crate::dwarf_parse::GimliReaderThing;
use crate::errors::{DebuggerError, Result};
use crate::Addr;

// the gimli::Reader we use
type GimliRd = EndianRcSlice<NativeEndian>;

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

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct OwnedSymbol {
    pub name: Option<String>,
    pub low_addr: Option<Addr>,
    pub high_addr: Option<Addr>,
    pub kind: SymbolKind,
    pub children: Vec<Self>,
}

impl OwnedSymbol {
    pub fn new(
        name: Option<impl ToString>,
        low_addr: Option<Addr>,
        high_addr: Option<Addr>,
        kind: SymbolKind,
        children: &[Self],
    ) -> Self {
        Self {
            name: name.map(|name| name.to_string()),
            low_addr,
            high_addr,
            kind,
            children: children.to_vec(),
        }
    }

    pub fn kind(&self) -> SymbolKind {
        self.kind
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

    pub fn children(&self) -> &[OwnedSymbol] {
        &self.children
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
