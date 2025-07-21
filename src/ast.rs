//! Contains the definitions for the Abstract Syntax Tree (AST) of the language.
//! The AST represents the hierarchical structure of the source code, making it
//! easier to analyze, transform, and compile. The `'src` lifetime parameter
//! indicates that many AST nodes borrow directly from the source string for efficiency.

// --- Core Types ---

/// A path represents a potentially namespaced identifier, like `Std::Collections::Map`.
pub type Path<'src> = Vec<&'src str>;

/// A top-level item in a source file.
#[derive(Debug, PartialEq, Clone)]
pub enum Item<'src> {
    Stmt(Stmt<'src>),
    Import {
        path: Path<'src>,
        alias: Option<&'src str>,
    },
    ExternFn {
        name: &'src str,
        params: Vec<(Option<&'src str>, Type<'src>)>,
        ret_type: Type<'src>,
    },
    Fn(Function<'src>),
    Struct(StructDef<'src>),
    Enum(EnumDef<'src>),
    Trait(TraitDef<'src>),
    Impl(ImplBlock<'src>),
    Effect(EffectDef<'src>),
    Handler(HandlerDef<'src>),
}

/// A statement within a block.
#[derive(Debug, PartialEq, Clone)]
pub enum Stmt<'src> {
    Let {
        is_mut: bool,
        name: &'src str,
        ty: Option<Type<'src>>,
        value: Expr<'src>,
    },
    Return(Option<Expr<'src>>),
    Assign(Expr<'src>, Expr<'src>),
    Expr(Expr<'src>),
    // Used for recovery; represents a statement that couldn't be parsed.
    Error,
}

/// An expression with optional type annotation.
#[derive(Debug, PartialEq, Clone)]
pub struct TypedExpr<'src> {
    pub expr: Expr<'src>,
    pub inferred_type: Option<Type<'src>>,
}

/// An expression.
#[derive(Debug, PartialEq, Clone)]
pub enum Expr<'src> {
    Literal(Literal<'src>),
    Array(Vec<Expr<'src>>),
    Map(Vec<(Expr<'src>, Expr<'src>)>),
    Path(Path<'src>),
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
        then_block: Box<Expr<'src>>, // Blocks are expressions
        else_block: Option<Box<Expr<'src>>>,
    },
    Match {
        scrutinee: Box<Expr<'src>>,
        arms: Vec<(Pattern<'src>, Expr<'src>)>,
    },
    While {
        cond: Box<Expr<'src>>,
        body: Box<Expr<'src>>, // Block expression
    },
    Perform {
        path: Path<'src>,
        args: Vec<Expr<'src>>,
    },
    Handle {
        body: Box<Expr<'src>>,
        handler: HandlerBody<'src>,
    },
    // Used for recovery; represents an expression that couldn't be parsed.
    Error,
}

/// A type annotation.
#[derive(Debug, PartialEq, Clone)]
pub struct Type<'src> {
    pub path: Path<'src>,
    pub generics: Vec<Type<'src>>,
}

/// A literal value.
#[derive(Debug, PartialEq, Clone)]
pub enum Literal<'src> {
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'src str),
}

/// A pattern used in `match` expressions.
#[derive(Debug, PartialEq, Clone)]
pub struct Pattern<'src> {
    pub path: Path<'src>,
    pub args: Vec<&'src str>,
}

// --- Item Definitions ---

#[derive(Debug, PartialEq, Clone)]
pub struct Function<'src> {
    pub name: &'src str,
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub effects: Vec<&'src str>,
    pub body: Expr<'src>, // Block expression
    pub is_public: bool, // Whether this function is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructDef<'src> {
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub fields: Vec<(&'src str, Type<'src>)>,
    pub is_public: bool, // Whether this struct is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef<'src> {
    pub name: Option<&'src str>, // Enums can be anonymous
    pub variants: Vec<(&'src str, Option<Vec<Type<'src>>>)>,
    pub is_public: bool, // Whether this enum is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct TraitDef<'src> {
    pub name: &'src str,
    pub methods: Vec<TraitMethod<'src>>,
    pub is_public: bool, // Whether this trait is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct TraitMethod<'src> {
    pub name: &'src str,
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub is_public: bool, // Whether this method is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct ImplBlock<'src> {
    pub trait_name: &'src str,
    pub target_type: Type<'src>,
    pub methods: Vec<Function<'src>>,
}

#[derive(Debug, PartialEq, Clone)]
pub struct EffectDef<'src> {
    pub name: &'src str,
    pub operations: Vec<EffectOp<'src>>,
    pub is_public: bool, // Whether this effect is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct EffectOp<'src> {
    pub name: &'src str,
    pub params: Vec<Type<'src>>,
    pub ret_type: Type<'src>,
    pub is_public: bool, // Whether this operation is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct HandlerDef<'src> {
    pub name: &'src str,
    pub effects: Vec<&'src str>,
    pub functions: Vec<Function<'src>>,
    pub is_public: bool, // Whether this handler is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub enum HandlerBody<'src> {
    Path(Path<'src>),
    Inline(Vec<Function<'src>>),
}

// --- Expression Operators ---

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum UnaryOp {
    Neg, // -
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum BinaryOp {
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Eq,  // ==
    Ne,  // !=
    Lt,  // <
    Gt,  // >
}
