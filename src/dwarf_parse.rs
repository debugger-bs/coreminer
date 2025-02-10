use gimli::{Reader, Unit};

use crate::debugger::Debuggee;
use crate::errors::Result;
use crate::Addr;

pub(crate) type GimliReaderThing = gimli::EndianReader<gimli::LittleEndian, std::rc::Rc<[u8]>>;

impl Debuggee<'_> {
    pub(crate) fn parse_addr(
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        attirbute: Option<gimli::Attribute<GimliReaderThing>>,
        base_addr: Addr,
    ) -> Result<Option<Addr>> {
        Ok(if let Some(a) = attirbute {
            let a: u64 = match dwarf.attr_address(unit, a.value())? {
                None => return Ok(None),
                Some(a) => a,
            };
            Some(Addr::from_relative(base_addr, a as usize))
        } else {
            None
        })
    }

    pub(crate) fn parse_string(
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        attirbute: Option<gimli::Attribute<GimliReaderThing>>,
    ) -> Result<Option<String>> {
        Ok(if let Some(a) = attirbute {
            Some(
                dwarf
                    .attr_string(unit, a.value())?
                    .to_string_lossy()?
                    .to_string(),
            )
        } else {
            None
        })
    }
}
