//! hir.rs
//!
//! Contains the definitions for the Hierarchical Intermediate Representation (HIR).
//! The HIR is a typed representation of the source code, created by the type checker
//! from the initial Abstract Syntax Tree (AST). Each node in the HIR, especially
//! expressions, carries resolved type information. This makes the HIR a more
//! suitable input for code generation and other analysis passes.

use crate::ast::{self, OwnedPath, Path};
use std::collections::HashMap;

//================================================================================//
//                                Core Type Definitions
//================================================================================//

/// The canonical, internal representation of a type within the compiler.
/// This enum is used throughout the HIR and later compilation stages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Bool,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    Str,
    Unit,
    Adt {
        name: OwnedPath,
        generics: Vec<Ty>,
    },
    Array(Box<Ty>),
    Map {
        key: Box<Ty>,
        value: Box<Ty>,
    },
    Function {
        param_types: Vec<Ty>,
        ret_type: Box<Ty>,
    },
    Infer(u32),
    SelfType,
    Never,
    Error,
}


#[derive(Debug, Clone)]
pub enum HirItem {
}
