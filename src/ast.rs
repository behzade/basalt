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
    ExternBlock {
        module_name: &'src str,
        functions: Vec<Function<'src>>,
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

/// An attribute, like `#[extern("wasi")]`.
#[derive(Debug, PartialEq, Clone)]
pub struct Attribute<'src> {
    pub name: &'src str,
    pub args: Vec<&'src str>,
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
    Unit,
}

/// A pattern used in `match` expressions.
/// A pattern used in `match` expressions.
#[derive(Debug, PartialEq, Clone)]
pub enum Pattern<'src> {
    /// A literal pattern, e.g., `1`, `"hello"`, `true`.
    Literal(Literal<'src>),
    /// An identifier that binds the matched value, e.g., `x`.
    Identifier(&'src str),
    /// A path to an enum variant, e.g., `Option::Some(x)`.
    Path {
        path: Path<'src>,
        args: Vec<Pattern<'src>>, // Note: args are now nested patterns
    },
    /// The wildcard pattern `_`.
    Wildcard,
}

// --- Item Definitions ---

#[derive(Debug, PartialEq, Clone)]
pub struct Function<'src> {
    pub attributes: Vec<Attribute<'src>>,
     pub name: &'src str,
    pub generics: Vec<&'src str>, // Generic type parameters (e.g., ["T", "U"])
    pub params: Vec<(Option<&'src str>, Type<'src>)>,
    pub ret_type: Option<Type<'src>>,
    pub effects: Vec<&'src str>,
    pub body: Expr<'src>, // Block expression
    pub is_public: bool,  // Whether this function is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct StructDef<'src> {
    pub attributes: Vec<Attribute<'src>>,
    pub name: &'src str,
    pub generics: Vec<&'src str>,
    pub fields: Vec<(&'src str, Type<'src>)>,
    pub is_public: bool, // Whether this struct is public/exported
}

#[derive(Debug, PartialEq, Clone)]
pub struct EnumDef<'src> {
    pub name: Option<&'src str>,  // Enums can be anonymous
    pub generics: Vec<&'src str>, // Generic type parameters (e.g., ["T", "E"])
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
    Not, // !
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
    Assign, // =
}

impl std::fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryOp::Add => write!(f, "+"),
            BinaryOp::Sub => write!(f, "-"),
            BinaryOp::Mul => write!(f, "*"),
            BinaryOp::Div => write!(f, "/"),
            BinaryOp::Eq => write!(f, "=="),
            BinaryOp::Ne => write!(f, "!="),
            BinaryOp::Lt => write!(f, "<"),
            BinaryOp::Gt => write!(f, ">"),
            BinaryOp::Assign => write!(f, "="),
        }
    }
}

// --- Owned AST Types for Symbol Signatures ---
// These are used in SymbolSignature to avoid lifetime issues

/// Owned version of Type for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedType {
    pub path: Vec<String>,
    pub generics: Vec<OwnedType>,
}

/// Owned version of Function for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedFunction {
    pub name: String,
    pub generics: Vec<String>, // Generic type parameters
    pub params: Vec<(Option<String>, OwnedType)>,
    pub ret_type: Option<OwnedType>,
    pub effects: Vec<String>,
    pub body: OwnedExpr, // Block expression
    pub is_public: bool,
}

/// Owned version of StructDef for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedStructDef {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<(String, OwnedType)>,
    pub is_public: bool,
}

/// Owned version of EnumDef for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedEnumDef {
    pub name: Option<String>,
    pub generics: Vec<String>, // Generic type parameters
    pub variants: Vec<(String, Option<Vec<OwnedType>>)>,
    pub is_public: bool,
}

/// Owned version of TraitDef for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedTraitDef {
    pub name: String,
    pub methods: Vec<OwnedTraitMethod>,
    pub is_public: bool,
}

/// Owned version of TraitMethod for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedTraitMethod {
    pub name: String,
    pub params: Vec<(Option<String>, OwnedType)>,
    pub ret_type: Option<OwnedType>,
    pub is_public: bool,
}

/// Owned version of EffectDef for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedEffectDef {
    pub name: String,
    pub operations: Vec<OwnedEffectOp>,
    pub is_public: bool,
}

/// Owned version of EffectOp for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedEffectOp {
    pub name: String,
    pub params: Vec<OwnedType>,
    pub ret_type: OwnedType,
    pub is_public: bool,
}

/// Owned version of HandlerDef for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedHandlerDef {
    pub name: String,
    pub effects: Vec<String>,
    pub functions: Vec<OwnedFunction>,
    pub is_public: bool,
}

/// Owned version of Expr for symbol signatures
#[derive(Debug, Clone)]
pub enum OwnedExpr {
    Literal(OwnedLiteral),
    Array(Vec<OwnedExpr>),
    Map(Vec<(OwnedExpr, OwnedExpr)>),
    Path(Vec<String>),
    Unary {
        op: UnaryOp,
        rhs: Box<OwnedExpr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<OwnedExpr>,
        rhs: Box<OwnedExpr>,
    },
    Call {
        fun: Box<OwnedExpr>,
        args: Vec<OwnedExpr>,
    },
    StructInit {
        path: Vec<String>,
        generics: Vec<OwnedType>,
        fields: Vec<(String, OwnedExpr)>,
    },
    Block {
        stmts: Vec<OwnedStmt>,
        last_expr: Option<Box<OwnedExpr>>,
    },
    If {
        cond: Box<OwnedExpr>,
        then_block: Box<OwnedExpr>,
        else_block: Option<Box<OwnedExpr>>,
    },
    Match {
        scrutinee: Box<OwnedExpr>,
        arms: Vec<(OwnedPattern, OwnedExpr)>,
    },
    While {
        cond: Box<OwnedExpr>,
        body: Box<OwnedExpr>,
    },
    Perform {
        path: Vec<String>,
        args: Vec<OwnedExpr>,
    },
    Handle {
        body: Box<OwnedExpr>,
        handler: OwnedHandlerBody,
    },
    Error,
}

/// Owned version of Stmt for symbol signatures
#[derive(Debug, Clone)]
pub enum OwnedStmt {
    Let {
        is_mut: bool,
        name: String,
        ty: Option<OwnedType>,
        value: OwnedExpr,
    },
    Return(Option<OwnedExpr>),
    Assign(OwnedExpr, OwnedExpr),
    Expr(OwnedExpr),
    Error,
}

/// Owned version of Pattern for symbol signatures
#[derive(Debug, Clone)]
pub enum OwnedPattern {
    Literal(OwnedLiteral),
    Identifier(String),
    Path {
        path: Vec<String>,
        args: Vec<OwnedPattern>,
    },
    Wildcard,
}

/// Owned version of Literal for symbol signatures
#[derive(Debug, Clone)]
pub enum OwnedLiteral {
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),
    Unit,
}

/// Owned version of HandlerBody for symbol signatures
#[derive(Debug, Clone)]
pub enum OwnedHandlerBody {
    Path(Vec<String>),
    Inline(Vec<OwnedFunction>),
}

// Conversion traits for owned types
impl<'src> From<&Type<'src>> for OwnedType {
    fn from(ty: &Type<'src>) -> Self {
        Self {
            path: ty.path.iter().map(|s| s.to_string()).collect(),
            generics: ty.generics.iter().map(|g| g.into()).collect(),
        }
    }
}

impl<'src> From<&Function<'src>> for OwnedFunction {
    fn from(func: &Function<'src>) -> Self {
        Self {
            name: func.name.to_string(),
            generics: func.generics.iter().map(|s| s.to_string()).collect(),
            params: func
                .params
                .iter()
                .map(|(name, ty)| (name.map(|s| s.to_string()), ty.into()))
                .collect(),
            ret_type: func.ret_type.as_ref().map(|t| t.into()),
            effects: func.effects.iter().map(|s| s.to_string()).collect(),
            body: (&func.body).into(),
            is_public: func.is_public,
        }
    }
}

impl<'src> From<&StructDef<'src>> for OwnedStructDef {
    fn from(struct_def: &StructDef<'src>) -> Self {
        Self {
            name: struct_def.name.to_string(),
            generics: struct_def.generics.iter().map(|s| s.to_string()).collect(),
            fields: struct_def
                .fields
                .iter()
                .map(|(name, ty)| (name.to_string(), ty.into()))
                .collect(),
            is_public: struct_def.is_public,
        }
    }
}

impl<'src> From<&EnumDef<'src>> for OwnedEnumDef {
    fn from(enum_def: &EnumDef<'src>) -> Self {
        Self {
            name: enum_def.name.map(|s| s.to_string()),
            generics: enum_def.generics.iter().map(|s| s.to_string()).collect(),
            variants: enum_def
                .variants
                .iter()
                .map(|(name, types)| {
                    (
                        name.to_string(),
                        types
                            .as_ref()
                            .map(|ts| ts.iter().map(|t| t.into()).collect()),
                    )
                })
                .collect(),
            is_public: enum_def.is_public,
        }
    }
}

impl<'src> From<&TraitDef<'src>> for OwnedTraitDef {
    fn from(trait_def: &TraitDef<'src>) -> Self {
        Self {
            name: trait_def.name.to_string(),
            methods: trait_def.methods.iter().map(|m| m.into()).collect(),
            is_public: trait_def.is_public,
        }
    }
}

impl<'src> From<&TraitMethod<'src>> for OwnedTraitMethod {
    fn from(method: &TraitMethod<'src>) -> Self {
        Self {
            name: method.name.to_string(),
            params: method
                .params
                .iter()
                .map(|(name, ty)| (name.map(|s| s.to_string()), ty.into()))
                .collect(),
            ret_type: method.ret_type.as_ref().map(|t| t.into()),
            is_public: method.is_public,
        }
    }
}

impl<'src> From<&EffectDef<'src>> for OwnedEffectDef {
    fn from(effect_def: &EffectDef<'src>) -> Self {
        Self {
            name: effect_def.name.to_string(),
            operations: effect_def.operations.iter().map(|op| op.into()).collect(),
            is_public: effect_def.is_public,
        }
    }
}

impl<'src> From<&EffectOp<'src>> for OwnedEffectOp {
    fn from(op: &EffectOp<'src>) -> Self {
        Self {
            name: op.name.to_string(),
            params: op.params.iter().map(|t| t.into()).collect(),
            ret_type: (&op.ret_type).into(),
            is_public: op.is_public,
        }
    }
}

impl<'src> From<&HandlerDef<'src>> for OwnedHandlerDef {
    fn from(handler_def: &HandlerDef<'src>) -> Self {
        Self {
            name: handler_def.name.to_string(),
            effects: handler_def.effects.iter().map(|s| s.to_string()).collect(),
            functions: handler_def.functions.iter().map(|f| f.into()).collect(),
            is_public: handler_def.is_public,
        }
    }
}

impl<'src> From<&Expr<'src>> for OwnedExpr {
    fn from(expr: &Expr<'src>) -> Self {
        match expr {
            Expr::Literal(lit) => OwnedExpr::Literal(lit.into()),
            Expr::Array(elements) => OwnedExpr::Array(elements.iter().map(|e| e.into()).collect()),
            Expr::Map(entries) => {
                OwnedExpr::Map(entries.iter().map(|(k, v)| (k.into(), v.into())).collect())
            }
            Expr::Path(path) => OwnedExpr::Path(path.iter().map(|s| s.to_string()).collect()),
            Expr::Unary { op, rhs } => OwnedExpr::Unary {
                op: *op,
                rhs: Box::new((&**rhs).into()),
            },
            Expr::Binary { op, lhs, rhs } => OwnedExpr::Binary {
                op: *op,
                lhs: Box::new((&**lhs).into()),
                rhs: Box::new((&**rhs).into()),
            },
            Expr::Call { fun, args } => OwnedExpr::Call {
                fun: Box::new((&**fun).into()),
                args: args.iter().map(|a| a.into()).collect(),
            },
            Expr::StructInit {
                path,
                generics,
                fields,
            } => OwnedExpr::StructInit {
                path: path.iter().map(|s| s.to_string()).collect(),
                generics: generics.iter().map(|g| g.into()).collect(),
                fields: fields
                    .iter()
                    .map(|(name, expr)| (name.to_string(), expr.into()))
                    .collect(),
            },
            Expr::Block { stmts, last_expr } => OwnedExpr::Block {
                stmts: stmts.iter().map(|s| s.into()).collect(),
                last_expr: last_expr.as_ref().map(|e| Box::new((&**e).into())),
            },
            Expr::If {
                cond,
                then_block,
                else_block,
            } => OwnedExpr::If {
                cond: Box::new((&**cond).into()),
                then_block: Box::new((&**then_block).into()),
                else_block: else_block.as_ref().map(|e| Box::new((&**e).into())),
            },
            Expr::Match { scrutinee, arms } => OwnedExpr::Match {
                scrutinee: Box::new((&**scrutinee).into()),
                arms: arms
                    .iter()
                    .map(|(pat, expr)| (pat.into(), expr.into()))
                    .collect(),
            },
            Expr::While { cond, body } => OwnedExpr::While {
                cond: Box::new((&**cond).into()),
                body: Box::new((&**body).into()),
            },
            Expr::Perform { path, args } => OwnedExpr::Perform {
                path: path.iter().map(|s| s.to_string()).collect(),
                args: args.iter().map(|a| a.into()).collect(),
            },
            Expr::Handle { body, handler } => OwnedExpr::Handle {
                body: Box::new((&**body).into()),
                handler: handler.into(),
            },
            Expr::Error => OwnedExpr::Error,
        }
    }
}

impl<'src> From<&Stmt<'src>> for OwnedStmt {
    fn from(stmt: &Stmt<'src>) -> Self {
        match stmt {
            Stmt::Let {
                is_mut,
                name,
                ty,
                value,
            } => OwnedStmt::Let {
                is_mut: *is_mut,
                name: name.to_string(),
                ty: ty.as_ref().map(|t| t.into()),
                value: value.into(),
            },
            Stmt::Return(expr) => OwnedStmt::Return(expr.as_ref().map(|_e| OwnedExpr::Error)), // Simplified for now
            Stmt::Assign(lhs, rhs) => OwnedStmt::Assign(lhs.into(), rhs.into()),
            Stmt::Expr(expr) => OwnedStmt::Expr(expr.into()),
            Stmt::Error => OwnedStmt::Error,
        }
    }
}

impl<'src> From<&Pattern<'src>> for OwnedPattern {
    fn from(pat: &Pattern<'src>) -> Self {
        match pat {
            Pattern::Literal(lit) => OwnedPattern::Literal(lit.into()),
            Pattern::Identifier(name) => OwnedPattern::Identifier(name.to_string()),
            Pattern::Path { path, args } => OwnedPattern::Path {
                path: path.iter().map(|s| s.to_string()).collect(),
                args: args.iter().map(|a| a.into()).collect(),
            },
            Pattern::Wildcard => OwnedPattern::Wildcard,
        }
    }
}

impl<'src> From<&Literal<'src>> for OwnedLiteral {
    fn from(lit: &Literal<'src>) -> Self {
        match lit {
            Literal::Bool(b) => OwnedLiteral::Bool(*b),
            Literal::I64(i) => OwnedLiteral::I64(*i),
            Literal::F64(f) => OwnedLiteral::F64(*f),
            Literal::Str(s) => OwnedLiteral::Str(s.to_string()),
            Literal::Unit => OwnedLiteral::Unit,
        }
    }
}

impl<'src> From<&HandlerBody<'src>> for OwnedHandlerBody {
    fn from(body: &HandlerBody<'src>) -> Self {
        match body {
            HandlerBody::Path(path) => {
                OwnedHandlerBody::Path(path.iter().map(|s| s.to_string()).collect())
            }
            HandlerBody::Inline(functions) => {
                OwnedHandlerBody::Inline(functions.iter().map(|f| f.into()).collect())
            }
        }
    }
}
