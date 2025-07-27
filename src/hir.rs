//! hir.rs
//!
//! Contains the definitions for the Hierarchical Intermediate Representation (HIR).
//! The HIR is a typed representation of the source code, created by the type checker
//! from the initial Abstract Syntax Tree (AST). Each node in the HIR, especially
//! expressions, carries resolved type information. This makes the HIR a more
//! suitable input for code generation and other analysis passes.

use crate::ast::{self, Path};
use std::collections::HashMap;

//================================================================================//
//                                Core Type Definitions
//================================================================================//

/// The canonical, internal representation of a type within the compiler.
/// This enum is used throughout the HIR and later compilation stages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty<'src> {
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
        name: Path<'src>,
        generics: Vec<Ty<'src>>,
    },
    Array(Box<Ty<'src>>),
    Map {
        key: Box<Ty<'src>>,
        value: Box<Ty<'src>>,
    },
    Function {
        param_types: Vec<Ty<'src>>,
        ret_type: Box<Ty<'src>>,
    },
    Infer(u32),
    Error,
}

#[derive(Debug, Clone)]
pub enum Item<'src> {
    Stmt(Stmt<'src>),
    Import {
        path: Path<'src>,
        alias: Option<&'src str>,
    },
    ExternFn {
        name: &'src str,
        params: Vec<(Option<&'src str>, Ty<'src>)>,
        ret_type: Ty<'src>,
    },
    ExternBlock {
        module_name: &'src str,
        functions: Vec<ast::Function<'src>>,
    },
    Fn(Function<'src>),
    Struct(StructDef<'src>),
    Enum(EnumDef<'src>),
    Trait(TraitDef<'src>),
    Impl(ImplBlock<'src>),
    Effect(EffectDef<'src>),
    Handler(HandlerDef<'src>),
}

/// A fully typed function definition.
#[derive(Debug, Clone)]
pub struct Function<'src> {
    pub name: &'src str,
    pub params: Vec<(Option<&'src str>, Ty<'src>)>, // Parameters are now typed with hir::Ty
    pub ret_type: Ty<'src>,                         // Return type is also hir::Ty
    pub body: Expr<'src>,                           // The body is a typed HIR expression
    pub is_public: bool,
}

/// A struct definition, using `hir::Ty` for its fields.
#[derive(Debug, Clone)]
pub struct StructDef<'src> {
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub fields: Vec<(&'src str, Ty<'src>)>,
    pub is_public: bool,
}

/// An enum definition, using `hir::Ty` for its variants.
#[derive(Debug, Clone)]
pub struct EnumDef<'src> {
    pub name: Option<&'src str>,
    pub generics: Vec<&'src str>, // Generic type parameters
    pub variants: Vec<(&'src str, Option<Vec<Ty<'src>>>)>,
    pub is_public: bool,
}

/// A trait definition, using `hir::Ty` for its method signatures.
#[derive(Debug, Clone)]
pub struct TraitDef<'src> {
    pub name: &'src str,
    pub methods: Vec<TraitMethod<'src>>,
    pub is_public: bool,
}

/// A trait method signature, using `hir::Ty` for types.
#[derive(Debug, Clone)]
pub struct TraitMethod<'src> {
    pub name: &'src str,
    pub params: Vec<(Option<&'src str>, Ty<'src>)>,
    pub ret_type: Ty<'src>,
    pub is_public: bool,
}

/// An implementation block, using `hir::Ty` for types.
#[derive(Debug, Clone)]
pub struct ImplBlock<'src> {
    pub trait_name: &'src str,
    pub target_type: Ty<'src>,
    pub methods: Vec<Function<'src>>,
}

/// An effect definition, using `hir::Ty` for operation signatures.
#[derive(Debug, Clone)]
pub struct EffectDef<'src> {
    pub name: &'src str,
    pub operations: Vec<EffectOp<'src>>,
    pub is_public: bool,
}

/// An effect operation signature, using `hir::Ty` for types.
#[derive(Debug, Clone)]
pub struct EffectOp<'src> {
    pub name: &'src str,
    pub params: Vec<Ty<'src>>,
    pub ret_type: Ty<'src>,
    pub is_public: bool,
}

/// A handler definition, using `hir::Ty` for types.
#[derive(Debug, Clone)]
pub struct HandlerDef<'src> {
    pub name: &'src str,
    pub effects: Vec<&'src str>,
    pub functions: Vec<Function<'src>>,
    pub is_public: bool,
}

/// A handler body, either a path or inline functions.
#[derive(Debug, Clone)]
pub enum HandlerBody<'src> {
    Path(Path<'src>),
    Inline(Vec<Function<'src>>),
}

//================================================================================//
//                            Statements and Expressions
//================================================================================//

/// An HIR statement.
#[derive(Debug, Clone)]
pub enum Stmt<'src> {
    Let {
        name: &'src str,
        is_mut: bool,
        value_ty: Ty<'src>, // The type of the variable
        value: Expr<'src>,  // The typed expression being assigned
    },
    Return(Option<Expr<'src>>),
    Assign(Expr<'src>, Expr<'src>),
    Expr(Expr<'src>), // A standalone expression statement
}

/// An HIR expression. Every node carries its resolved type.
#[derive(Debug, Clone)]
pub struct Expr<'src> {
    pub kind: ExprKind<'src>,
    pub ty: Ty<'src>, // The resolved type of this expression
                      // pub span: Span, // It's highly recommended to carry spans forward for better error messages
}

/// The different kinds of expressions in the HIR.
/// This mirrors `ast::Expr` but contains `hir::Expr` nodes as children.
#[derive(Debug, Clone)]
pub enum ExprKind<'src> {
    Literal(ast::Literal<'src>),
    /// A local variable or function reference
    Path(Path<'src>),
    /// An enum variant reference (e.g., Color::Red)
    EnumVariant {
        enum_name: &'src str,
        variant_name: &'src str,
    },
    /// A module-qualified path (e.g., Std::print)
    ModulePath {
        module: &'src str,
        symbol: &'src str,
    },
    FieldAccess {
        receiver: Box<Expr<'src>>,
        field: &'src str,
    },
    Unary {
        op: ast::UnaryOp,
        rhs: Box<Expr<'src>>,
    },
    Binary {
        op: ast::BinaryOp,
        lhs: Box<Expr<'src>>,
        rhs: Box<Expr<'src>>,
    },
    Call {
        fun: Box<Expr<'src>>,
        args: Vec<Expr<'src>>,
    },
    StructInit {
        path: Path<'src>,
        fields: HashMap<&'src str, Expr<'src>>,
    },
    Block {
        stmts: Vec<Stmt<'src>>,
        last_expr: Option<Box<Expr<'src>>>,
    },
    If {
        cond: Box<Expr<'src>>,
        then_block: Box<Expr<'src>>,
        else_block: Option<Box<Expr<'src>>>,
    },
    Match {
        scrutinee: Box<Expr<'src>>,
        arms: Vec<(Pattern<'src>, Expr<'src>)>,
    },
    While {
        cond: Box<Expr<'src>>,
        body: Box<Expr<'src>>,
    },
    Array(Vec<Expr<'src>>),
    Map(Vec<(Expr<'src>, Expr<'src>)>),
    Perform {
        path: Path<'src>,
        args: Vec<Expr<'src>>,
    },
    Handle {
        body: Box<Expr<'src>>,
        handler: HandlerBody<'src>,
    },
}

//================================================================================//
//                                     Patterns
//================================================================================//

/// A pattern used in `match` expressions. It is typed to ensure correctness.
#[derive(Debug, Clone)]
pub struct Pattern<'src> {
    pub kind: PatternKind<'src>,
    pub ty: Ty<'src>, // The type of the value the whole pattern matches
}

#[derive(Debug, Clone)]
pub enum PatternKind<'src> {
    /// A literal pattern, e.g., `1`, `"hello"`, `true`.
    Literal(ast::Literal<'src>),
    /// An identifier that binds the matched value to a new variable, e.g., `x`.
    Binding { name: &'src str, is_mut: bool },
    /// A path to an enum variant, e.g., `Option::Some(x)`.
    AdtVariant {
        path: Path<'src>,
        fields: Vec<Pattern<'src>>,
    },
    /// The wildcard pattern `_`.
    Wildcard,
}
