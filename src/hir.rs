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
    SelfType,
    Never,
    Error,
}

#[derive(Debug, Clone)]
pub enum Item {
    // Stmt is fundamental, but its inner expressions are desugared.
    Stmt(Stmt),

    // Functions are heavily annotated with resolved types.
    Fn(Function),

    // Structs and enums are kept, with resolved types for all fields.
    Struct(HirStructDef),
    // TODO: Enums: will be added in second phase

    // // TODO: Traits and impls will be added in third phase
    // Trait(HirTraitDef),
    // Impl(HirImplBlock),

    // TODO: Effects and handler will be added in fourth phase
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: OwnedPath,
        value: Expr,
        ty: Ty,
    },
    Return(Option<Expr>),
    Assign(Expr, Expr),
    Expr(Expr),
    Error,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(ast::OwnedLiteral),
    Path(OwnedPath),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: OwnedPath,
    pub params: Vec<(OwnedPath, Ty)>,
    pub ret_type: Ty,
    pub body: HirBlock,
    pub is_public: bool,
}

#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<Stmt>,
    pub last_expr: Option<Box<Expr>>,
}

#[derive(Debug, Clone)]
pub struct HirStructDef {
    pub name: OwnedPath,
    pub fields: Vec<(OwnedPath, Ty)>,
}
