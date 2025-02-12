use gimli::{Attribute, Encoding};

use crate::dbginfo::{OwnedSymbol, SymbolKind};
use crate::debuggee::Debuggee;
use crate::dwarf_parse::{FrameInfos, GimliReaderThing};
use crate::errors::{DebuggerError, Result};
use crate::{Addr, Word};

pub type VariableExpression = String;

#[derive(Debug, Clone)]
pub enum VariableValue {
    Other(Word),
}

impl Debuggee<'_> {
    pub fn filter_expressions(
        &self,
        haystack: &[OwnedSymbol],
        expression: &VariableExpression,
    ) -> Result<Vec<OwnedSymbol>> {
        Ok(haystack
            .iter()
            .filter(|a| a.name() == Some(expression))
            .cloned()
            .collect())
    }

    pub fn var_read(&self, sym: &OwnedSymbol) -> Result<VariableValue> {
        match sym.kind() {
            SymbolKind::Variable | SymbolKind::Parameter => (),
            _ => return Err(DebuggerError::WrongSymbolKind(sym.kind())),
        }
        if sym.datatype().is_none() {
            return Err(DebuggerError::VariableSymbolNoType);
        }
        if sym.location().is_none() {
            return Err(DebuggerError::SymbolHasNoLocation);
        }
        let loc = sym.location().unwrap();
        let _datatype = self.get_type_for_symbol(sym)?;
        let (addr, size) = self.read_location_from_attribute(loc, sym.encoding())?;

        let mut buf = vec![0; size];
        let rd = crate::mem_read(&mut buf, self.pid, addr)?;
        assert_eq!(rd, size);

        todo!()
    }

    pub(crate) fn read_location_from_attribute(
        &self,
        loc_attr: &Attribute<GimliReaderThing>,
        encoding: Encoding,
    ) -> Result<(Addr, usize)> {
        let mut frame_info = FrameInfos::empty();

        let location = self.parse_location(loc_attr, &mut frame_info, encoding)?;

        todo!()
    }
}
