use tracing::{info, trace};

use crate::dbginfo::{search_through_symbols, OwnedSymbol, SymbolKind};
use crate::debuggee::Debuggee;
use crate::dwarf_parse::FrameInfo;
use crate::errors::{DebuggerError, Result};
use crate::{get_reg, mem_read, mem_write, set_reg, Addr, Word, WORD_BYTES};

pub type VariableExpression = String;

#[derive(Debug, Clone)]
pub enum VariableValue {
    Bytes(Vec<u8>),
    Other(Word),
    Numeric(gimli::Value),
}

impl VariableValue {
    fn byte_size(&self) -> usize {
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

    fn to_u64(&self) -> u64 {
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

    fn resize_to_bytes(&self, target_size: usize) -> Vec<u8> {
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
    pub fn filter_expressions(
        &self,
        haystack: &[OwnedSymbol],
        expression: &VariableExpression,
    ) -> Result<Vec<OwnedSymbol>> {
        Ok(search_through_symbols(haystack, |s| {
            s.name() == Some(expression)
        }))
    }

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
