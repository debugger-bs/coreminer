//! # Variable Access Module
//!
//! Provides functionality for accessing and manipulating variables in the debugged process.
//!
//! This module contains types and methods for reading and writing variables
//! in the debuggee's memory space, leveraging DWARF debug information to locate
//! and interpret the variables correctly. It handles the complexities of
//! different variable storage locations (registers, memory, etc.) and value types.
//!
//! Key components:
//! - [`VariableExpression`]: A type for referring to variables by name
//! - [`VariableValue`]: An enum representing different forms of variable values
//! - Methods on the [`Debuggee`](crate::debuggee::Debuggee) for variable access

use serde::Serialize;
use tracing::{info, trace};

use crate::dbginfo::{search_through_symbols, OwnedSymbol, SymbolKind};
use crate::debuggee::Debuggee;
use crate::dwarf_parse::FrameInfo;
use crate::errors::{DebuggerError, Result};
use crate::{get_reg, mem_read, mem_write, set_reg, Addr, Word, WORD_BYTES};

/// A type alias for variable expressions (typically variable names)
///
/// [`VariableExpression`] is used to refer to variables in the debugged program,
/// typically by their source-level names.
///
/// In the future, this might include something like dereferencing, or logic, to filter from
/// various [OwnedSymbols][OwnedSymbol].
pub type VariableExpression = String;

/// Represents a variable value in one of several forms
///
/// [`VariableValue`] encapsulates the various ways a variable's value might be
/// represented, such as raw bytes, numeric values, or machine words. This allows
/// the debugger to work with variables of different types and sizes.
///
/// # Examples
///
/// ```
/// use coreminer::variable::VariableValue;
/// use gimli::Value;
///
/// // Create a variable value from raw bytes
/// let bytes_value = VariableValue::Bytes(vec![0x01, 0x02, 0x03, 0x04]);
///
/// // Create a variable value from a machine word
/// let word_value = VariableValue::Other(0x123456789);
///
/// // Create a variable value from a DWARF numeric value
/// let numeric_value = VariableValue::Numeric(Value::U32(42));
///
/// // Convert a variable value to a u64
/// let value_as_u64 = bytes_value.to_u64();
/// ```
#[derive(Debug, Clone, Serialize)]
pub enum VariableValue {
    /// Raw byte representation of a value
    Bytes(Vec<u8>),

    /// Machine word representation of a value
    Other(Word),

    /// DWARF numeric value
    #[serde(serialize_with = "serialize_gimli_value")]
    Numeric(gimli::Value),
}

impl VariableValue {
    /// Returns the size of the variable value in bytes
    ///
    /// # Returns
    ///
    /// The size of the variable value in bytes
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::variable::VariableValue;
    /// use gimli::Value;
    ///
    /// let bytes_value = VariableValue::Bytes(vec![0x01, 0x02, 0x03, 0x04]);
    /// assert_eq!(bytes_value.byte_size(), 4);
    ///
    /// let word_value = VariableValue::Other(0x123456789);
    /// assert_eq!(word_value.byte_size(), 8); // Assuming 64-bit (8-byte) words
    ///
    /// let numeric_value = VariableValue::Numeric(Value::U32(42));
    /// assert_eq!(numeric_value.byte_size(), 4);
    /// ```
    pub fn byte_size(&self) -> usize {
        match self {
            Self::Bytes(b) => b.len(),
            Self::Other(_w) => WORD_BYTES,
            Self::Numeric(v) => match v.value_type() {
                gimli::ValueType::U8 | gimli::ValueType::I8 => 1,
                gimli::ValueType::U16 | gimli::ValueType::I16 => 2,
                gimli::ValueType::U32 | gimli::ValueType::I32 | gimli::ValueType::F32 => 4,
                gimli::ValueType::U64
                | gimli::ValueType::I64
                | gimli::ValueType::F64
                | gimli::ValueType::Generic => 8,
            },
        }
    }

    /// Converts the variable value to a [u64]
    ///
    /// # Returns
    ///
    /// The variable value as a [u64]
    ///
    /// # Panics
    ///
    /// This method will panic if [self] is a [VariableValue::Bytes] which has more bytes than a [u64] can hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::variable::VariableValue;
    /// use gimli::Value;
    ///
    /// let bytes_value = VariableValue::Bytes(vec![0x42, 0x00, 0x00, 0x00]);
    /// assert_eq!(bytes_value.to_u64(), 0x42);
    ///
    /// let word_value = VariableValue::Other(0x123456789);
    /// assert_eq!(word_value.to_u64(), 0x123456789);
    ///
    /// let numeric_value = VariableValue::Numeric(Value::U32(42));
    /// assert_eq!(numeric_value.to_u64(), 42);
    /// ```
    pub fn to_u64(&self) -> u64 {
        match self {
            Self::Bytes(b) => {
                if b.len() > WORD_BYTES {
                    panic!("too many bytes to put into a word")
                }
                // NOTE: this is safe because `b` should never have more bytes than a u64
                crate::bytes_to_u64(b).unwrap()
            }
            Self::Other(w) => crate::bytes_to_u64(&w.to_ne_bytes()).unwrap(),
            Self::Numeric(v) => match v {
                gimli::Value::U8(v) => (*v).into(),
                gimli::Value::I8(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::U16(v) => (*v).into(),
                gimli::Value::I16(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::U32(v) => (*v).into(),
                gimli::Value::I32(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::F32(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::U64(v) => *v,
                gimli::Value::I64(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::F64(v) => crate::bytes_to_u64(&v.to_ne_bytes()).unwrap(),
                gimli::Value::Generic(v) => *v,
            },
        }
    }

    /// Resizes a value to the specified number of bytes
    ///
    /// # Parameters
    ///
    /// * `target_size` - The target size in bytes
    ///
    /// # Returns
    ///
    /// A vector of bytes with the specified size
    ///
    /// # Panics
    ///
    /// This method will panic if [self] is a [VariableValue::Bytes] which has more bytes than a [u64] can hold.
    ///
    /// # Examples
    ///
    /// ```
    /// use coreminer::variable::VariableValue;
    /// use gimli::Value;
    ///
    /// let value = VariableValue::Other(0x123456789);
    ///
    /// // Resize to 4 bytes
    /// let bytes = value.resize_to_bytes(4);
    /// assert_eq!(bytes, vec![0x89, 0x67, 0x45, 0x23]);
    ///
    /// // Resize to 2 bytes
    /// let bytes = value.resize_to_bytes(2);
    /// assert_eq!(bytes, vec![0x89, 0x67]);
    /// ```
    pub fn resize_to_bytes(&self, target_size: usize) -> Vec<u8> {
        if target_size > WORD_BYTES {
            panic!("requested byte size was larger than a word")
        }

        let mut data = self.to_u64().to_ne_bytes().to_vec();
        data.truncate(target_size);
        data
    }
}

impl From<usize> for VariableValue {
    fn from(value: usize) -> Self {
        VariableValue::Numeric(gimli::Value::Generic(value as u64))
    }
}
impl From<gimli::Value> for VariableValue {
    fn from(value: gimli::Value) -> Self {
        Self::Numeric(value)
    }
}

impl Debuggee {
    /// Filters variable expressions to find matching symbols
    ///
    /// # Parameters
    ///
    /// * `haystack` - The symbols to search through
    /// * `expression` - The variable expression to match
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<OwnedSymbol>)` - The matching symbols
    /// * `Err(DebuggerError)` - If filtering failed
    ///
    /// # Errors
    ///
    /// This function can fail if there are issues with the symbol table.
    pub fn filter_expressions(
        &self,
        haystack: &[OwnedSymbol],
        expression: &VariableExpression,
    ) -> Result<Vec<OwnedSymbol>> {
        Ok(search_through_symbols(haystack, |s| {
            s.name() == Some(expression)
        }))
    }

    /// Checks if a symbol is a valid variable
    ///
    /// # Parameters
    ///
    /// * `sym` - The symbol to check
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the symbol is a valid variable
    /// * `Err(DebuggerError)` - If the symbol is not a valid variable
    ///
    /// # Errors
    ///
    /// This function fails if:
    /// - The symbol is not a variable or parameter
    /// - The symbol does not have a data type
    /// - The symbol does not have a location
    ///
    /// If it fails, the symbol should not be used for either [Self::var_write] or [Self::var_read].
    fn check_sym_variable_ok(&self, sym: &OwnedSymbol) -> Result<()> {
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
        Ok(())
    }

    /// Writes a value to a variable
    ///
    /// Prefer to use the more high level [crate::debugger::Debugger::write_variable].
    ///
    /// # Parameters
    ///
    /// * `sym` - The symbol representing the variable
    /// * `frame_info` - Stack frame information
    /// * `value` - The value to write
    ///
    /// # Returns
    ///
    /// * `Ok(())` - If the write was successful
    /// * `Err(DebuggerError)` - If the write failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The symbol is not a valid variable
    /// - The variable's location cannot be determined
    /// - Memory or register access fails
    /// - The data type of the variable cannot be determined
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::debuggee::Debuggee;
    /// use coreminer::dwarf_parse::FrameInfo;
    /// use coreminer::addr::Addr;
    /// use coreminer::variable::VariableValue;
    ///
    /// # fn example(debuggee: &Debuggee, sym: &coreminer::dbginfo::OwnedSymbol) -> coreminer::errors::Result<()> {
    /// // Create frame information
    /// // Calculate these with the ELF and DWARF information
    /// let frame_info = FrameInfo::new(
    ///     Some(Addr::from(0x7fffffffe000usize)),
    ///     Some(Addr::from(0x7fffffffe010usize))
    /// );
    ///
    /// // Write the value 42 to the variable
    /// debuggee.var_write(sym, &frame_info, VariableValue::from(42usize))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn var_write(
        &self,
        sym: &OwnedSymbol,
        frame_info: &FrameInfo,
        value: VariableValue,
    ) -> Result<()> {
        self.check_sym_variable_ok(sym)?;
        let datatype = match self.get_type_for_symbol(sym)? {
            Some(d) => d,
            None => return Err(DebuggerError::NoDatatypeFound),
        };

        let loc_attr = sym.location().unwrap();
        let location = self.parse_location(loc_attr, frame_info, sym.encoding())?;

        match location {
            gimli::Location::Address { address } => {
                let byte_size = if let Some(bs) = datatype.byte_size() {
                    bs
                } else {
                    panic!("datatype found but it had no byte_size?")
                };
                let value_raw = value.resize_to_bytes(byte_size);
                let addr: Addr = address.into();
                trace!("writing to {addr}");
                let written = mem_write(&value_raw, self.pid, addr)?;
                assert_eq!(written, value.byte_size());
            }
            gimli::Location::Register { register } => {
                set_reg(self.pid, register.try_into()?, value.to_u64())?
            }
            other => unimplemented!(
                "writing to variable with gimli location of type {other:?} is not implemented"
            ),
        }

        Ok(())
    }

    /// Reads the value of a variable
    ///
    /// Prefer to use the more high level [crate::debugger::Debugger::read_variable].
    ///
    /// # Parameters
    ///
    /// * `sym` - The symbol representing the variable
    /// * `frame_info` - Stack frame information
    ///
    /// # Returns
    ///
    /// * `Ok(VariableValue)` - The variable's value
    /// * `Err(DebuggerError)` - If the read failed
    ///
    /// # Errors
    ///
    /// This function can fail if:
    /// - The symbol is not a valid variable
    /// - The variable's location cannot be determined
    /// - Memory or register access fails
    /// - The data type of the variable cannot be determined
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use coreminer::debuggee::Debuggee;
    /// use coreminer::dwarf_parse::FrameInfo;
    /// use coreminer::addr::Addr;
    ///
    /// # fn example(debuggee: &Debuggee, sym: &coreminer::dbginfo::OwnedSymbol) -> coreminer::errors::Result<()> {
    /// // Create frame information
    /// // Calculate these with the ELF and DWARF information
    /// let frame_info = FrameInfo::new(
    ///     Some(Addr::from(0x7fffffffe000usize)),
    ///     Some(Addr::from(0x7fffffffe010usize))
    /// );
    ///
    /// // Read the variable's value
    /// let value = debuggee.var_read(sym, &frame_info)?;
    /// println!("Variable value: {:?}", value);
    /// # Ok(())
    /// # }
    /// ```
    pub fn var_read(&self, sym: &OwnedSymbol, frame_info: &FrameInfo) -> Result<VariableValue> {
        self.check_sym_variable_ok(sym)?;
        let datatype = match self.get_type_for_symbol(sym)? {
            Some(d) => d,
            None => return Err(DebuggerError::NoDatatypeFound),
        };

        let loc_attr = sym.location().unwrap();
        let location = self.parse_location(loc_attr, frame_info, sym.encoding())?;

        let value = match location {
            gimli::Location::Value { value } => value.into(),
            gimli::Location::Bytes { value } => VariableValue::Bytes(value.to_vec()),
            gimli::Location::Address { address } => {
                let addr: Addr = address.into();
                info!("reading var from {addr}");
                let size = datatype.byte_size().expect("datatype had no byte_size");
                let mut buf = vec![0; size];
                let len = mem_read(&mut buf, self.pid, addr)?;
                assert_eq!(len, size);

                VariableValue::Bytes(buf)
            }
            gimli::Location::Register { register } => {
                VariableValue::Other(get_reg(self.pid, register.try_into()?)? as i64)
            }
            gimli::Location::Empty => todo!(),
            other => unimplemented!("gimli location of type {other:?} is not implemented"),
        };

        Ok(value)
    }
}

fn serialize_gimli_value<S>(
    value: &gimli::Value,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match value {
        gimli::Value::U8(v) => serializer.serialize_u8(*v),
        gimli::Value::I8(v) => serializer.serialize_i8(*v),
        gimli::Value::U16(v) => serializer.serialize_u16(*v),
        gimli::Value::I16(v) => serializer.serialize_i16(*v),
        gimli::Value::U32(v) => serializer.serialize_u32(*v),
        gimli::Value::I32(v) => serializer.serialize_i32(*v),
        gimli::Value::U64(v) => serializer.serialize_u64(*v),
        gimli::Value::I64(v) => serializer.serialize_i64(*v),
        gimli::Value::F32(v) => serializer.serialize_f32(*v),
        gimli::Value::F64(v) => serializer.serialize_f64(*v),
        gimli::Value::Generic(v) => serializer.serialize_u64(*v),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_variable_value_sizing() {
        let v = VariableValue::Numeric(gimli::Value::U8(42));
        assert_eq!(v.byte_size(), 1);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::I8(42));
        assert_eq!(v.byte_size(), 1);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::U16(42));
        assert_eq!(v.byte_size(), 2);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::I16(42));
        assert_eq!(v.byte_size(), 2);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::U32(42));
        assert_eq!(v.byte_size(), 4);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::I32(42));
        assert_eq!(v.byte_size(), 4);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::U64(42));
        assert_eq!(v.byte_size(), 8);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::I64(42));
        assert_eq!(v.byte_size(), 8);
        assert_eq!(v.to_u64(), 42);

        let v = VariableValue::Numeric(gimli::Value::F32(42.19));
        assert_eq!(v.byte_size(), 4);

        let v = VariableValue::Numeric(gimli::Value::F64(42.19));
        assert_eq!(v.byte_size(), 8);

        let v = VariableValue::Other(19);
        assert_eq!(v.byte_size(), 8);
        assert_eq!(v.to_u64(), 19);

        let v = VariableValue::Bytes(vec![0x19, 19, 19]);
        assert_eq!(v.byte_size(), 3);
        assert_eq!(v.to_u64(), 1250073);
    }

    #[test]
    fn test_resize_bytes() {
        let v = VariableValue::Other(42);
        let bytes = v.resize_to_bytes(4);
        assert_eq!(bytes.len(), 4);
        assert_eq!(bytes, vec![42, 0, 0, 0]);

        let bytes = v.resize_to_bytes(8);
        assert_eq!(bytes.len(), 8);
        assert_eq!(bytes, vec![42, 0, 0, 0, 0, 0, 0, 0]);

        let bytes = v.resize_to_bytes(1);
        assert_eq!(bytes.len(), 1);
        assert_eq!(bytes, vec![42]);
    }

    #[test]
    fn test_signed_integer_to_bytes() {
        let v: i8 = 19;
        let b = v.to_ne_bytes();
        assert_eq!(b.len(), 1);
        assert_eq!(b, [19]);

        let v: i32 = 19;
        let b = v.to_ne_bytes();
        assert_eq!(b.len(), 4);
        assert_eq!(b, [19, 0, 0, 0]);

        let v: i32 = -19;
        let b = v.to_ne_bytes();
        assert_eq!(b.len(), 4);
        assert_eq!(b, [237, 255, 255, 255]);
    }
}
