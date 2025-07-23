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
