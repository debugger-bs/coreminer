use gimli::{Reader, Unit};
use tracing::warn;

use crate::debugger::Debuggee;
use crate::errors::{DebuggerError, Result};
use crate::Addr;

pub(crate) type GimliReaderThing = gimli::EndianReader<gimli::LittleEndian, std::rc::Rc<[u8]>>;

impl Debuggee<'_> {
    pub(crate) fn parse_addr_low(
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
        base_addr: Addr,
    ) -> Result<Option<Addr>> {
        Ok(if let Some(a) = attribute {
            let a: u64 = match dwarf.attr_address(unit, a.value())? {
                None => {
                    warn!("could not parse addr: {a:?}");
                    return Ok(None);
                }
                Some(a) => a,
            };
            Some(Addr::from_relative(base_addr, a as usize))
        } else {
            None
        })
    }

    pub(crate) fn parse_addr_high(
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
        low: Option<Addr>,
    ) -> Result<Option<Addr>> {
        Ok(if let Some(a) = attribute {
            let addr: Addr = match a.value().udata_value() {
                None => {
                    warn!("could not parse addr: {a:?}");
                    return Ok(None);
                }
                Some(a) => {
                    if let Some(l) = low {
                        l + a as usize
                    } else {
                        return Err(DebuggerError::HighAddrExistsButNotLowAddr);
                    }
                }
            };
            Some(addr)
        } else {
            None
        })
    }

    pub(crate) fn parse_string(
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
    ) -> Result<Option<String>> {
        Ok(if let Some(a) = attribute {
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

    pub(crate) fn parse_datatype(
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
    ) -> Result<Option<usize>> {
        Ok(if let Some(a) = attribute {
            if let gimli::AttributeValue::UnitRef(thing) = a.value() {
                Some(thing.0)
            } else {
                warn!("idk");
                None
            }
        } else {
            None
        })
    }
}
