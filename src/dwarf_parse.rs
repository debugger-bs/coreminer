use core::panic;
use std::os::unix::fs::OpenOptionsExt;

use gimli::write::LocationListOffsets;
use gimli::{Evaluation, Expression, Piece, Reader, Unit};
use nix::unistd::Pid;
use tracing::warn;

use crate::dbginfo::GimliLocation;
use crate::debugger::Debuggee;
use crate::errors::{DebuggerError, Result};
use crate::{mem_read, Addr};

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

    pub(crate) fn parse_location(
        pid: Pid,
        dwarf: &gimli::Dwarf<GimliReaderThing>,
        unit: &Unit<GimliReaderThing>,
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
        frame_base: Addr,
        registers: nix::libc::user_regs_struct,
    ) -> Result<Option<GimliLocation>> {
        let attribute = match attribute {
            None => return Ok(None),
            Some(a) => a,
        };

        match attribute.value() {
            gimli::AttributeValue::Exprloc(expr) => Self::eval_expression(expr)?,
            gimli::AttributeValue::LocationListsRef(loclist_offs) => {
                Self::parse_loclist(loclist_offs)?
            }
            _ => panic!("we did not know a location could be this"),
        }
    }

    pub(crate) fn parse_loclist(
        loclist_offset: LocationListOffsets,
    ) -> Result<Option<GimliLocation>> {
        todo!()
    }

    pub(crate) fn eval_expression(
        pid: Pid,
        mut evaluation: Evaluation<GimliReaderThing>,
    ) -> Result<Option<GimliLocation>> {
        let mut res = evaluation.evaluate()?;
        let pieces;
        loop {
            match res {
                gimli::EvaluationResult::Complete => {
                    pieces = evaluation.result();
                    break;
                }
                gimli::EvaluationResult::RequiresMemory {
                    address,
                    size,
                    .. // there is more but that is getting to complicated, just give gimli 
                    // unsized values of the right size
                } => {
                    let mut buff = vec![0; size as usize];
                    let addr: Addr = address.into(); // NOTE: may be relative?
                    let read_this_many_bytes = mem_read(&mut buff, pid, addr)?;
                    assert_eq!(size as usize, read_this_many_bytes);
                    let value = to_value(size, &buff);
                    evaluation.resume_with_memory(value)?;
                }
                gimli::EvaluationResult::RequiresRegister { register, .. /* ignore the actual type and give as word */ } => {
                    let reg= crate::Register::try_from(register)?;
                    let reg_value = crate::get_reg(pid, reg)?;
                    evaluation.resume_with_register(gimli::Value::from_u64(gimli::ValueType::Generic, reg_value)?)?;
                }
            }
        }
        todo!()
    }
}

fn to_value(size: u8, buff: &[u8]) -> gimli::Value {
    match size {
        1 => gimli::Value::U8(buff[0]),
        2 => gimli::Value::U16(u16::from_be_bytes([buff[0], buff[1]])),
        4 => gimli::Value::U32(u32::from_be_bytes([buff[0], buff[1], buff[2], buff[3]])),
        x => unimplemented!("Requested memory with size {x}, which is not supported yet."),
    }
}
