//! hir.rs
//!
//! Contains the definitions for the Hierarchical Intermediate Representation (HIR).
//! The HIR is a typed representation of the source code, created by the type checker
//! from the initial Abstract Syntax Tree (AST). Each node in the HIR, especially
//! expressions, carries resolved type information. This makes the HIR a more
//! suitable input for code generation and other analysis passes.

use std::path::PathBuf;

//================================================================================//
//                                Core Type Definitions
//================================================================================//

/// A fully-qualified path to an item.
pub type OwnedPath = Vec<String>;

/// The canonical, internal representation of a type within the compiler.
/// This enum is used throughout the HIR and later compilation stages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Special(SpecialTy),
    Primitive(PrimitiveTy),
    Adt(AdtTy),
    Array(Box<Ty>),
    Map {
        key: Box<PrimitiveTy>,
        value: Box<Ty>,
    },
    Function {
        param_types: Vec<Ty>,
        ret_type: Box<Ty>,
        effects: Vec<Ty>, // Represents the canonical effect types
    },
    // A placeholder for generic types like `T` before monomorphization
    Generic(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpecialTy {
    Unit,     // The `()` type.
    Never,    // The `!` type, for functions that never return.
    SelfType, // The `Self` type.
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveTy {
    Bool,
    Byte,
    I32,
    I64,
    F64,
    Str,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AdtTy {
    Struct { name: OwnedPath, generics: Vec<Ty> },
    Enum { name: OwnedPath, generics: Vec<Ty> },
    Trait { name: OwnedPath, generics: Vec<Ty> },
    Effect { name: OwnedPath, generics: Vec<Ty> },
}

//================================================================================//
//                                HIR Item Definitions
//================================================================================//

/// Represents a top-level item in a module.
#[derive(Debug, Clone)]
pub enum Item {
    Fn(HirFunction),
    Struct(HirStructDef),
    Enum(HirEnumDef),
    TypeAlias(HirTypeAlias),
    Trait(HirTraitDef),
    Effect(HirEffectDef),
    Impl(HirImplBlock),
    Handler(HirHandlerDef),
    // Note: Imports, externs, etc., are often resolved before HIR generation.
}

#[derive(Debug, Clone)]
pub struct HirFunction {
    pub signature: HirFunctionSignature,
    pub body: HirBlock,
    pub is_public: bool,
    /// Absolute path to the file where this function is defined
    pub defined_in: PathBuf,
    /// Token-index span covering the function item (best-effort)
    pub span: crate::token::SimpleSpan,
    /// Context for this function's parameters and local variables
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirFunctionSignature {
    pub name: String,
    pub params: Vec<HirParam>, // Name and resolved type
    pub ret_type: Ty,
    pub effects: Vec<Ty>, // List of canonical effect types this function can perform
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirParam {
    pub name: String,
    pub ty: Ty,
    pub span: Option<crate::token::SimpleSpan>,
}

#[derive(Debug, Clone)]
pub struct HirStructDef {
    pub name: String,
    pub fields: Vec<HirField>,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
    /// Context for this struct's field declarations
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirField {
    pub name: String,
    pub ty: Ty,
    pub name_span: Option<crate::token::SimpleSpan>,
}

#[derive(Debug, Clone)]
pub struct HirEnumDef {
    pub name: String,
    pub variants: Vec<HirEnumVariant>,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
    /// Context for this enum's variant declarations
    pub context_id: Option<ContextId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HirEnumVariant {
    pub name: String,
    pub payload: Option<Vec<Ty>>,
    pub name_span: Option<crate::token::SimpleSpan>,
}

#[derive(Debug, Clone)]
pub struct HirTypeAlias {
    pub name: String,
    pub aliased: Ty,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
}

#[derive(Debug, Clone)]
pub struct HirTraitDef {
    pub name: String,
    pub methods: Vec<HirFunctionSignature>,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
}

#[derive(Debug, Clone)]
pub struct HirImplBlock {
    pub trait_path: Option<OwnedPath>, // The trait being implemented, if any.
    pub target_type: Ty,               // The type the trait is implemented for.
    pub methods: Vec<HirFunction>,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
}

#[derive(Debug, Clone)]
pub struct HirEffectDef {
    pub name: String,
    pub operations: Vec<HirFunctionSignature>,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
}

#[derive(Debug, Clone)]
pub struct HirHandlerDef {
    pub name: String,
    pub effects: Vec<Ty>,
    pub functions: Vec<HirFunction>,
    pub is_public: bool,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
}

//================================================================================//
//                            Statement & Expression HIR
//================================================================================//

/// A statement in a block.
#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        value: Option<Expr>,
        ty: Ty, // Type is resolved and non-optional
        is_mut: bool,
        span: crate::token::SimpleSpan,
        /// Span of the variable identifier token
        name_span: Option<crate::token::SimpleSpan>,
    },
    Return { value: Option<Expr>, span: crate::token::SimpleSpan },
    Assign { lhs: Expr, rhs: Expr, span: crate::token::SimpleSpan }, // lhs (e.g., path or field access) and rhs
    Expr { expr: Expr, span: crate::token::SimpleSpan },
    Error { span: crate::token::SimpleSpan },
}

/// A block of code, which has a list of statements and an optional final expression.
#[derive(Debug, Clone)]
pub struct HirBlock {
    pub stmts: Vec<Stmt>,
    pub last_expr: Option<Box<Expr>>,
    pub ty: Ty, // The type of the value the block evaluates to
}

/// An expression, which always has a resolved type.
#[derive(Debug, Clone)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
    pub span: crate::token::SimpleSpan,
    /// Optional semantic resolution for goto/hover without re-resolving
    pub resolution: Option<Resolution>,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Literal(PrimitiveTy, String), // e.g., (PrimitiveTy::I32, "123")
    Array(Vec<Expr>),
    Map(Vec<(Expr, Expr)>),
    Path(OwnedPath),
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
    },
    Unary {
        op: UnaryOp,
        rhs: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Call {
        fun: Box<Expr>,
        args: Vec<Expr>,
    },
    StructInit {
        path: OwnedPath,
        fields: Vec<(String, Expr)>,
    },
    Block(HirBlock),
    If {
        cond: Box<Expr>,
        then_block: HirBlock,
        else_block: Option<Box<Expr>>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<(HirPattern, Expr)>,
    },
    While {
        cond: Box<Expr>,
        body: HirBlock,
    },
    Perform {
        path: OwnedPath, // Path to the effect operation
        args: Vec<Expr>,
    },
    Handle {
        body: HirBlock,
        handler: HirHandlerBody,
    },
    Cast {
        expr: Box<Expr>, // The expression being cast
                         // The target type is in the parent Expr's `ty` field
    },
    Error,
}

/// Semantic resolution info attached to expressions when available
#[derive(Debug, Clone)]
pub enum Resolution {
    /// Reference to a local binding (including parameters); name and declaration span
    Local { name: String, decl_span: Option<crate::token::SimpleSpan> },
    /// Access to a struct field; owner type path and field name
    Field { owner: OwnedPath, field: String },
    /// Reference to a top-level function; file and item span
    Function { defined_in: std::path::PathBuf, span: crate::token::SimpleSpan },
    /// Reference to a method function in an impl; file and item span
    Method { defined_in: std::path::PathBuf, span: crate::token::SimpleSpan },
}

#[derive(Debug, Clone)]
pub enum HirHandlerBody {
    Path(OwnedPath), // A reference to a top-level handler
    Inline(Vec<HirFunction>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Negate, // -
    Not,    // !
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Assign,
    And,
    Or,
    Xor,
    BitShiftLeft,
    BitShiftRight,
}

//================================================================================//
//                                  Pattern HIR
//================================================================================//

/// A pattern used in `match` arms and `let` bindings, with a resolved type.
#[derive(Debug, Clone)]
pub struct HirPattern {
    pub kind: HirPatternKind,
    pub ty: Ty,
}

//================================================================================//
//                                 Context & Symbols
//================================================================================//

pub type ContextId = usize;

#[derive(Debug, Clone)]
pub enum HirContextKind {
    Module,
    Function,
    ImplMethod,
    Struct,
    Enum,
    Trait,
    Effect,
    HandlerFunction,
    Block,
}

#[derive(Debug, Clone)]
pub enum HirSymbolKind {
    Type,
    TypeAlias,
    Struct,
    Enum,
    Trait,
    Effect,
    Function,
    Method,
    Param,
    Variable,
    Field,
    EnumVariant,
}

#[derive(Debug, Clone)]
pub struct HirSymbolDecl {
    pub name: String,
    pub kind: HirSymbolKind,
    pub ty: Option<Ty>,
    pub is_mut: Option<bool>,
    /// Token-index span for the declaration (item/name)
    pub span: crate::token::SimpleSpan,
    pub name_span: Option<crate::token::SimpleSpan>,
}

#[derive(Debug, Clone)]
pub struct HirContext {
    pub id: ContextId,
    pub parent: Option<ContextId>,
    pub kind: HirContextKind,
    pub defined_in: PathBuf,
    pub span: crate::token::SimpleSpan,
    pub symbols: Vec<HirSymbolDecl>,
    pub children: Vec<ContextId>,
}

#[derive(Debug, Clone)]
pub enum HirPatternKind {
    Literal(PrimitiveTy, String),
    Identifier(String),
    Path {
        path: OwnedPath,
        args: Vec<HirPattern>,
    },
    Wildcard,
}
