//! # Debug Information Module
//!
//! Provides structures and functionality for parsing and handling ELF and DWARF information.
//!
//! This module contains types and functions for working with debug information extracted from
//! executable files. It includes representations of debug symbols, compilation units, and
//! mechanisms to search through debug symbols. The debug information is essential for
//! setting memory addresses into context, inspecting variables, and
//! understanding program structure at runtime.

use std::fmt::Debug;
use std::rc::Rc;

use gimli::{Attribute, Encoding, EndianRcSlice, NativeEndian, Reader};
use object::{Object, ObjectSection};
use serde::Serialize;

use crate::dwarf_parse::GimliReaderThing;
use crate::errors::{DebuggerError, Result};
use crate::Addr;

/// Type alias for the Gimli reader used for DWARF information parsing
type GimliRd = EndianRcSlice<NativeEndian>;
/// Type alias for a location in the DWARF debug information
pub type GimliLocation = gimli::Location<GimliReaderThing, <GimliReaderThing as Reader>::Offset>;

/// Represents debug information extracted from an executable
///
/// This struct holds the parsed DWARF debug information along with the
/// original object file information, providing access to symbols, source
/// locations, and other debug data.
pub struct CMDebugInfo<'executable> {
    /// The original object file information
    pub object_info: object::File<'executable>,
    /// Line number information for address-to-line mapping
    pub linedata: addr2line::Context<GimliRd>,
    /// The parsed DWARF debug information
    pub dwarf: gimli::Dwarf<GimliReaderThing>,
}

/// Categorizes different types of symbols found in debug information
///
/// This enum represents the various kinds of symbols that can be found in
/// DWARF debug information, such as functions, variables, types, etc.
///
/// This is not an exhaustive enumeration and merely focuses on the most
/// important kinds for this debugger. Uncovered types are treated as [SymbolKind::Other].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub enum SymbolKind {
    /// A function or method
    Function,

    /// A compilation unit (typically a source file)
    CompileUnit,

    /// A variable with storage
    Variable,

    /// A symbol type not explicitly covered by other variants
    Other,

    /// A basic data type (int, float, etc.)
    BaseType,

    /// A constant value
    Constant,

    /// A function parameter
    Parameter,

    /// A lexical scope block
    Block,
}

/// Represents a parsed symbol from the debug information
///
/// [`OwnedSymbol`] contains information about a symbol extracted from DWARF debug
/// information, including its name, address range, type information, and children
/// symbols that are in its scope.
///
/// Note that `encoding`, `frame_base` and `location` are skipped when serializing this with
/// [`serde`], as they are [`gimli`] datatypes.
#[derive(Clone, Serialize)]
pub struct OwnedSymbol {
    offset: usize,
    name: Option<String>,
    low_addr: Option<Addr>,
    high_addr: Option<Addr>,
    datatype: Option<usize>,
    kind: SymbolKind,
    children: Vec<Self>,
    #[serde(skip)]
    location: Option<Attribute<GimliReaderThing>>,
    #[serde(skip)]
    frame_base: Option<Attribute<GimliReaderThing>>,
    byte_size: Option<usize>,
    #[serde(skip)]
    encoding: gimli::Encoding,
}

impl OwnedSymbol {
    /// Creates a new symbol with the given parameters
    ///
    /// The parameters of this function only include the necessary parameters. Optional fields like
    /// the datatype can be set with the respective functions, e.g. [Self::set_datatype].
    ///
    /// # Parameters
    ///
    /// * `code` - The DWARF offset of the symbol
    /// * `kind` - The kind of symbol
    /// * `children` - Child symbols within this symbol's scope
    /// * `encoding` - The DWARF encoding information
    ///
    /// The many optional parameters are available, e.g. with [OwnedSymbol::name] and
    /// [OwnedSymbol::set_name].
    ///
    /// # Returns
    ///
    /// A new `OwnedSymbol` instance
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
    /// Creates a new debug information instance from an object file
    ///
    /// This function parses the DWARF debug information from the provided object file
    /// and creates a structured representation that can be used for debugging operations.
    ///
    /// If the binary is stripped, this will still work, but the DWARF information will be empty,
    /// meaning that no symbols will exist.
    ///
    /// # Parameters
    ///
    /// * `object_info` - The object file containing debug information
    ///
    /// # Returns
    ///
    /// * `Ok(CMDebugInfo)` - The parsed debug information
    /// * `Err(DebuggerError)` - If the debug information could not be parsed
    ///
    /// # Errors
    ///
    /// This function will return an error if the DWARF information in the object file
    /// is invalid or cannot be parsed.
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

impl Debug for OwnedSymbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedSymbol")
            .field("offset", &self.offset)
            .field("kind", &self.kind)
            .field("name", &self.name)
            .field("low_addr", &self.low_addr)
            .field("high_addr", &self.high_addr)
            .field("datatype", &self.datatype)
            .field(
                "location",
                &format_args!("{}", &dbg_large_option(self.location())),
            )
            .field(
                "frame_base",
                &format_args!("{}", &dbg_large_option(self.frame_base())),
            )
            .field("byte_size", &self.byte_size)
            .field("children", &self.children)
            .finish()
    }
}

/// Searches for [symbols](OwnedSymbol) matching a predicate in a symbol hierarchy
///
/// This function recursively traverses a symbol tree and collects all symbols
/// that match the provided filter function.
///
/// The symbols are organized in a tree. At the start, are the compilation units, which have all
/// functions, globals, and many more as children. Functions have their variables as children, and
/// so on. To search through all symbols, this function recursively considers not only the list of
/// given symbols, but the children too, and the children of all children.
///
/// The relevant children get cloned.
///
/// # Parameters
///
/// * `haystack` - The symbols to search through
/// * `fil` - A filter function that returns true for symbols to include
///
/// # Returns
///
/// A vector of symbols that match the filter function
///
/// # Examples
///
/// ```no_run
/// use coreminer::dbginfo::{search_through_symbols, OwnedSymbol};
///
/// fn find_main_function(symbols: &[OwnedSymbol]) -> Vec<OwnedSymbol> {
///     search_through_symbols(symbols, |sym| sym.name() == Some("main"))
/// }
/// ```
pub fn search_through_symbols<F>(haystack: &[OwnedSymbol], fil: F) -> Vec<OwnedSymbol>
where
    F: Fn(&OwnedSymbol) -> bool,
{
    let mut relevant = Vec::new();

    fn finder<F>(buf: &mut Vec<OwnedSymbol>, s: &OwnedSymbol, fil: &F)
    where
        F: Fn(&OwnedSymbol) -> bool,
    {
        for c in s.children() {
            finder(buf, c, fil);
        }
        if fil(s) {
            buf.push(s.clone());
        }
    }

    for s in haystack {
        finder(&mut relevant, s, &fil);
    }

    relevant
}

/// Helper function to format large option values for debug output
///
/// This function returns a string representation of an option that avoids
/// verbose output for large contained values.
fn dbg_large_option<T>(o: Option<T>) -> &'static str {
    match o {
        Some(_inner) => "Some(...)",
        None => "None",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_encoding() -> gimli::Encoding {
        gimli::Encoding {
            format: gimli::Format::Dwarf32,
            version: 4,
            address_size: 8,
        }
    }

    #[test]
    fn test_owned_symbol_basic() {
        let encoding = test_encoding();
        let sym = OwnedSymbol::new(42, SymbolKind::Function, &[], encoding);

        assert_eq!(sym.offset(), 42);
        assert_eq!(sym.kind(), SymbolKind::Function);
        assert_eq!(sym.name(), None);
        assert_eq!(sym.children().len(), 0);
        assert_eq!(sym.encoding(), encoding);
    }

    #[test]
    fn test_owned_symbol_setters() {
        let mut sym = OwnedSymbol::new(0, SymbolKind::Variable, &[], test_encoding());

        sym.set_name(Some("test_var".to_string()));
        sym.set_datatype(Some(123));
        sym.set_byte_size(Some(4));
        sym.set_low_addr(Some(Addr::from(0x1000usize)));
        sym.set_high_addr(Some(Addr::from(0x1100usize)));

        assert_eq!(sym.name(), Some("test_var"));
        assert_eq!(sym.datatype(), Some(123));
        assert_eq!(sym.byte_size(), Some(4));
        assert_eq!(sym.low_addr(), Some(Addr::from(0x1000usize)));
        assert_eq!(sym.high_addr(), Some(Addr::from(0x1100usize)));
    }

    // Test SymbolKind conversion from DwTag
    #[test]
    fn test_symbol_kind_from_dwtag() {
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_subprogram).unwrap(),
            SymbolKind::Function
        );
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_variable).unwrap(),
            SymbolKind::Variable
        );
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_base_type).unwrap(),
            SymbolKind::BaseType
        );
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_formal_parameter).unwrap(),
            SymbolKind::Parameter
        );
        // Test blocks are grouped correctly
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_try_block).unwrap(),
            SymbolKind::Block
        );
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_lexical_block).unwrap(),
            SymbolKind::Block
        );
        // Test unknown tag becomes Other
        assert_eq!(
            SymbolKind::try_from(gimli::DW_TAG_array_type).unwrap(),
            SymbolKind::Other
        );
    }

    #[test]
    fn test_search_through_symbols() {
        let encoding = test_encoding();
        let child1 = OwnedSymbol::new(1, SymbolKind::Variable, &[], encoding);
        let child2 = {
            let mut sym = OwnedSymbol::new(2, SymbolKind::Function, &[], encoding);
            sym.set_name(Some("target".to_string()));
            sym
        };
        let parent = OwnedSymbol::new(0, SymbolKind::Function, &[child1, child2], encoding);

        // Search for symbol by name
        let results = search_through_symbols(&[parent.clone()], |s| s.name() == Some("target"));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name(), Some("target"));

        // Search by kind
        let results =
            search_through_symbols(&[parent.clone()], |s| s.kind() == SymbolKind::Variable);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].offset(), 1);
    }

    #[test]
    fn test_symbol_tree() {
        let encoding = test_encoding();
        let child = {
            let mut sym = OwnedSymbol::new(1, SymbolKind::Variable, &[], encoding);
            sym.set_name(Some("child".to_string()));
            sym
        };
        let mut parent = OwnedSymbol::new(0, SymbolKind::Function, &[], encoding);
        parent.set_name(Some("parent".to_string()));
        parent.set_children(vec![child]);

        assert_eq!(parent.children().len(), 1);
        assert_eq!(parent.children()[0].name(), Some("child"));
    }
}
