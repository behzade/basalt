//! Contains the definitions for the Abstract Syntax Tree (AST) of the language.
//! The AST represents the hierarchical structure of the source code, making it
//! easier to analyze, transform, and compile. The `'src` lifetime parameter
//! indicates that many AST nodes borrow directly from the source string for efficiency.
use crate::token::SimpleSpan;

// --- Spanned Wrapper and Type Aliases ---

/// A generic wrapper that adds a source code span to an AST node.
#[derive(Debug, PartialEq, Clone)]
pub struct Spanned<T> {
    pub node: T,
    pub span: SimpleSpan,
}

/// A path represents a potentially namespaced identifier, like `Std::Collections::Map`.
pub type Path<'src> = Vec<&'src str>;
pub type OwnedPath = Vec<String>;

// Core recursive AST types are now aliases for the Spanned wrapper.
pub type Item<'src> = Spanned<ItemNode<'src>>;
pub type Stmt<'src> = Spanned<StmtNode<'src>>;
pub type Expr<'src> = Spanned<ExprNode<'src>>;
pub type Pattern<'src> = Spanned<PatternNode<'src>>;
pub type Type<'src> = Spanned<TypeNode<'src>>;

// --- Core AST Node Definitions ---

/// A top-level item in a source file.
#[derive(Debug, PartialEq, Clone)]
pub enum ItemNode<'src> {
    Stmt(Stmt<'src>),
    ImportBlock { imports: Vec<ImportPath<'src>> },
    Fn(Function<'src>),
    Method(Method<'src>),
    Struct(StructDef<'src>),
    Enum(EnumDef<'src>),
    Trait(TraitDef<'src>),
    Satisfies(SatisfiesBlock<'src>),
    Effect(EffectDef<'src>),
    Handler(HandlerDef<'src>),
}

#[derive(Debug, PartialEq, Clone)]
pub struct ImportPath<'src> {
    pub path: Vec<&'src str>,
    pub alias: Option<&'src str>,
}

/// A statement within a block.
#[derive(Debug, PartialEq, Clone)]
pub enum StmtNode<'src> {
    Let {
        is_mut: bool,
        name: &'src str,
        ty: Option<Type<'src>>,
        value: Expr<'src>,
    },
    Return(Option<Expr<'src>>),
    Assign(Expr<'src>, Expr<'src>),
    Expr(Expr<'src>),
    Error,
}

/// An expression.
#[derive(Debug, PartialEq, Clone)]
pub enum ExprNode<'src> {
    Literal(Literal<'src>),
    Array(Vec<Expr<'src>>),
    Map(Vec<(Expr<'src>, Expr<'src>)>),
    Path(Path<'src>),
    FieldAccess {
        receiver: Box<Expr<'src>>,
        field: &'src str,
    },
    Unary {
        op: UnaryOp,
        rhs: Box<Expr<'src>>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr<'src>>,
        rhs: Box<Expr<'src>>,
    },
    Call {
        fun: Box<Expr<'src>>,
        args: Vec<Expr<'src>>,
    },
    StructInit {
        path: Path<'src>,
        generics: Vec<Type<'src>>,
        fields: Vec<(&'src str, Expr<'src>)>,
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
    Perform {
        path: Path<'src>,
        args: Vec<Expr<'src>>,
    },
    Handle {
        body: Box<Expr<'src>>,
        handler: HandlerBody<'src>,
    },
    Cast {
        expr: Box<Expr<'src>>,
        ty: Type<'src>,
    },
    Error,
}

/// A type annotation.
#[derive(Debug, PartialEq, Clone)]
pub struct TypeNode<'src> {
    pub path: Path<'src>,
    pub generics: Vec<Type<'src>>,
}

/// A literal value.
#[derive(Debug, PartialEq, Clone)]
pub enum Literal<'src> {
    Bool(bool),
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
    Str(&'src str),
    Unit,
}

/// A pattern used in `match` expressions.
#[derive(Debug, PartialEq, Clone)]
pub enum PatternNode<'src> {
    Literal(Literal<'src>),
    Identifier(&'src str),
    Path {
        path: Path<'src>,
        args: Vec<Pattern<'src>>,
    },
    Wildcard,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Function<'src> {
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub effects: Vec<&'src str>,
    pub body: Expr<'src>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Method<'src> {
    pub type_name: &'src str,
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub effects: Vec<&'src str>,
    pub body: Expr<'src>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructDef<'src> {
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub fields: Vec<(&'src str, Type<'src>)>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef<'src> {
    pub name: Option<&'src str>,
    pub generics: Vec<&'src str>,
    pub variants: Vec<(&'src str, Option<Vec<Type<'src>>>)>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TraitDef<'src> {
    pub name: &'src str,
    pub methods: Vec<TraitMethod<'src>>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TraitMethod<'src> {
    pub name: &'src str,
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SatisfiesBlock<'src> {
    pub target_type: Type<'src>,
    pub trait_names: Vec<&'src str>,
    pub methods: Option<Vec<Function<'src>>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EffectDef<'src> {
    pub name: &'src str,
    pub operations: Vec<EffectOp<'src>>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EffectOp<'src> {
    pub name: &'src str,
    pub params: Vec<Type<'src>>,
    pub ret_type: Type<'src>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub struct HandlerDef<'src> {
    pub name: &'src str,
    pub effects: Vec<&'src str>,
    pub functions: Vec<Function<'src>>,
    pub is_public: bool,
}

#[derive(Debug, PartialEq, Clone)]
pub enum HandlerBody<'src> {
    Path(Path<'src>),
    Inline(Vec<Function<'src>>),
}

// --- Expression Operators ---

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, PartialEq, Clone, Copy)]
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
    BinaryXor,
    BinaryAnd,
    BinaryOr,
    BitShiftLeft,
    BitShiftRight,
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
                BinaryOp::Mod => "%",
                BinaryOp::Eq => "==",
                BinaryOp::Ne => "!=",
                BinaryOp::Lt => "<",
                BinaryOp::Lte => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Gte => ">=",
                BinaryOp::Assign => "=",
                BinaryOp::And => "&&",
                BinaryOp::Or => "||",
                BinaryOp::BinaryXor => "^",
                BinaryOp::BinaryAnd => "&",
                BinaryOp::BinaryOr => "|",
                BinaryOp::BitShiftLeft => "<<",
                BinaryOp::BitShiftRight => ">>",
            }
        )
    }
}
