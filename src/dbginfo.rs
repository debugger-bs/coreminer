use std::rc::Rc;

use gimli::{DW_TAG_compile_unit, DW_TAG_subprogram, EndianRcSlice, EndianReader, NativeEndian};
use object::{Object, ObjectSection};

use crate::errors::{DebuggerError, Result};
use crate::Addr;

// the gimli::Reader we use
type GimliRd = EndianRcSlice<NativeEndian>;

pub struct CMDebugInfo<'executable> {
    pub object_info: object::File<'executable>,
    pub linedata: addr2line::Context<GimliRd>,
    pub dwarf: gimli::Dwarf<EndianReader<NativeEndian, Rc<[u8]>>>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum SymbolKind {
    Function,
    CompileUnit,
}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct OwnedSymbol {
    pub name: String,
    pub low_addr: Addr,
    pub high_addr: Addr,
    pub kind: SymbolKind,
    pub children: Vec<OwnedSymbol>,
}

impl OwnedSymbol {
    pub fn new(
        name: impl ToString,
        low_addr: Addr,
        high_addr: Addr,
        kind: SymbolKind,
        children: &[Self],
    ) -> Self {
        Self {
            name: name.to_string(),
            low_addr,
            high_addr,
            kind,
            children: children.to_vec(),
        }
    }

    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn high_addr(&self) -> Addr {
        self.high_addr
    }

    pub fn low_addr(&self) -> Addr {
        self.low_addr
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn children(&self) -> &[Self] {
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
            DW_TAG_compile_unit => SymbolKind::CompileUnit,
            DW_TAG_subprogram => SymbolKind::Function,
            _ => return Err(DebuggerError::DwTagNotImplemented(value)),
        })
    }
}
