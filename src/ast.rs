use chumsky::span::SimpleSpan;

use std::collections::HashMap;

pub type Span = SimpleSpan;
#[derive(Debug, PartialEq, Clone)]
pub struct Spanned<T>(pub T, pub Span);

//==================================================================================================
//
//  Program & Statements
//
//==================================================================================================

/// The root of the AST, representing a whole program file.
#[derive(Debug)]
pub struct Program<'src> {
    pub items: Vec<Spanned<Item<'src>>>,
}

/// Statements can be item definitions, imports, or expressions.
#[derive(Debug)]
pub enum Stmt<'src> {
    Expr(Spanned<Expr<'src>>),
    Item(Spanned<Item<'src>>),
    Import(Spanned<Import<'src>>),
}

/// An import statement, e.g., `import Std::Collections::Map as MyMap;`
#[derive(Debug)]
pub struct Import<'src> {
    pub path: Spanned<Path<'src>>,
    pub alias: Option<&'src str>,
}

//==================================================================================================
//
//  Top-Level Items
//
//==================================================================================================

/// Top-level items like structs, functions, enums, traits, etc.
#[derive(Debug)]
pub enum Item<'src> {
    Struct(StructDef<'src>),
    Enum(EnumDef<'src>),
    Trait(TraitDef<'src>),
    Impl(ImplDef<'src>),
    Function(FunctionDef<'src>),
    Effect(EffectDef<'src>),
}

/// `struct MyStruct<T> { field: T }`
#[derive(Debug)]
pub struct StructDef<'src> {
    pub name: &'src str,
    pub generic_params: Option<Vec<&'src str>>,
    pub fields: Vec<(&'src str, Spanned<Type<'src>>)>,
}

/// `enum MyEnum { VariantA, VariantB(i64) }`
#[derive(Debug)]
pub struct EnumDef<'src> {
    pub name: &'src str,
    pub variants: Vec<EnumVariant<'src>>,
}

/// A variant within an enum definition.
#[derive(Debug)]
pub enum EnumVariant<'src> {
    Unit(&'src str),
    Tuple(&'src str, Vec<Spanned<Type<'src>>>),
}

/// `trait MyTrait { fn do_something(&self) -> bool; }`
#[derive(Debug)]
pub struct TraitDef<'src> {
    pub name: &'src str,
    pub methods: Vec<Spanned<FunctionSignature<'src>>>,
}

/// `impl MyTrait for MyType { ... }` or `impl MyType { ... }`
#[derive(Debug)]
pub struct ImplDef<'src> {
    pub trait_name: Option<Spanned<Path<'src>>>,
    pub for_type: Spanned<Type<'src>>,
    pub methods: Vec<Spanned<FunctionDef<'src>>>,
}

/// `fn my_func(a: i64) -> i64 { ... }` or `extern fn my_func(a: i64) -> i64;`
#[derive(Debug)]
pub struct FunctionDef<'src> {
    pub name: &'src str,
    pub is_extern: bool,
    pub signature: FunctionSignature<'src>,
    pub body: Option<Spanned<Expr<'src>>>,
}

/// The signature of a function, used in `fn` and `trait` definitions.
#[derive(Debug)]
pub struct FunctionSignature<'src> {
    pub params: Vec<(&'src str, Spanned<Type<'src>>)>,
    pub return_type: Spanned<Type<'src>>,
    pub effects: Option<Vec<Spanned<Path<'src>>>>,
}

/// `effect MyEffect { operation(String) -> None }`
#[derive(Debug)]
pub struct EffectDef<'src> {
    pub name: &'src str,
    pub operations: Vec<EffectOperation<'src>>,
}

/// A single operation within an effect definition.
#[derive(Debug)]
pub struct EffectOperation<'src> {
    pub name: &'src str,
    pub params: Vec<Spanned<Type<'src>>>,
    pub return_type: Spanned<Type<'src>>,
}

//==================================================================================================
//
//  Expressions
//
//==================================================================================================

/// The main expression enum, covering all expression types.
#[derive(Debug)]
pub enum Expr<'src> {
    Literal(Literal<'src>),
    Path(Path<'src>),
    Block {
        stmts: Vec<Spanned<Stmt<'src>>>,
        last_expr: Option<Box<Spanned<Expr<'src>>>>,
    },
    Unary {
        op: UnaryOp,
        right: Box<Spanned<Expr<'src>>>,
    },
    Binary(BinaryOperation<'src>),
    Index {
        left: Box<Spanned<Expr<'src>>>,
        index: Box<Spanned<Expr<'src>>>,
    },
    Let(VariableDeclaration<'src>),
    FunctionCall(FunctionCall<'src>),
    StructInstantiation(StructInstantiation<'src>),
    MemberAccess(MemberAccess<'src>),
    If(If<'src>),
    For(For<'src>),
    Match(Match<'src>),
    Handle(Handle<'src>),
    Perform(Perform<'src>),
    Return(Option<Box<Spanned<Expr<'src>>>>),
}

/// Supporting structs for `Expr` variants.

#[derive(Debug)]
pub struct VariableDeclaration<'src> {
    pub mutable: bool,
    pub name: &'src str,
    pub type_annotation: Option<Spanned<Type<'src>>>,
    pub value: Box<Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub struct FunctionCall<'src> {
    pub callee: Box<Spanned<Expr<'src>>>,
    pub generic_args: Option<Vec<Spanned<Type<'src>>>>,
    pub args: Vec<Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub struct StructInstantiation<'src> {
    pub name: Spanned<Path<'src>>,
    pub generic_args: Option<Vec<Spanned<Type<'src>>>>,
    pub fields: HashMap<&'src str, Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub struct MemberAccess<'src> {
    pub object: Box<Spanned<Expr<'src>>>,
    pub member: &'src str,
}

#[derive(Debug)]
pub struct BinaryOperation<'src> {
    pub left: Box<Spanned<Expr<'src>>>,
    pub op: BinaryOp,
    pub right: Box<Spanned<Expr<'src>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg, // Negation (-)
    Not, // Logical Not (!)
}

#[derive(Debug)]
pub struct If<'src> {
    pub condition: Box<Spanned<Expr<'src>>>,
    pub then_block: Box<Spanned<Expr<'src>>>,
    pub else_block: Option<Box<Spanned<Expr<'src>>>>,
}

#[derive(Debug)]
pub struct For<'src> {
    pub condition: Box<Spanned<Expr<'src>>>,
    pub body: Box<Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub struct Match<'src> {
    pub expression: Box<Spanned<Expr<'src>>>,
    pub arms: Vec<MatchArm<'src>>,
}

#[derive(Debug)]
pub struct MatchArm<'src> {
    pub pattern: Pattern<'src>,
    pub expression: Box<Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub enum Pattern<'src> {
    Path(Path<'src>),
    Tuple(Vec<&'src str>),
    Wildcard,
}

#[derive(Debug)]
pub struct Handle<'src> {
    pub expression: Box<Spanned<Expr<'src>>>,
    pub handler: Spanned<Handler<'src>>,
}

#[derive(Debug)]
pub struct Handler<'src> {
    pub effect: Option<Spanned<Path<'src>>>,
    pub arms: Vec<HandlerArm<'src>>,
}

#[derive(Debug)]
pub struct HandlerArm<'src> {
    pub name: &'src str,
    pub params: Vec<(&'src str, Spanned<Type<'src>>)>,
    pub return_type: Spanned<Type<'src>>,
    pub body: Box<Spanned<Expr<'src>>>,
}

#[derive(Debug)]
pub struct Perform<'src> {
    pub effect: Spanned<Path<'src>>,
    pub args: Vec<Spanned<Expr<'src>>>,
}

//==================================================================================================
//
//  Literals, Types, and Paths
//
//==================================================================================================

/// AST nodes for literals like numbers, strings, arrays, and maps.
#[derive(Debug)]
pub enum Literal<'src> {
    I64(i64),
    F64(f64),
    Bool(bool),
    Str(&'src str),
    Array(Vec<Spanned<Expr<'src>>>),
    Map(Vec<(Spanned<Expr<'src>>, Spanned<Expr<'src>>)>),
}

/// AST nodes for types.
#[derive(Debug, Clone)]
pub enum Type<'src> {
    I64,
    F64,
    Bool,
    StringType,
    Array(Box<Spanned<Type<'src>>>),
    Map(Box<Spanned<Type<'src>>>, Box<Spanned<Type<'src>>>),
    Custom(Path<'src>),
    Generic(&'src str),
    None, // Represents the absence of a return type, e.g., in functions.
}

/// AST node for paths like `Std::Collections::Map`.
#[derive(Debug, Clone)]
pub enum Path<'src> {
    Identifier(&'src str),
    Namespaced(Box<Spanned<Path<'src>>>, &'src str),
}
