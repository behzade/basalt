use std::collections::HashMap;

use crate::hir::{self, HirBlock};
use crate::interpreter::env::Scope;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AllocationOwner {
    pub id: u64,
    pub generation: u64,
}

#[derive(Debug, Clone)]
pub struct Managed<T> {
    pub value: T,
    pub owner: Option<AllocationOwner>,
}

impl<T> Managed<T> {
    pub(crate) fn new(value: T) -> Self {
        Self { value, owner: None }
    }
}

impl<T> std::ops::Deref for Managed<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: PartialEq> PartialEq for Managed<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

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
    Region(AllocationOwner),
    Str(Managed<String>),
    Array(Managed<Vec<Value>>),
    Map(Managed<Vec<(Value, Value)>>),
    Struct {
        path: Vec<String>,
        fields: HashMap<String, Value>,
        owner: Option<AllocationOwner>,
    },
    EnumVariant {
        path: Vec<String>,
        fields: HashMap<String, Value>,
        owner: Option<AllocationOwner>,
    },
    Function(FunctionValue),
    Handler(HandlerValue),
}

#[derive(Debug, Clone)]
pub struct FunctionValue {
    pub(crate) owner: Option<AllocationOwner>,
    pub params: Vec<hir::HirParam>,
    pub body: HirBlock,
    pub(crate) captured: Vec<Scope>, // shared captured lexical bindings
}

#[derive(Debug, Clone)]
pub struct HandlerValue {
    pub(crate) owner: Option<AllocationOwner>,
    pub entries: Vec<HandlerEntry>,
}

#[derive(Debug, Clone)]
pub struct HandlerEntry {
    pub effects: Vec<hir::Ty>,
    pub functions: Vec<hir::HirFunction>,
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
            (V::Region(a), V::Region(b)) => a == b,
            (V::Str(a), V::Str(b)) => a == b,
            (V::Array(a), V::Array(b)) => a == b,
            (V::Map(a), V::Map(b)) => a == b,
            (
                V::Struct {
                    path: p1,
                    fields: f1,
                    ..
                },
                V::Struct {
                    path: p2,
                    fields: f2,
                    ..
                },
            ) => p1 == p2 && f1 == f2,
            (
                V::EnumVariant {
                    path: p1,
                    fields: f1,
                    ..
                },
                V::EnumVariant {
                    path: p2,
                    fields: f2,
                    ..
                },
            ) => p1 == p2 && f1 == f2,
            // Functions are not comparable for equality in this simple runtime
            (V::Function(_), V::Function(_)) => false,
            (V::Handler(_), V::Handler(_)) => false,
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
            Value::Region(owner) => write!(f, "<region {}:{}>", owner.id, owner.generation),
            Value::Str(s) => write!(f, "\"{}\"", s.value),
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
            Value::Map(entries) => {
                write!(f, "{{")?;
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}: {}", key, value)?;
                }
                write!(f, "}}")
            }
            Value::Struct { path, fields, .. } => {
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
            Value::EnumVariant { path, fields, .. } => {
                let name = if path.is_empty() {
                    "<variant>".to_string()
                } else {
                    path.join("::")
                };
                if fields.is_empty() {
                    write!(f, "{}", name)
                } else {
                    write!(f, "{}(", name)?;
                    let mut iter = fields.iter();
                    if let Some((_, v)) = iter.next() {
                        write!(f, "{}", v)?;
                        for (_, v) in iter {
                            write!(f, ", {}", v)?;
                        }
                    }
                    write!(f, ")")
                }
            }
            Value::Function(_) => write!(f, "<fn>"),
            Value::Handler(_) => write!(f, "<handler>"),
        }
    }
}

impl Value {
    pub(crate) fn owner(&self) -> Option<AllocationOwner> {
        match self {
            Value::Str(value) => value.owner,
            Value::Array(value) => value.owner,
            Value::Map(value) => value.owner,
            Value::Struct { owner, .. } | Value::EnumVariant { owner, .. } => *owner,
            Value::Function(value) => value.owner,
            Value::Handler(value) => value.owner,
            _ => None,
        }
    }

    pub(crate) fn assign_owner_recursive(&mut self, owner: AllocationOwner) {
        match self {
            Value::Str(value) => value.owner = Some(owner),
            Value::Array(value) => {
                value.owner = Some(owner);
                for item in &mut value.value {
                    item.assign_owner_recursive(owner);
                }
            }
            Value::Map(value) => {
                value.owner = Some(owner);
                for (key, item) in &mut value.value {
                    key.assign_owner_recursive(owner);
                    item.assign_owner_recursive(owner);
                }
            }
            Value::Struct {
                fields,
                owner: value_owner,
                ..
            }
            | Value::EnumVariant {
                fields,
                owner: value_owner,
                ..
            } => {
                *value_owner = Some(owner);
                for item in fields.values_mut() {
                    item.assign_owner_recursive(owner);
                }
            }
            Value::Function(value) => value.owner = Some(owner),
            Value::Handler(value) => value.owner = Some(owner),
            _ => {}
        }
    }

    pub(crate) fn detach_views_for_copy(&mut self, owner: AllocationOwner) {
        match self {
            Value::Array(items) => {
                for item in &mut items.value {
                    item.detach_views_for_copy(owner);
                }
            }
            Value::Map(entries) => {
                for (key, value) in &mut entries.value {
                    key.detach_views_for_copy(owner);
                    value.detach_views_for_copy(owner);
                }
            }
            Value::Struct { fields, .. } | Value::EnumVariant { fields, .. } => {
                for value in fields.values_mut() {
                    value.detach_views_for_copy(owner);
                }
            }
            Value::Function(function) => {
                for scope in &mut function.captured {
                    for binding in scope.values_mut() {
                        let mut captured = binding.value();
                        captured.detach_views_for_copy(owner);
                        captured.assign_owner_recursive(owner);
                        *binding = binding.detached_with(captured, owner);
                    }
                }
            }
            _ => {}
        }
    }

    pub(crate) fn visit_owners(&self, visit: &mut impl FnMut(AllocationOwner)) {
        if let Some(owner) = self.owner() {
            visit(owner);
        }
        if let Value::Region(owner) = self {
            visit(*owner);
        }
        match self {
            Value::Array(items) => {
                for item in &items.value {
                    item.visit_owners(visit);
                }
            }
            Value::Map(entries) => {
                for (key, value) in &entries.value {
                    key.visit_owners(visit);
                    value.visit_owners(visit);
                }
            }
            Value::Struct { fields, .. } | Value::EnumVariant { fields, .. } => {
                for value in fields.values() {
                    value.visit_owners(visit);
                }
            }
            Value::Function(function) => {
                for binding in function.captured.iter().flat_map(|scope| scope.values()) {
                    binding.value().visit_owners(visit);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn visit_region_handles(&self, visit: &mut impl FnMut(AllocationOwner)) {
        match self {
            Value::Region(owner) => visit(*owner),
            Value::Array(items) => {
                for item in &items.value {
                    item.visit_region_handles(visit);
                }
            }
            Value::Map(entries) => {
                for (key, value) in &entries.value {
                    key.visit_region_handles(visit);
                    value.visit_region_handles(visit);
                }
            }
            Value::Struct { fields, .. } | Value::EnumVariant { fields, .. } => {
                for value in fields.values() {
                    value.visit_region_handles(visit);
                }
            }
            Value::Function(function) => {
                for binding in function.captured.iter().flat_map(|scope| scope.values()) {
                    binding.value().visit_region_handles(visit);
                }
            }
            _ => {}
        }
    }

    pub(crate) fn allocation_kind(&self) -> &'static str {
        match self {
            Value::Unit
            | Value::Bool(_)
            | Value::Byte(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_) => "scalar",
            Value::Region(_) => "region capability",
            Value::Str(_) => "str",
            Value::Array(_) => "array",
            Value::Map(_) => "map",
            Value::Struct { .. } => "struct",
            Value::EnumVariant { .. } => "enum variant",
            Value::Function(_) => "function",
            Value::Handler(_) => "handler",
        }
    }

    pub(crate) fn stack_size_bytes(&self) -> usize {
        match self {
            Value::Unit
            | Value::Bool(_)
            | Value::Byte(_)
            | Value::I8(_)
            | Value::I16(_)
            | Value::I32(_)
            | Value::I64(_)
            | Value::U8(_)
            | Value::U16(_)
            | Value::U32(_)
            | Value::U64(_)
            | Value::F32(_)
            | Value::F64(_)
            | Value::Region(_) => 0,
            Value::Str(s) => s.len(),
            Value::Array(items) => {
                std::mem::size_of::<Value>() * items.len()
                    + items.iter().map(Value::stack_size_bytes).sum::<usize>()
            }
            Value::Map(entries) => {
                std::mem::size_of::<(Value, Value)>() * entries.len()
                    + entries
                        .iter()
                        .map(|(key, value)| key.stack_size_bytes() + value.stack_size_bytes())
                        .sum::<usize>()
            }
            Value::Struct { path, fields, .. } | Value::EnumVariant { path, fields, .. } => {
                path.iter().map(String::len).sum::<usize>()
                    + fields
                        .iter()
                        .map(|(key, value)| key.len() + value.stack_size_bytes())
                        .sum::<usize>()
            }
            Value::Function(function) => {
                std::mem::size_of::<FunctionValue>()
                    + function
                        .captured
                        .iter()
                        .flat_map(|scope| scope.iter())
                        .map(|(key, value)| key.len() + value.stack_size_bytes())
                        .sum::<usize>()
            }
            Value::Handler(handler) => {
                std::mem::size_of::<HandlerValue>()
                    + std::mem::size_of::<HandlerEntry>() * handler.entries.len()
            }
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
        Value::Region(_) => 0,
        // Strings, collections, structs, and functions default to success
        Value::Str(_)
        | Value::Array(_)
        | Value::Map(_)
        | Value::Struct { .. }
        | Value::EnumVariant { .. }
        | Value::Function(_)
        | Value::Handler(_) => 0,
    }
}
