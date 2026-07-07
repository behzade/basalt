// --- Owned AST Types for Symbol Signatures ---
// These are used in SymbolSignature to avoid lifetime issues
//
use crate::{ast::*, token::SimpleSpan};
use serde::Serialize;

/// A generic wrapper that adds a source code span to any AST node.
#[derive(Debug, Clone, Serialize)]
pub struct Spanned<T> {
    pub item: T,
    #[serde(serialize_with = "crate::token::serialize_simple_span")]
    pub span: SimpleSpan,
}

// --- Type Aliases for Readability ---
pub type SpannedExpr = Spanned<OwnedExpr>;
pub type SpannedStmt = Spanned<OwnedStmt>;
pub type SpannedPattern = Spanned<OwnedPattern>;

/// Owned version of Type for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedType {
    pub path: Vec<String>,
    pub generics: Vec<OwnedType>,
    // Optional function type shape when `path == ["fn"]`
    pub fn_params: Option<Vec<OwnedType>>,
    pub fn_ret: Option<Box<OwnedType>>,
    pub fn_effects: Option<Vec<OwnedType>>,
}

/// Owned version of Function for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedFunction {
    pub name: String,
    pub generics: Vec<String>,
    pub params: Vec<(bool, Option<String>, OwnedType)>,
    pub ret_type: Option<OwnedType>,
    pub effects: Vec<String>,
    pub body: SpannedExpr, // UPDATED
    pub is_public: bool,
}

/// Owned version of StructDef for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedStructDef {
    pub name: String,
    pub generics: Vec<String>,
    pub fields: Vec<(String, OwnedType)>,
    pub is_public: bool,
}

/// Owned version of EnumDef for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedEnumDef {
    pub name: Option<String>,
    pub generics: Vec<String>,
    pub variants: Vec<(String, Option<Vec<OwnedType>>)>,
    pub is_public: bool,
}

/// Owned version of EffectDef for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedEffectDef {
    pub name: String,
    pub operations: Vec<OwnedEffectOp>,
    pub is_public: bool,
}

/// Owned version of EffectOp for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedEffectOp {
    pub name: String,
    pub params: Vec<OwnedType>,
    pub ret_type: OwnedType,
    pub is_public: bool,
}

/// Owned version of HandlerDef for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub struct OwnedHandlerDef {
    pub name: String,
    pub effects: Vec<String>,
    pub functions: Vec<OwnedFunction>,
    pub is_public: bool,
}

/// Owned version of Expr for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub enum OwnedExpr {
    Literal(OwnedLiteral),
    Path(Vec<String>),
    FieldAccess {
        receiver: Box<SpannedExpr>, // UPDATED
        field: String,
    },
    /// Explicit method call syntax preserved from AST
    MethodCall {
        receiver: Box<SpannedExpr>,
        method: String,
        args: Vec<SpannedExpr>,
    },
    Unary {
        op: UnaryOp,
        rhs: Box<SpannedExpr>, // UPDATED
    },
    Binary {
        op: BinaryOp,
        lhs: Box<SpannedExpr>, // UPDATED
        rhs: Box<SpannedExpr>, // UPDATED
    },
    Call {
        fun: Box<SpannedExpr>,  // UPDATED
        args: Vec<SpannedExpr>, // UPDATED
    },
    StructInit {
        path: Vec<String>,
        generics: Vec<OwnedType>,
        fields: Vec<(String, SpannedExpr)>, // UPDATED
    },
    UnionInit {
        path: Vec<String>,
        variant: String,
        fields: Vec<(String, SpannedExpr)>,
    },
    Block {
        stmts: Vec<SpannedStmt>,             // UPDATED
        last_expr: Option<Box<SpannedExpr>>, // UPDATED
    },
    If {
        cond: Box<SpannedExpr>,               // UPDATED
        then_block: Box<SpannedExpr>,         // UPDATED
        else_block: Option<Box<SpannedExpr>>, // UPDATED
    },
    Match {
        scrutinee: Box<SpannedExpr>,              // UPDATED
        arms: Vec<(SpannedPattern, SpannedExpr)>, // UPDATED
    },
    While {
        cond: Box<SpannedExpr>, // UPDATED
        body: Box<SpannedExpr>, // UPDATED
    },
    Perform {
        path: Vec<String>,
        args: Vec<SpannedExpr>, // UPDATED
    },
    Handle {
        body: Box<SpannedExpr>, // UPDATED
        handler: OwnedHandlerBody,
    },
    /// Anonymous function literal: fn(params) -> ret effects { effects } { body }
    FnLiteral {
        params: Vec<(bool, Option<String>, OwnedType)>,
        ret_type: Option<OwnedType>,
        effects: Vec<OwnedType>,
        body: Box<SpannedExpr>,
    },
    Cast {
        expr: Box<SpannedExpr>, // UPDATED
        ty: OwnedType,
    },
    Error,
}

/// Owned version of Stmt for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub enum OwnedStmt {
    Let {
        is_mut: bool,
        name: String,
        ty: Option<OwnedType>,
        value: Option<SpannedExpr>, // UPDATED
    },
    Return(Option<SpannedExpr>),      // UPDATED
    Assign(SpannedExpr, SpannedExpr), // UPDATED
    Expr(SpannedExpr),                // UPDATED
    Error,
}

/// Owned version of Pattern for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub enum OwnedPattern {
    Literal(OwnedLiteral),
    Identifier(String),
    VariantBind {
        binding: String,
        variant_path: Vec<String>,
    },
    Wildcard,
}

/// Owned version of Literal for symbol signatures
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum OwnedLiteral {
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
    Str(String),
    Unit,
}

/// Owned version of HandlerBody for symbol signatures
#[derive(Debug, Clone, Serialize)]
pub enum OwnedHandlerBody {
    Path(Vec<String>),
    Inline(Vec<OwnedFunction>),
}

#[derive(Debug, Clone, Serialize)]
pub enum OwnedItem {
    Stmt(SpannedStmt), // UPDATED
    ImportBlock { imports: Vec<OwnedImportPath> },
    Fn(OwnedFunction),
    Struct(OwnedStructDef),
    Enum(OwnedEnumDef),
    Effect(OwnedEffectDef),
    Handler(OwnedHandlerDef),
    TypeAlias(OwnedTypeAliasDef),
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnedImportPath {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

pub type OwnedItemWithSpan = Spanned<OwnedItem>;

#[derive(Debug, Clone, Serialize)]
pub struct OwnedTypeAliasDef {
    pub name: String,
    pub generics: Vec<String>,
    pub aliased: OwnedTypeAliasBody,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize)]
pub enum OwnedTypeAliasBody {
    Union(Vec<(String, OwnedType)>),
    Type(OwnedType),
}

impl<'src> From<&Function<'src>> for OwnedFunction {
    fn from(func: &Function<'src>) -> Self {
        Self {
            name: func.name.to_string(),
            generics: func.generics.iter().map(|s| s.to_string()).collect(),
            params: func
                .params
                .iter()
                .map(|(is_mut, name, ty)| (*is_mut, name.map(|s| s.to_string()), ty.into()))
                .collect(),
            ret_type: func.ret_type.as_ref().map(|t| t.into()),
            effects: func
                .effects
                .iter()
                .map(|t| match &t.node {
                    TypeNode::Path { path, .. } => {
                        path.last().map(|s| s.to_string()).unwrap_or_default()
                    }
                    TypeNode::Never => "!".to_string(),
                    _ => "<effect>".to_string(),
                })
                .collect(),
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
            effects: handler_def
                .effects
                .iter()
                .map(|t| match &t.node {
                    TypeNode::Path { path, .. } => {
                        path.last().map(|s| s.to_string()).unwrap_or_default()
                    }
                    TypeNode::Never => "!".to_string(),
                    _ => "<effect>".to_string(),
                })
                .collect(),
            functions: handler_def.functions.iter().map(|f| f.into()).collect(),
            is_public: handler_def.is_public,
        }
    }
}

impl<'src> From<&Literal<'src>> for OwnedLiteral {
    fn from(lit: &Literal<'src>) -> Self {
        match lit {
            Literal::Bool(b) => OwnedLiteral::Bool(*b),
            Literal::I8(i) => OwnedLiteral::I8(*i),
            Literal::I16(i) => OwnedLiteral::I16(*i),
            Literal::I32(i) => OwnedLiteral::I32(*i),
            Literal::I64(i) => OwnedLiteral::I64(*i),
            Literal::U8(i) => OwnedLiteral::U8(*i),
            Literal::U16(i) => OwnedLiteral::U16(*i),
            Literal::U32(i) => OwnedLiteral::U32(*i),
            Literal::U64(i) => OwnedLiteral::U64(*i),
            Literal::F32(f) => OwnedLiteral::F32(*f),
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

impl<'src> From<&ImportPath<'src>> for OwnedImportPath {
    fn from(import_path: &ImportPath<'src>) -> Self {
        Self {
            path: import_path.path.iter().map(|s| s.to_string()).collect(),
            alias: import_path.alias.map(|s| s.to_string()),
        }
    }
}

impl<'src> From<&TypeAliasDef<'src>> for OwnedTypeAliasDef {
    fn from(def: &TypeAliasDef<'src>) -> Self {
        let aliased = match &def.aliased {
            TypeAliasBody::Type(t) => OwnedTypeAliasBody::Type(t.into()),
            TypeAliasBody::Union { variants } => OwnedTypeAliasBody::Union(
                variants
                    .iter()
                    .map(|(n, t)| (n.to_string(), t.into()))
                    .collect(),
            ),
        };
        Self {
            name: def.name.to_string(),
            generics: def.generics.iter().map(|s| s.to_string()).collect(),
            aliased,
            is_public: def.is_public,
        }
    }
}

impl<'src> From<&Item<'src>> for OwnedItem {
    fn from(item: &Item<'src>) -> Self {
        match &item.node {
            ItemNode::Stmt(stmt) => OwnedItem::Stmt(stmt.into()),
            ItemNode::ImportBlock { imports } => OwnedItem::ImportBlock {
                imports: imports.iter().map(|i| i.into()).collect(),
            },
            ItemNode::Fn(f) => OwnedItem::Fn(f.into()),
            ItemNode::Struct(s) => OwnedItem::Struct(s.into()),
            ItemNode::Enum(e) => OwnedItem::Enum(e.into()),
            ItemNode::Effect(eff) => OwnedItem::Effect(eff.into()),
            ItemNode::Handler(h) => OwnedItem::Handler(h.into()),
            ItemNode::TypeAlias(ta) => OwnedItem::TypeAlias(ta.into()),
        }
    }
}

impl<'src> From<&Type<'src>> for OwnedType {
    fn from(ty: &Type<'src>) -> Self {
        match &ty.node {
            TypeNode::Path { path, generics } => OwnedType {
                path: path.iter().map(|s| s.to_string()).collect(),
                generics: generics.iter().map(|g| g.into()).collect(),
                fn_params: None,
                fn_ret: None,
                fn_effects: None,
            },
            TypeNode::Union(_) => OwnedType {
                path: vec!["union".to_string()],
                generics: vec![],
                fn_params: None,
                fn_ret: None,
                fn_effects: None,
            },
            TypeNode::Function {
                params,
                ret,
                effects,
            } => OwnedType {
                path: vec!["fn".to_string()],
                generics: vec![],
                fn_params: Some(params.iter().map(|t| t.into()).collect()),
                fn_ret: Some(Box::new(ret.as_ref().into())),
                fn_effects: Some(effects.iter().map(|t| t.into()).collect()),
            },
            TypeNode::Handler { .. } => OwnedType {
                path: vec!["handler".to_string()],
                generics: vec![],
                fn_params: None,
                fn_ret: None,
                fn_effects: None,
            },
            TypeNode::Never => OwnedType {
                path: vec!["!".to_string()],
                generics: vec![],
                fn_params: None,
                fn_ret: None,
                fn_effects: None,
            },
        }
    }
}

impl<'src> From<&Expr<'src>> for SpannedExpr {
    fn from(spanned_expr: &Expr<'src>) -> Self {
        let owned_node = match &spanned_expr.node {
            ExprNode::Literal(lit) => OwnedExpr::Literal(lit.into()),
            ExprNode::Path(path) => OwnedExpr::Path(path.iter().map(|s| s.to_string()).collect()),
            ExprNode::FieldAccess { receiver, field } => OwnedExpr::FieldAccess {
                receiver: Box::new(receiver.as_ref().into()),
                field: field.to_string(),
            },
            ExprNode::MethodCall {
                receiver,
                method,
                args,
            } => OwnedExpr::MethodCall {
                receiver: Box::new(receiver.as_ref().into()),
                method: method.to_string(),
                args: args.iter().map(|a| a.into()).collect(),
            },
            ExprNode::Unary { op, rhs } => OwnedExpr::Unary {
                op: *op,
                rhs: Box::new(rhs.as_ref().into()),
            },
            ExprNode::Binary { op, lhs, rhs } => OwnedExpr::Binary {
                op: *op,
                lhs: Box::new(lhs.as_ref().into()),
                rhs: Box::new(rhs.as_ref().into()),
            },
            ExprNode::Call { fun, args } => OwnedExpr::Call {
                fun: Box::new(fun.as_ref().into()),
                args: args.iter().map(|a| a.into()).collect(),
            },
            ExprNode::StructInit {
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
            ExprNode::UnionInit {
                path,
                variant,
                fields,
            } => OwnedExpr::UnionInit {
                path: path.iter().map(|s| s.to_string()).collect(),
                variant: variant.to_string(),
                fields: fields
                    .iter()
                    .map(|(n, e)| (n.to_string(), e.into()))
                    .collect(),
            },
            ExprNode::Block { stmts, last_expr } => OwnedExpr::Block {
                stmts: stmts.iter().map(|s| s.into()).collect(),
                last_expr: last_expr.as_ref().map(|e| Box::new(e.as_ref().into())),
            },
            ExprNode::If {
                cond,
                then_block,
                else_block,
            } => OwnedExpr::If {
                cond: Box::new(cond.as_ref().into()),
                then_block: Box::new(then_block.as_ref().into()),
                else_block: else_block.as_ref().map(|e| Box::new(e.as_ref().into())),
            },
            ExprNode::Match { scrutinee, arms } => OwnedExpr::Match {
                scrutinee: Box::new(scrutinee.as_ref().into()),
                arms: arms
                    .iter()
                    .map(|(pat, expr)| (pat.into(), expr.into()))
                    .collect(),
            },
            ExprNode::While { cond, body } => OwnedExpr::While {
                cond: Box::new(cond.as_ref().into()),
                body: Box::new(body.as_ref().into()),
            },
            ExprNode::Perform { path, args } => OwnedExpr::Perform {
                path: path.iter().map(|s| s.to_string()).collect(),
                args: args.iter().map(|a| a.into()).collect(),
            },
            ExprNode::Handle { body, handler } => OwnedExpr::Handle {
                body: Box::new(body.as_ref().into()),
                handler: handler.into(), // Assumes HandlerBody has a From impl
            },
            ExprNode::FnLiteral {
                params,
                ret_type,
                effects,
                body,
            } => OwnedExpr::FnLiteral {
                params: params
                    .iter()
                    .map(|(is_mut, n, t)| (*is_mut, n.map(|s| s.to_string()), t.into()))
                    .collect(),
                ret_type: ret_type.as_ref().map(|t| t.into()),
                effects: effects.iter().map(|t| t.into()).collect(),
                body: Box::new(body.as_ref().into()),
            },
            ExprNode::Cast { expr, ty } => OwnedExpr::Cast {
                expr: Box::new(expr.as_ref().into()),
                ty: ty.into(),
            },
            ExprNode::Error => OwnedExpr::Error,
        };

        Spanned {
            item: owned_node,
            span: spanned_expr.span,
        }
    }
}

impl<'src> From<&Stmt<'src>> for SpannedStmt {
    fn from(spanned_stmt: &Stmt<'src>) -> Self {
        let owned_node = match &spanned_stmt.node {
            StmtNode::Let {
                is_mut,
                name,
                ty,
                value,
            } => OwnedStmt::Let {
                is_mut: *is_mut,
                name: name.to_string(),
                ty: ty.as_ref().map(|t| t.into()),
                value: value.as_ref().map(|v| v.into()),
            },
            StmtNode::Return(expr) => OwnedStmt::Return(expr.as_ref().map(|e| e.into())),
            StmtNode::Assign(lhs, rhs) => OwnedStmt::Assign(lhs.into(), rhs.into()),
            StmtNode::Expr(expr) => OwnedStmt::Expr(expr.into()),
            StmtNode::Error => OwnedStmt::Error,
        };

        Spanned {
            item: owned_node,
            span: spanned_stmt.span,
        }
    }
}

impl<'src> From<&Pattern<'src>> for SpannedPattern {
    fn from(spanned_pat: &Pattern<'src>) -> Self {
        let owned_node = match &spanned_pat.node {
            PatternNode::Literal(lit) => OwnedPattern::Literal(lit.into()),
            PatternNode::Identifier(name) => OwnedPattern::Identifier(name.to_string()),
            PatternNode::VariantBind {
                binding,
                variant_path,
            } => OwnedPattern::VariantBind {
                binding: binding.to_string(),
                variant_path: variant_path.iter().map(|s| s.to_string()).collect(),
            },
            PatternNode::Wildcard => OwnedPattern::Wildcard,
        };

        Spanned {
            item: owned_node,
            span: spanned_pat.span,
        }
    }
}
