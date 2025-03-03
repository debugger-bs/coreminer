//! # DWARF Debug Information Parser
//!
//! Provides functionality for parsing and interpreting DWARF debug information.
//!
//! This module contains utilities for working with DWARF debug information,
//! which is essential for mapping between memory addresses and source code,
//! understanding variable locations, and inspecting program structure at runtime.
//!
//! The module focuses on:
//!
//! - Parsing DWARF expressions to locate variables in memory or registers
//! - Evaluating DWARF location descriptions
//! - Extracting type and scope information
//! - Managing frame information for stack unwinding and variable access
//!
//! DWARF is a standardized debugging data format used by many compilers and
//! debugging tools. This module leverages the `gimli` crate to parse and interpret
//! DWARF sections from executable files.

use gimli::{Encoding, Expression, Reader, Unit};
use tracing::{trace, warn};

use crate::dbginfo::GimliLocation;
use crate::debuggee::Debuggee;
use crate::errors::{DebuggerError, Result};
use crate::{mem_read, Addr};

/// Type alias for the Gimli reader used throughout the module
///
/// This specialized reader type is used to access DWARF information in memory
/// with the correct endianness.
pub(crate) type GimliReaderThing = gimli::EndianReader<gimli::LittleEndian, std::rc::Rc<[u8]>>;

/// Represents stack frame information needed for variable access
///
/// In order to locate variables correctly, a debugger needs information about
/// the current stack frame, including base pointers and canonical frame address.
/// This struct holds that information.
pub struct FrameInfo {
    /// Base address of the current stack frame
    pub frame_base: Option<Addr>,

    /// Canonical Frame Address (CFA) for the current frame
    pub canonical_frame_address: Option<Addr>,
}

impl FrameInfo {
    /// Creates a new [`FrameInfo`] instance
    ///
    /// # Parameters
    ///
    /// * `frame_base` - Base address of the current stack frame
    /// * `canonical_frame_address` - Canonical Frame Address for the current frame
    ///
    /// # Returns
    ///
    /// A new `FrameInfo` instance with the provided frame information
    ///
    /// # Examples
    ///
    /// You could just fill this with any [Addr], but it's more complicated.
    /// See [`crate::debugger::Debugger::prepare_variable_access`].
    #[must_use]
    pub fn new(frame_base: Option<Addr>, canonical_frame_address: Option<Addr>) -> FrameInfo {
        FrameInfo {
            frame_base,
            canonical_frame_address,
        }
    }

    /// Gets the Canonical Frame Address
    ///
    /// The Canonical Frame Address (CFA) is a value defined by the ABI that
    /// identifies a fixed position within a stack frame. Read up on it, it's rather confusing.
    ///
    /// # Returns
    ///
    /// The Canonical Frame Address, if available
    #[must_use]
    pub fn canonical_frame_address(&self) -> Option<Addr> {
        self.canonical_frame_address
    }

    /// Gets the frame base address
    ///
    /// The frame base is typically the value of the frame pointer register (rbp/ebp)
    /// and serves as a reference point for accessing local variables.
    ///
    /// # Returns
    ///
    /// The frame base address, if available
    #[must_use]
    pub fn frame_base(&self) -> Option<Addr> {
        self.frame_base
    }
}

impl Debuggee {
    /// Parses a DWARF low address attribute (`DW_AT_low_pc`)
    ///
    /// # Parameters
    ///
    /// * `dwarf` - The DWARF information
    /// * `unit` - The compilation unit
    /// * `attribute` - The attribute to parse
    /// * `base_addr` - The base address of the loaded executable
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Addr))` - The parsed address
    /// * `Ok(None)` - If the attribute is not present
    /// * `Err(DebuggerError)` - If parsing failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The attribute value cannot be parsed as an address
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
            Some(base_addr + a as usize)
        } else {
            None
        })
    }

    /// Parses a DWARF high address attribute (`DE_AT_high_pc`)
    ///
    /// # Parameters
    ///
    /// * `attribute` - The attribute to parse
    /// * `low` - The corresponding low address
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Addr))` - The parsed address
    /// * `Ok(None)` - If the attribute is not present
    /// * `Err(DebuggerError)` - If parsing failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The attribute value cannot be parsed as an address
    /// - A high address is provided without a corresponding low address
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

    /// Parses a DWARF string attribute, like `DW_AT_name`
    ///
    /// # Parameters
    ///
    /// * `dwarf` - The DWARF information
    /// * `unit` - The compilation unit
    /// * `attribute` - The attribute to parse
    ///
    /// # Returns
    ///
    /// * `Ok(Some(String))` - The parsed string
    /// * `Ok(None)` - If the attribute is not present
    /// * `Err(DebuggerError)` - If parsing failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The attribute value cannot be parsed as a string
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

    /// Parses a DWARF datatype reference attribute (`DW_AT_type`)
    ///
    /// # Parameters
    ///
    /// * `attribute` - The attribute to parse
    ///
    /// # Returns
    ///
    /// * `Some(usize)` - The parsed datatype reference
    /// * `None` - If the attribute is not present
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The attribute value is not a valid unit reference
    pub(crate) fn parse_datatype(
        attribute: Option<gimli::Attribute<GimliReaderThing>>,
    ) -> Option<usize> {
        if let Some(a) = attribute {
            if let gimli::AttributeValue::UnitRef(thing) = a.value() {
                Some(thing.0)
            } else {
                warn!("idk");
                None
            }
        } else {
            None
        }
    }

    /// Parses a DWARF location attribute
    ///
    /// Location attributes describe where a variable or parameter is stored,
    /// which could be in memory, a register, or computed by an expression.
    ///
    /// # Parameters
    ///
    /// * `attribute` - The attribute to parse
    /// * `frame_info` - Stack frame information for context
    /// * `encoding` - DWARF encoding information
    ///
    /// # Returns
    ///
    /// * `Ok(GimliLocation)` - The parsed location
    /// * `Err(DebuggerError)` - If parsing failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The attribute value is not a valid location expression
    /// - Evaluation of the location expression fails
    ///
    /// # Panics
    ///
    /// This function will panic if the attribute value is not an expression location
    pub(crate) fn parse_location(
        &self,
        attribute: &gimli::Attribute<GimliReaderThing>,
        frame_info: &FrameInfo,
        encoding: Encoding,
    ) -> Result<GimliLocation> {
        match attribute.value() {
            gimli::AttributeValue::Exprloc(expr) => {
                self.eval_expression(expr, frame_info, encoding)
            }
            _ => unimplemented!("we did not know a location could be this"),
        }
    }

    /// Evaluates a DWARF expression
    ///
    /// DWARF expressions are used to compute the location of variables,
    /// parameters, and other program entities at runtime.
    ///
    /// # Parameters
    ///
    /// * `expression` - The DWARF expression to evaluate
    /// * `frame_info` - Stack frame information for context
    /// * `encoding` - DWARF encoding information
    ///
    /// # Returns
    ///
    /// * `Ok(GimliLocation)` - The resulting location
    /// * `Err(DebuggerError)` - If evaluation failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - Memory access fails
    /// - Register access fails
    /// - Required frame information is missing
    /// - The expression is invalid or unsupported
    ///
    /// # Panics
    ///
    /// This function will panic if the expression evaluation returns no pieces
    pub(crate) fn eval_expression(
        &self,
        expression: Expression<GimliReaderThing>,
        frame_info: &FrameInfo,
        encoding: Encoding,
    ) -> Result<GimliLocation> {
        let mut evaluation = expression.evaluation(encoding);
        let mut res = evaluation.evaluate()?;
        loop {
            match res {
                gimli::EvaluationResult::Complete => {
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
                    let read_this_many_bytes = mem_read(&mut buff, self.pid, addr)?;
                    assert_eq!(size as usize, read_this_many_bytes);
                    let value = to_value(size, &buff);
                    res = evaluation.resume_with_memory(value)?;
                }
                gimli::EvaluationResult::RequiresRegister { register, .. /* ignore the actual type and give as word */ } => {
                    let reg_kind= crate::Register::try_from(register)?;
                    let reg_value = crate::get_reg(self.pid, reg_kind)?;
                    res = evaluation.resume_with_register(gimli::Value::from_u64(gimli::ValueType::Generic, reg_value)?)?;
                }
                gimli::EvaluationResult::RequiresFrameBase =>{
                    let frame_base: Addr = frame_info.frame_base.expect("no frame base was given");
                    trace!("frame_base: {frame_base}");

                    res = evaluation.resume_with_frame_base(
                        frame_base.u64()
                    )?;
                }
                gimli::EvaluationResult::RequiresCallFrameCfa => {
                    let cfa: Addr = frame_info.canonical_frame_address.expect("no cfa was given");
                    trace!("cfa: {cfa}");
                    res = evaluation.resume_with_call_frame_cfa(cfa.into())?;
                }
                other => {
                    unimplemented!("Gimli expression parsing for {other:?} is not implemented")
                }
            }
        }
        let pieces = evaluation.result();

        if pieces.is_empty() {
            warn!("really? we did all that parsing and got NOTHING");
            Err(DebuggerError::VarExprReturnedNothing(
                "No pieces".to_string(),
            ))
        } else {
            let loc = pieces[0].location.clone();
            trace!("location for the expression: {loc:?}");
            Ok(loc)
        }
    }
}

/// Converts bytes to a DWARF value based on size
///
/// # Parameters
///
/// * `size` - The size of the value in bytes
/// * `buff` - The raw bytes to convert
///
/// # Returns
///
/// A DWARF value of the appropriate type and size
///
/// # Panics
///
/// This function will panic if the requested size is not supported
/// (currently supports 1, 2, and 4 byte values)
fn to_value(size: u8, buff: &[u8]) -> gimli::Value {
    match size {
        1 => gimli::Value::U8(buff[0]),
        2 => gimli::Value::U16(u16::from_be_bytes([buff[0], buff[1]])),
        4 => gimli::Value::U32(u32::from_be_bytes([buff[0], buff[1], buff[2], buff[3]])),
        x => unimplemented!("Requested memory with size {x}, which is not supported yet."),
    }
}
