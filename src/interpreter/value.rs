use std::collections::HashMap;

use crate::hir::{self, HirBlock};

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Bool(bool),
    Byte(u8),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Str(String),
    Array(Vec<Value>),
    Struct {
        path: Vec<String>,
        fields: HashMap<String, Value>,
    },
    Function(FunctionValue),
}

#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub params: Vec<hir::HirParam>,
    pub body: HirBlock,
    pub captured: Vec<HashMap<String, Value>>, // captured lexical scopes
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        use Value as V;
        match (self, other) {
            (V::Unit, V::Unit) => true,
            (V::Bool(a), V::Bool(b)) => a == b,
            (V::Byte(a), V::Byte(b)) => a == b,
            (V::I8(a), V::I8(b)) => a == b,
            (V::I16(a), V::I16(b)) => a == b,
            (V::I32(a), V::I32(b)) => a == b,
            (V::I64(a), V::I64(b)) => a == b,
            (V::U8(a), V::U8(b)) => a == b,
            (V::U16(a), V::U16(b)) => a == b,
            (V::U32(a), V::U32(b)) => a == b,
            (V::U64(a), V::U64(b)) => a == b,
            (V::F32(a), V::F32(b)) => a == b,
            (V::F64(a), V::F64(b)) => a == b,
            (V::Str(a), V::Str(b)) => a == b,
            (V::Array(a), V::Array(b)) => a == b,
            (
                V::Struct {
                    path: p1,
                    fields: f1,
                },
                V::Struct {
                    path: p2,
                    fields: f2,
                },
            ) => p1 == p2 && f1 == f2,
            // Functions are not comparable for equality in this simple runtime
            (V::Function(_), V::Function(_)) => false,
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Unit => write!(f, "()"),
            Value::Bool(b) => write!(f, "{}", b),
            Value::Byte(b) => write!(f, "{}", b),
            Value::I8(i) => write!(f, "{}", i),
            Value::I16(i) => write!(f, "{}", i),
            Value::I32(i) => write!(f, "{}", i),
            Value::I64(i) => write!(f, "{}", i),
            Value::U8(i) => write!(f, "{}", i),
            Value::U16(i) => write!(f, "{}", i),
            Value::U32(i) => write!(f, "{}", i),
            Value::U64(i) => write!(f, "{}", i),
            Value::F32(x) => write!(f, "{}", x),
            Value::F64(x) => write!(f, "{}", x),
            Value::Str(s) => write!(f, "\"{}\"", s),
            Value::Array(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Struct { path, fields } => {
                let name = if path.is_empty() {
                    "<anon>".to_string()
                } else {
                    path.join("::")
                };
                write!(f, "{} {{", name)?;
                let mut iter = fields.iter();
                if let Some((k, v)) = iter.next() {
                    write!(f, "{}: {}", k, v)?;
                    for (k, v) in iter {
                        write!(f, ", {}: {}", k, v)?;
                    }
                }
                write!(f, "}}")
            }
            Value::Function(_) => write!(f, "<fn>"),
        }
    }
}

/// Convert a runtime value into a process exit code.
/// Convention: 0 indicates success, non-zero indicates failure.
pub fn value_to_exit_code(value: &Value) -> i32 {
    match value {
        Value::Unit => 0,
        Value::Bool(b) => {
            if *b {
                0
            } else {
                1
            }
        }
        Value::Byte(b) => *b as i32,
        Value::I8(i) => *i as i32,
        Value::I16(i) => *i as i32,
        Value::I32(i) => *i,
        Value::I64(i) => {
            if *i > i32::MAX as i64 {
                i32::MAX
            } else if *i < i32::MIN as i64 {
                i32::MIN
            } else {
                *i as i32
            }
        }
        Value::U8(i) => *i as i32,
        Value::U16(i) => *i as i32,
        Value::U32(i) => (*i).min(i32::MAX as u32) as i32,
        Value::U64(i) => (*i).min(i32::MAX as u64) as i32,
        Value::F32(f) => {
            if f.is_nan() {
                1
            } else {
                *f as i32
            }
        }
        Value::F64(f) => {
            if f.is_nan() {
                1
            } else {
                *f as i32
            }
        }
        // Strings, arrays, and functions default to success
        Value::Str(_) | Value::Array(_) | Value::Struct { .. } | Value::Function(_) => 0,
    }
}
