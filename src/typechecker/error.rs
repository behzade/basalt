//! typechecker/error.rs
//!
//! Defines the structured errors that can be produced by the type checker.

use crate::hir;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeError<'src> {
    MismatchedTypes {
        expected: hir::Ty<'src>,
        found: hir::Ty<'src>,
    },
    UnknownVariable(&'src str),
    UnknownFunction(&'src str),
    UnknownStruct(&'src str),
    UnknownEnum(&'src str),
    UnknownEnumVariant {
        enum_name: &'src str,
        variant_name: &'src str,
    },
    WrongArgumentCount {
        expected: usize,
        found: usize,
    },
    WrongNumberOfArguments {
        expected: usize,
        found: usize,
    },
    WrongArgumentType {
        expected: hir::Ty<'src>,
        found: hir::Ty<'src>,
    },
    UnknownStructField {
        struct_name: &'src str,
        field_name: &'src str,
    },
    MissingStructField {
        struct_name: &'src str,
        field_name: &'src str,
    },
    InvalidOperator {
        op: String,
        ty: hir::Ty<'src>,
    },
    InvalidPattern {
        pattern: String,
    },
    UnificationError(hir::Ty<'src>, hir::Ty<'src>),
    // New import-related errors
    UnknownModule {
        namespace: &'src str,
        module: &'src str,
    },
    UnknownModuleSymbol {
        namespace: &'src str,
        module: &'src str,
        symbol: &'src str,
    },
    MissingImport {
        symbol: &'src str,
        suggested_import: Option<String>,
    },
}

impl<'src> fmt::Display for hir::Ty<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            hir::Ty::Bool => write!(f, "bool"),
            hir::Ty::I32 => write!(f, "i32"),
            hir::Ty::I64 => write!(f, "i64"),
            hir::Ty::F64 => write!(f, "f64"),
            hir::Ty::Str => write!(f, "string"),
            hir::Ty::Unit => write!(f, "()"),
            hir::Ty::Adt { name, generics } => {
                write!(f, "{}", name.join("::"))?;
                if !generics.is_empty() {
                    write!(f, "<")?;
                    // FIX: Renamed `gen` to `g` to avoid conflict with reserved keyword.
                    for (i, g) in generics.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        write!(f, "{}", g)?;
                    }
                    write!(f, ">")?;
                }
                Ok(())
            }
            hir::Ty::Array(inner) => write!(f, "[{}]", inner),
            hir::Ty::Map { key, value } => write!(f, "Map<{}, {}>", key, value),
            hir::Ty::Function {
                param_types,
                ret_type,
            } => {
                write!(f, "fn(")?;
                for (i, param) in param_types.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", param)?;
                }
                write!(f, ") -> {}", ret_type)
            }
            hir::Ty::Infer(id) => write!(f, "?T{}", id),
            hir::Ty::Error => write!(f, "<type error>"),
        }
    }
}

impl<'src> TypeError<'src> {
    fn type_to_string(ty: &hir::Ty<'src>) -> String {
        match ty {
            hir::Ty::Bool => "bool".to_string(),
            hir::Ty::I32 => "i32".to_string(),
            hir::Ty::I64 => "i64".to_string(),
            hir::Ty::F64 => "f64".to_string(),
            hir::Ty::Str => "string".to_string(),
            hir::Ty::Unit => "()".to_string(),
            hir::Ty::Array(elem) => format!("[{}]", Self::type_to_string(elem)),
            hir::Ty::Map { key, value } => {
                format!("Map<{}, {}>", Self::type_to_string(key), Self::type_to_string(value))
            }
            hir::Ty::Adt { name, generics } => {
                if generics.is_empty() {
                    name.join("::")
                } else {
                    let generic_str = generics.iter()
                        .map(|g| Self::type_to_string(g))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("{}<{}>", name.join("::"), generic_str)
                }
            }
            hir::Ty::Function { param_types, ret_type } => {
                let param_str = param_types.iter()
                    .map(|p| Self::type_to_string(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("fn({}) -> {}", param_str, Self::type_to_string(ret_type))
            }
            hir::Ty::Infer(id) => format!("?{}", id),
            hir::Ty::Error => "error".to_string(),
        }
    }
}

impl<'src> fmt::Display for TypeError<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeError::MismatchedTypes { expected, found } => {
                write!(f, "Mismatched types: expected `{}`, found `{}`", 
                    Self::type_to_string(expected), 
                    Self::type_to_string(found))
            }
            TypeError::UnknownVariable(name) => write!(f, "Unknown variable: `{}`", name),
            TypeError::UnknownFunction(name) => write!(f, "Unknown function: `{}`", name),
            TypeError::UnknownStruct(name) => write!(f, "Unknown struct: `{}`", name),
            TypeError::UnknownEnum(name) => write!(f, "Unknown enum: `{}`", name),
            TypeError::UnknownEnumVariant { enum_name, variant_name } => {
                write!(f, "Unknown variant `{}` in enum `{}`", variant_name, enum_name)
            }
            TypeError::WrongArgumentCount { expected, found } => {
                write!(f, "Wrong number of arguments: expected {}, found {}", expected, found)
            }
            TypeError::WrongNumberOfArguments { expected, found } => {
                write!(f, "Wrong number of arguments: expected {}, found {}", expected, found)
            }
            TypeError::WrongArgumentType { expected, found } => {
                write!(f, "Wrong argument type: expected `{}`, found `{}`", 
                    Self::type_to_string(expected), 
                    Self::type_to_string(found))
            }
            TypeError::UnknownStructField { struct_name, field_name } => {
                write!(f, "Unknown field `{}` in struct `{}`", field_name, struct_name)
            }
            TypeError::MissingStructField { struct_name, field_name } => {
                write!(f, "Missing field `{}` in struct `{}`", field_name, struct_name)
            }
            TypeError::InvalidOperator { op, ty } => {
                write!(f, "Cannot apply operator `{}` to type `{}`", op, Self::type_to_string(ty))
            }
            TypeError::InvalidPattern { pattern } => {
                write!(f, "Invalid pattern: {}", pattern)
            }
            TypeError::UnificationError(ty1, ty2) => {
                write!(f, "Cannot unify types `{}` and `{}`", 
                    Self::type_to_string(ty1), 
                    Self::type_to_string(ty2))
            }
            TypeError::UnknownModule { namespace, module } => {
                write!(f, "Unknown module `{}::{}`", namespace, module)
            }
            TypeError::UnknownModuleSymbol { namespace, module, symbol } => {
                write!(f, "Unknown symbol `{}` in module `{}::{}`", symbol, namespace, module)
            }
            TypeError::MissingImport { symbol, suggested_import } => {
                if let Some(suggestion) = suggested_import {
                    write!(f, "Unknown symbol `{}`. Try importing it: {}", symbol, suggestion)
                } else {
                    write!(f, "Unknown symbol `{}`", symbol)
                }
            }
        }
    }
}
