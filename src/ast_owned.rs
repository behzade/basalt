// --- Owned AST Types for Symbol Signatures ---
// These are used in SymbolSignature to avoid lifetime issues
//
use crate::ast::*;

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

/// Owned version of Method for symbol signatures
#[derive(Debug, Clone)]
pub struct OwnedMethod {
    pub type_name: String, // The type this method belongs to (e.g., "Counter")
    pub name: String,      // The method name (e.g., "new")
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

#[derive(Debug, Clone)]
pub struct OwnedImplBlock {
    pub trait_name: String,
    pub target_type: OwnedType,
    pub methods: Vec<OwnedFunction>,
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
    FieldAccess {
        receiver: Box<OwnedExpr>,
        field: String,
    },
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
    Cast {
        expr: Box<OwnedExpr>,
        ty: OwnedType,
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
    I32(i32),
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

#[derive(Debug, Clone)]
pub enum OwnedItem {
    Stmt(OwnedStmt),
    ImportBlock {
        imports: Vec<OwnedImportPath>,
    },
    ExternBlock {
        module_name: String,
        functions: Vec<OwnedFunction>,
    },
    Fn(OwnedFunction),
    Method(OwnedMethod),
    Struct(OwnedStructDef),
    Enum(OwnedEnumDef),
    Trait(OwnedTraitDef),
    Effect(OwnedEffectDef),
    Handler(OwnedHandlerDef),
    Satisfies(OwnedSatisfiesBlock),
}

#[derive(Debug, Clone)]
pub struct OwnedImportPath {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OwnedSatisfiesBlock {
    pub target_type: OwnedType,
    pub trait_names: Vec<String>,
    pub methods: Option<Vec<OwnedFunction>>,
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

impl<'src> From<&Method<'src>> for OwnedMethod {
    fn from(method: &Method<'src>) -> Self {
        Self {
            type_name: method.type_name.to_string(),
            name: method.name.to_string(),
            generics: method.generics.iter().map(|s| s.to_string()).collect(),
            params: method
                .params
                .iter()
                .map(|(name, ty)| (name.map(|s| s.to_string()), ty.into()))
                .collect(),
            ret_type: method.ret_type.as_ref().map(|ty| ty.into()),
            effects: method.effects.iter().map(|s| s.to_string()).collect(),
            body: (&method.body).into(),
            is_public: method.is_public,
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
            Expr::FieldAccess { receiver, field } => OwnedExpr::FieldAccess {
                receiver: Box::new((&**receiver).into()),
                field: field.to_string(),
            },
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
            Expr::Cast { expr, ty } => OwnedExpr::Cast {
                expr: Box::new((&**expr).into()),
                ty: ty.into(),
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
            Literal::I32(i) => OwnedLiteral::I32(*i),
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

impl<'src> From<&SatisfiesBlock<'src>> for OwnedSatisfiesBlock {
    fn from(satisfies: &SatisfiesBlock<'src>) -> Self {
        Self {
            target_type: (&satisfies.target_type).into(),
            trait_names: satisfies
                .trait_names
                .iter()
                .map(|s| s.to_string())
                .collect(),
            methods: satisfies
                .methods
                .as_ref()
                .map(|ms| ms.iter().map(|m| m.into()).collect::<Vec<OwnedFunction>>()),
        }
    }
}

impl<'src> From<&ImportPath<'src>> for OwnedImportPath {
    fn from(import_path: &ImportPath<'src>) -> Self {
        Self {
            path: import_path.path.iter().map(|s| s.to_string()).collect(),
            alias: import_path.alias.map(|s| s.to_string()),
        }
    }
}

// For item -> owned item conversion
impl<'src> From<&Item<'src>> for OwnedItem {
    fn from(item: &Item<'src>) -> Self {
        match item {
            Item::Stmt(stmt) => OwnedItem::Stmt(stmt.into()),
            Item::ImportBlock { imports } => OwnedItem::ImportBlock {
                imports: imports.iter().map(|i| i.into()).collect(),
            },
            Item::Fn(func) => OwnedItem::Fn(func.into()),
            Item::Struct(struct_def) => OwnedItem::Struct(struct_def.into()),
            Item::Enum(enum_def) => OwnedItem::Enum(enum_def.into()),
            Item::Trait(trait_def) => OwnedItem::Trait(trait_def.into()),
            Item::Effect(effect_def) => OwnedItem::Effect(effect_def.into()),
            Item::Handler(handler_def) => OwnedItem::Handler(handler_def.into()),
            Item::Method(method) => OwnedItem::Method(method.into()),
            Item::Satisfies(satisfies) => OwnedItem::Satisfies(satisfies.into()),
        }
    }
}
