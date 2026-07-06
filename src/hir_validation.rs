use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use chumsky::span::Span;

use crate::hir;
use crate::token::SimpleSpan;
use crate::type_unifier::TypeUnifier;

#[derive(Debug, Clone)]
pub struct HirValidationError {
    pub message: String,
    pub path: PathBuf,
    pub span: SimpleSpan,
}

pub fn validate_program(items: &[hir::Item]) -> Result<(), Vec<HirValidationError>> {
    let mut validator = Validator::new(items);

    for item in items {
        validator.validate_item(item);
    }

    if validator.errors.is_empty() {
        Ok(())
    } else {
        Err(validator.errors)
    }
}

struct Validator {
    errors: Vec<HirValidationError>,
    structs: HashMap<hir::OwnedPath, hir::HirStructDef>,
    enums: HashMap<hir::OwnedPath, hir::HirEnumDef>,
    effects: HashMap<hir::OwnedPath, hir::HirEffectDef>,
    handlers: HashMap<String, hir::HirHandlerDef>,
    enum_variants: HashMap<hir::OwnedPath, (hir::OwnedPath, Vec<hir::Ty>)>,
}

impl Validator {
    fn new(items: &[hir::Item]) -> Self {
        let mut validator = Self {
            errors: vec![],
            structs: HashMap::new(),
            enums: HashMap::new(),
            effects: HashMap::new(),
            handlers: HashMap::new(),
            enum_variants: HashMap::new(),
        };

        for item in items {
            match item {
                hir::Item::Struct(s) => {
                    validator.structs.insert(vec![s.name.clone()], s.clone());
                }
                hir::Item::Effect(e) => {
                    validator.effects.insert(vec![e.name.clone()], e.clone());
                }
                hir::Item::Enum(e) => {
                    let enum_path = vec![e.name.clone()];
                    validator.enums.insert(enum_path.clone(), e.clone());
                    for variant in &e.variants {
                        let mut variant_path = enum_path.clone();
                        variant_path.push(variant.name.clone());
                        validator.enum_variants.insert(
                            variant_path,
                            (
                                enum_path.clone(),
                                variant.payload.clone().unwrap_or_default(),
                            ),
                        );
                    }
                }
                hir::Item::Handler(h) => {
                    validator.handlers.insert(h.name.clone(), h.clone());
                }
                _ => {}
            }
        }

        validator
    }

    fn validate_item(&mut self, item: &hir::Item) {
        match item {
            hir::Item::Fn(f) => self.validate_function(f),
            hir::Item::Struct(s) => {
                for field in &s.fields {
                    self.validate_ty(&field.ty, &s.defined_in, s.span);
                }
            }
            hir::Item::Enum(e) => {
                for variant in &e.variants {
                    if let Some(payload) = &variant.payload {
                        for ty in payload {
                            self.validate_ty(ty, &e.defined_in, e.span);
                        }
                    }
                }
            }
            hir::Item::TypeAlias(a) => self.validate_ty(&a.aliased, &a.defined_in, a.span),
            hir::Item::Effect(e) => {
                for op in &e.operations {
                    self.validate_signature(op, &e.defined_in, e.span);
                }
            }
            hir::Item::Handler(h) => {
                for effect in &h.effects {
                    self.validate_ty(effect, &h.defined_in, h.span);
                }
                for f in &h.functions {
                    self.validate_function(f);
                }
            }
        }
    }

    fn is_never(ty: &hir::Ty) -> bool {
        matches!(ty, hir::Ty::Special(hir::SpecialTy::Never))
    }

    fn is_normalized_to(actual: &hir::Ty, expected: &hir::Ty) -> bool {
        actual == expected || Self::is_never(actual)
    }

    fn matches_with_generics(
        actual: &hir::Ty,
        expected: &hir::Ty,
        bindings: &mut HashMap<String, hir::Ty>,
    ) -> bool {
        if Self::is_never(actual) {
            return true;
        }

        match expected {
            hir::Ty::Generic(name) => match bindings.get(name) {
                Some(bound) => actual == bound,
                None => {
                    bindings.insert(name.clone(), actual.clone());
                    true
                }
            },
            hir::Ty::Special(_) | hir::Ty::Primitive(_) => actual == expected,
            hir::Ty::Adt(expected_adt) => match (actual, expected_adt) {
                (
                    hir::Ty::Adt(hir::AdtTy::Struct {
                        name: actual_name,
                        generics: actual_generics,
                    }),
                    hir::AdtTy::Struct {
                        name: expected_name,
                        generics: expected_generics,
                    },
                )
                | (
                    hir::Ty::Adt(hir::AdtTy::Enum {
                        name: actual_name,
                        generics: actual_generics,
                    }),
                    hir::AdtTy::Enum {
                        name: expected_name,
                        generics: expected_generics,
                    },
                )
                | (
                    hir::Ty::Adt(hir::AdtTy::Effect {
                        name: actual_name,
                        generics: actual_generics,
                    }),
                    hir::AdtTy::Effect {
                        name: expected_name,
                        generics: expected_generics,
                    },
                ) => {
                    actual_name == expected_name
                        && actual_generics.len() == expected_generics.len()
                        && actual_generics.iter().zip(expected_generics).all(
                            |(actual, expected)| {
                                Self::matches_with_generics(actual, expected, bindings)
                            },
                        )
                }
                _ => false,
            },
            hir::Ty::Array(expected_elem) => match actual {
                hir::Ty::Array(actual_elem) => {
                    Self::matches_with_generics(actual_elem, expected_elem, bindings)
                }
                _ => false,
            },
            hir::Ty::Map {
                key: expected_key,
                value: expected_value,
            } => match actual {
                hir::Ty::Map {
                    key: actual_key,
                    value: actual_value,
                } => {
                    actual_key == expected_key
                        && Self::matches_with_generics(actual_value, expected_value, bindings)
                }
                _ => false,
            },
            hir::Ty::Function {
                param_types: expected_params,
                ret_type: expected_ret,
                effects: expected_effects,
            } => {
                match actual {
                    hir::Ty::Function {
                        param_types: actual_params,
                        ret_type: actual_ret,
                        effects: actual_effects,
                    } => {
                        actual_params.len() == expected_params.len()
                            && actual_effects.len() == expected_effects.len()
                            && actual_params.iter().zip(expected_params).all(
                                |(actual, expected)| {
                                    Self::matches_with_generics(actual, expected, bindings)
                                },
                            )
                            && Self::matches_with_generics(actual_ret, expected_ret, bindings)
                            && actual_effects.iter().zip(expected_effects).all(
                                |(actual, expected)| {
                                    Self::matches_with_generics(actual, expected, bindings)
                                },
                            )
                    }
                    _ => false,
                }
            }
            hir::Ty::Handler {
                effects: expected_effects,
            } => match actual {
                hir::Ty::Handler {
                    effects: actual_effects,
                } => {
                    actual_effects.len() == expected_effects.len()
                        && actual_effects
                            .iter()
                            .zip(expected_effects)
                            .all(|(actual, expected)| {
                                Self::matches_with_generics(actual, expected, bindings)
                            })
                }
                _ => false,
            },
        }
    }

    fn primitive(ty: &hir::Ty) -> Option<&hir::PrimitiveTy> {
        if let hir::Ty::Primitive(pty) = ty {
            Some(pty)
        } else {
            None
        }
    }

    fn is_signed_numeric(ty: &hir::Ty) -> bool {
        matches!(
            Self::primitive(ty),
            Some(
                hir::PrimitiveTy::I8
                    | hir::PrimitiveTy::I16
                    | hir::PrimitiveTy::I32
                    | hir::PrimitiveTy::I64
                    | hir::PrimitiveTy::F32
                    | hir::PrimitiveTy::F64
            )
        )
    }

    fn is_integer(ty: &hir::Ty) -> bool {
        matches!(
            Self::primitive(ty),
            Some(
                hir::PrimitiveTy::Byte
                    | hir::PrimitiveTy::I8
                    | hir::PrimitiveTy::I16
                    | hir::PrimitiveTy::I32
                    | hir::PrimitiveTy::I64
                    | hir::PrimitiveTy::U8
                    | hir::PrimitiveTy::U16
                    | hir::PrimitiveTy::U32
                    | hir::PrimitiveTy::U64
            )
        )
    }

    fn check_duplicate_fields(
        &mut self,
        fields: &[(String, hir::Expr)],
        path: &PathBuf,
        what: &str,
    ) {
        let mut seen = HashSet::new();
        for (field, value) in fields {
            if !seen.insert(field) {
                self.error(
                    path,
                    value.span,
                    format!("HIR {} has duplicate field `{}`", what, field),
                );
            }
        }
    }

    fn validate_function(&mut self, f: &hir::HirFunction) {
        self.validate_signature(&f.signature, &f.defined_in, f.span);
        self.validate_block(&f.body, &f.defined_in, &f.signature.ret_type);
        if !Self::is_normalized_to(&f.body.ty, &f.signature.ret_type) {
            self.error(
                &f.defined_in,
                f.span,
                format!(
                    "HIR function body type {:?} does not match signature return type {:?}",
                    f.body.ty, f.signature.ret_type
                ),
            );
        }
    }

    fn validate_signature(
        &mut self,
        sig: &hir::HirFunctionSignature,
        path: &PathBuf,
        span: SimpleSpan,
    ) {
        for param in &sig.params {
            self.validate_ty(&param.ty, path, param.span.unwrap_or(span));
        }
        self.validate_ty(&sig.ret_type, path, span);
        for effect in &sig.effects {
            self.validate_ty(effect, path, span);
        }
    }

    fn validate_block(&mut self, block: &hir::HirBlock, path: &PathBuf, expected_ty: &hir::Ty) {
        for stmt in &block.stmts {
            self.validate_stmt(stmt, path, expected_ty);
        }

        match &block.last_expr {
            Some(expr) => {
                self.validate_expr(expr, path);
                if !Self::is_normalized_to(&expr.ty, &block.ty) {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR block last expression type {:?} does not match block type {:?}",
                            expr.ty, block.ty
                        ),
                    );
                }
            }
            None => {
                if block.ty != hir::Ty::Special(hir::SpecialTy::Unit)
                    && block.ty != hir::Ty::Special(hir::SpecialTy::Never)
                {
                    self.error(
                        path,
                        SimpleSpan::new((), 0..0),
                        format!("HIR block without value has non-unit type {:?}", block.ty),
                    );
                }
            }
        }
        self.validate_ty(&block.ty, path, SimpleSpan::new((), 0..0));
        if !Self::is_normalized_to(&block.ty, expected_ty) {
            self.error(
                path,
                SimpleSpan::new((), 0..0),
                format!(
                    "HIR block type {:?} is not assignable to expected type {:?}",
                    block.ty, expected_ty
                ),
            );
        }
    }

    fn validate_stmt(&mut self, stmt: &hir::Stmt, path: &PathBuf, expected_return_ty: &hir::Ty) {
        match stmt {
            hir::Stmt::Let {
                value, ty, span, ..
            } => {
                self.validate_ty(ty, path, *span);
                if let Some(value) = value {
                    self.validate_expr(value, path);
                    if !Self::is_normalized_to(&value.ty, ty) {
                        self.error(
                            path,
                            *span,
                            format!(
                                "HIR let value type {:?} is not assignable to {:?}",
                                value.ty, ty
                            ),
                        );
                    }
                }
            }
            hir::Stmt::Return { value, span } => {
                let actual = value
                    .as_ref()
                    .map(|expr| {
                        self.validate_expr(expr, path);
                        expr.ty.clone()
                    })
                    .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                if !Self::is_normalized_to(&actual, expected_return_ty) {
                    self.error(
                        path,
                        *span,
                        format!(
                            "HIR return type {:?} is not assignable to {:?}",
                            actual, expected_return_ty
                        ),
                    );
                }
            }
            hir::Stmt::Assign { lhs, rhs, span } => {
                self.validate_expr(lhs, path);
                self.validate_expr(rhs, path);
                if !Self::is_normalized_to(&rhs.ty, &lhs.ty) {
                    self.error(
                        path,
                        *span,
                        format!(
                            "HIR assignment rhs type {:?} is not assignable to lhs type {:?}",
                            rhs.ty, lhs.ty
                        ),
                    );
                }
            }
            hir::Stmt::Expr { expr, .. } => self.validate_expr(expr, path),
            hir::Stmt::Error { span } => {
                self.error(path, *span, "HIR contains error statement".to_string());
            }
        }
    }

    fn validate_expr(&mut self, expr: &hir::Expr, path: &PathBuf) {
        self.validate_ty(&expr.ty, path, expr.span);
        match &expr.kind {
            hir::ExprKind::Literal(pty, _) => {
                let lit_ty = hir::Ty::Primitive(pty.clone());
                if expr.ty != lit_ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR literal primitive {:?} does not match expression type {:?}",
                            pty, expr.ty
                        ),
                    );
                }
            }
            hir::ExprKind::Array(items) => {
                let elem_ty = match &expr.ty {
                    hir::Ty::Array(elem_ty) => Some(elem_ty.as_ref()),
                    _ => {
                        self.error(
                            path,
                            expr.span,
                            format!("HIR array expression has non-array type {:?}", expr.ty),
                        );
                        None
                    }
                };
                for item in items {
                    self.validate_expr(item, path);
                    if let Some(elem_ty) = elem_ty {
                        if !Self::is_normalized_to(&item.ty, elem_ty) {
                            self.error(
                                path,
                                item.span,
                                format!(
                                    "HIR array element type {:?} is not assignable to {:?}",
                                    item.ty, elem_ty
                                ),
                            );
                        }
                    }
                }
            }
            hir::ExprKind::Map(entries) => {
                let entry_tys = match &expr.ty {
                    hir::Ty::Map { key, value } => Some((key.as_ref(), value.as_ref())),
                    _ => {
                        self.error(
                            path,
                            expr.span,
                            format!("HIR map expression has non-map type {:?}", expr.ty),
                        );
                        None
                    }
                };
                for (key, value) in entries {
                    self.validate_expr(key, path);
                    self.validate_expr(value, path);
                    if let Some((key_ty, value_ty)) = entry_tys {
                        if key.ty != hir::Ty::Primitive(key_ty.clone()) {
                            self.error(
                                path,
                                key.span,
                                format!(
                                    "HIR map key type {:?} does not match {:?}",
                                    key.ty, key_ty
                                ),
                            );
                        }
                        if !Self::is_normalized_to(&value.ty, value_ty) {
                            self.error(
                                path,
                                value.span,
                                format!(
                                    "HIR map value type {:?} is not assignable to {:?}",
                                    value.ty, value_ty
                                ),
                            );
                        }
                    }
                }
            }
            hir::ExprKind::Path(_) => {}
            hir::ExprKind::FieldAccess { receiver, field } => {
                self.validate_expr(receiver, path);
                let expected = match &receiver.ty {
                    hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) => match self.structs.get(name) {
                        Some(struct_def) => match struct_def
                            .fields
                            .iter()
                            .find(|f| f.name == *field)
                        {
                            Some(field_def) => Some((name.clone(), field_def.ty.clone())),
                            None => {
                                self.error(
                                    path,
                                    expr.span,
                                    format!("HIR field `{}` does not exist on {:?}", field, name),
                                );
                                None
                            }
                        },
                        None => {
                            self.error(
                                path,
                                expr.span,
                                format!("HIR field access owner {:?} is not a known struct", name),
                            );
                            None
                        }
                    },
                    _ => {
                        self.error(
                            path,
                            expr.span,
                            format!(
                                "HIR field access receiver has non-struct type {:?}",
                                receiver.ty
                            ),
                        );
                        None
                    }
                };
                if let Some((owner, field_ty)) = &expected {
                    if &expr.ty != field_ty {
                        self.error(
                            path,
                            expr.span,
                            format!(
                                "HIR field access type {:?} does not match field type {:?}",
                                expr.ty, field_ty
                            ),
                        );
                    }
                    if let Some(hir::Resolution::Field {
                        owner: resolved_owner,
                        field: resolved,
                    }) = &expr.resolution
                    {
                        if resolved_owner != owner {
                            self.error(
                                path,
                                expr.span,
                                format!(
                                    "HIR field access resolution owner {:?} does not match receiver {:?}",
                                    resolved_owner, owner
                                ),
                            );
                        }
                        if resolved != field {
                            self.error(
                                path,
                                expr.span,
                                format!(
                                    "HIR field access resolution field `{}` does not match access `{}`",
                                    resolved, field
                                ),
                            );
                        }
                    }
                }
            }
            hir::ExprKind::Unary { op, rhs } => {
                self.validate_expr(rhs, path);
                self.validate_unary(*op, rhs, expr, path);
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                self.validate_expr(lhs, path);
                self.validate_expr(rhs, path);
                self.validate_binary(*op, lhs, rhs, expr, path);
            }
            hir::ExprKind::Call { fun, args } => {
                self.validate_expr(fun, path);
                for arg in args {
                    self.validate_expr(arg, path);
                }
                match &fun.ty {
                    hir::Ty::Function {
                        param_types,
                        ret_type,
                        ..
                    } => {
                        if args.len() != param_types.len() {
                            self.error(
                                path,
                                expr.span,
                                format!(
                                    "HIR call arity {} does not match function arity {}",
                                    args.len(),
                                    param_types.len()
                                ),
                            );
                        }
                        for (arg, param_ty) in args.iter().zip(param_types) {
                            if !Self::is_normalized_to(&arg.ty, param_ty) {
                                self.error(
                                    path,
                                    arg.span,
                                    format!(
                                        "HIR call argument type {:?} is not assignable to {:?}",
                                        arg.ty, param_ty
                                    ),
                                );
                            }
                        }
                        if ret_type.as_ref() != &expr.ty {
                            self.error(
                                path,
                                expr.span,
                                format!(
                                    "HIR call result type {:?} is not assignable to expression type {:?}",
                                    ret_type, expr.ty
                                ),
                            );
                        }
                    }
                    _ => {
                        self.error(
                            path,
                            fun.span,
                            format!("HIR call target has non-function type {:?}", fun.ty),
                        );
                    }
                }
            }
            hir::ExprKind::StructInit {
                path: init_path,
                fields,
            } => {
                for (_, field) in fields {
                    self.validate_expr(field, path);
                }
                self.validate_struct_init(init_path, fields, expr, path);
            }
            hir::ExprKind::Block(block) => self.validate_block(block, path, &expr.ty),
            hir::ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                self.validate_expr(cond, path);
                if cond.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.error(path, cond.span, "HIR if condition is not bool".to_string());
                }
                let expected_then_ty = if else_block.is_some() {
                    &expr.ty
                } else {
                    &then_block.ty
                };
                self.validate_block(then_block, path, expected_then_ty);
                if else_block.is_some() && !Self::is_normalized_to(&then_block.ty, &expr.ty) {
                    self.error(
                        path,
                        then_block
                            .last_expr
                            .as_ref()
                            .map(|last| last.span)
                            .unwrap_or(expr.span),
                        format!(
                            "HIR if then branch type {:?} is not assignable to expression type {:?}",
                            then_block.ty, expr.ty
                        ),
                    );
                }
                if let Some(else_expr) = else_block {
                    self.validate_expr(else_expr, path);
                    if !Self::is_normalized_to(&else_expr.ty, &expr.ty) {
                        self.error(
                            path,
                            else_expr.span,
                            format!(
                                "HIR if else branch type {:?} is not assignable to expression type {:?}",
                                else_expr.ty, expr.ty
                            ),
                        );
                    }
                }
            }
            hir::ExprKind::Match { scrutinee, arms } => {
                self.validate_expr(scrutinee, path);
                for (pattern, arm) in arms {
                    self.validate_pattern(pattern, &scrutinee.ty, path, arm.span);
                    self.validate_expr(arm, path);
                    if !Self::is_normalized_to(&arm.ty, &expr.ty) {
                        self.error(
                            path,
                            arm.span,
                            format!(
                                "HIR match arm type {:?} is not assignable to expression type {:?}",
                                arm.ty, expr.ty
                            ),
                        );
                    }
                }
            }
            hir::ExprKind::While { cond, body } => {
                self.validate_expr(cond, path);
                if cond.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.error(
                        path,
                        cond.span,
                        "HIR while condition is not bool".to_string(),
                    );
                }
                self.validate_block(body, path, &body.ty);
            }
            hir::ExprKind::Perform {
                path: perform_path,
                args,
            } => {
                for arg in args {
                    self.validate_expr(arg, path);
                }
                self.validate_perform(perform_path, args, expr, path);
            }
            hir::ExprKind::Handler(handler) => {
                if !matches!(expr.ty, hir::Ty::Handler { .. }) {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR handler expression has non-handler type {:?}", expr.ty),
                    );
                }
                match handler {
                    hir::HirHandlerBody::Path(handler_path) => {
                        if handler_path.last().is_none() {
                            self.error(path, expr.span, "HIR handler path is empty".to_string());
                        }
                    }
                    hir::HirHandlerBody::Inline(functions) => {
                        for f in functions {
                            self.validate_function(f);
                        }
                    }
                }
            }
            hir::ExprKind::Handle { body, handler } => {
                self.validate_block(body, path, &expr.ty);
                self.validate_expr(handler, path);
                if !matches!(handler.ty, hir::Ty::Handler { .. }) {
                    self.error(
                        path,
                        handler.span,
                        format!("HIR handle uses non-handler type {:?}", handler.ty),
                    );
                }
            }
            hir::ExprKind::FnLiteral(f) => {
                for param in &f.params {
                    self.validate_ty(&param.ty, path, param.span.unwrap_or(expr.span));
                }
                self.validate_ty(&f.ret_type, path, expr.span);
                for effect in &f.effects {
                    self.validate_ty(effect, path, expr.span);
                }
                self.validate_block(&f.body, path, &f.ret_type);
                if !Self::is_normalized_to(&f.body.ty, &f.ret_type) {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR function literal body type {:?} is not assignable to return type {:?}",
                            f.body.ty, f.ret_type
                        ),
                    );
                }
                let fn_ty = hir::Ty::Function {
                    param_types: f.params.iter().map(|p| p.ty.clone()).collect(),
                    ret_type: Box::new(f.ret_type.clone()),
                    effects: f.effects.clone(),
                };
                if expr.ty != fn_ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR function literal type {:?} does not match expression type {:?}",
                            fn_ty, expr.ty
                        ),
                    );
                }
            }
            hir::ExprKind::Cast { expr: inner } => {
                self.validate_expr(inner, path);
                self.validate_ty(&expr.ty, path, expr.span);
                if !Self::is_never(&inner.ty)
                    && inner.ty != expr.ty
                    && !(TypeUnifier::is_numeric(&inner.ty) && TypeUnifier::is_numeric(&expr.ty))
                {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR cast from {:?} to {:?} is not valid", inner.ty, expr.ty),
                    );
                }
            }
            hir::ExprKind::Error => {
                self.error(path, expr.span, "HIR contains error expression".to_string());
            }
        }
    }

    fn validate_unary(
        &mut self,
        op: hir::UnaryOp,
        rhs: &hir::Expr,
        expr: &hir::Expr,
        path: &PathBuf,
    ) {
        match op {
            hir::UnaryOp::Negate => {
                if !Self::is_signed_numeric(&rhs.ty) {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR unary negate operand is not signed numeric: {:?}",
                            rhs.ty
                        ),
                    );
                }
                if expr.ty != rhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR unary negate result type {:?} does not match operand type {:?}",
                            expr.ty, rhs.ty
                        ),
                    );
                }
            }
            hir::UnaryOp::Not => {
                let bool_ty = hir::Ty::Primitive(hir::PrimitiveTy::Bool);
                if rhs.ty != bool_ty || expr.ty != bool_ty {
                    self.error(
                        path,
                        expr.span,
                        "HIR unary not is not bool-typed".to_string(),
                    );
                }
            }
        }
    }

    fn validate_struct_init(
        &mut self,
        init_path: &hir::OwnedPath,
        fields: &[(String, hir::Expr)],
        expr: &hir::Expr,
        path: &PathBuf,
    ) {
        self.check_duplicate_fields(fields, path, "struct init");
        match &expr.ty {
            hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) => {
                if init_path != name {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR struct init path {:?} does not match expression type {:?}",
                            init_path, name
                        ),
                    );
                }

                let Some(struct_def) = self.structs.get(name).cloned() else {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR struct init references unknown struct {:?}", name),
                    );
                    return;
                };

                for field_def in &struct_def.fields {
                    match fields.iter().find(|(field, _)| field == &field_def.name) {
                        Some((_, value)) => {
                            if !Self::is_normalized_to(&value.ty, &field_def.ty) {
                                self.error(
                                    path,
                                    value.span,
                                    format!(
                                        "HIR struct field `{}` type {:?} does not match {:?}",
                                        field_def.name, value.ty, field_def.ty
                                    ),
                                );
                            }
                        }
                        None => {
                            self.error(
                                path,
                                expr.span,
                                format!("HIR struct init missing field `{}`", field_def.name),
                            );
                        }
                    }
                }

                for (field, value) in fields {
                    if !struct_def
                        .fields
                        .iter()
                        .any(|field_def| field_def.name == *field)
                    {
                        self.error(
                            path,
                            value.span,
                            format!("HIR struct init has unknown field `{}`", field),
                        );
                    }
                }
            }
            hir::Ty::Adt(hir::AdtTy::Enum { name, .. }) => {
                let Some((enum_path, payload)) = self.enum_variants.get(init_path).cloned() else {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR enum init references unknown variant {:?}", init_path),
                    );
                    return;
                };
                if &enum_path != name {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR enum init variant {:?} belongs to {:?}, not {:?}",
                            init_path, enum_path, name
                        ),
                    );
                }
                if let [
                    hir::Ty::Adt(hir::AdtTy::Struct {
                        name: payload_struct,
                        ..
                    }),
                ] = payload.as_slice()
                {
                    let Some(struct_def) = self.structs.get(payload_struct).cloned() else {
                        self.error(
                            path,
                            expr.span,
                            format!(
                                "HIR enum init references unknown payload struct {:?}",
                                payload_struct
                            ),
                        );
                        return;
                    };

                    for field_def in &struct_def.fields {
                        match fields.iter().find(|(field, _)| field == &field_def.name) {
                            Some((_, value)) => {
                                if !Self::is_normalized_to(&value.ty, &field_def.ty) {
                                    self.error(
                                        path,
                                        value.span,
                                        format!(
                                            "HIR enum payload field `{}` type {:?} does not match {:?}",
                                            field_def.name, value.ty, field_def.ty
                                        ),
                                    );
                                }
                            }
                            None => {
                                self.error(
                                    path,
                                    expr.span,
                                    format!(
                                        "HIR enum payload struct init missing field `{}`",
                                        field_def.name
                                    ),
                                );
                            }
                        }
                    }

                    for (field, value) in fields {
                        if !struct_def
                            .fields
                            .iter()
                            .any(|field_def| field_def.name == *field)
                        {
                            self.error(
                                path,
                                value.span,
                                format!(
                                    "HIR enum payload struct init has unknown field `{}`",
                                    field
                                ),
                            );
                        }
                    }
                } else {
                    if fields.len() != payload.len() {
                        self.error(
                            path,
                            expr.span,
                            format!(
                                "HIR enum init payload arity {} does not match variant arity {}",
                                fields.len(),
                                payload.len()
                            ),
                        );
                    }
                    for ((_, value), expected) in fields.iter().zip(payload.iter()) {
                        if !Self::is_normalized_to(&value.ty, expected) {
                            self.error(
                                path,
                                value.span,
                                format!(
                                    "HIR enum payload type {:?} does not match {:?}",
                                    value.ty, expected
                                ),
                            );
                        }
                    }
                }
            }
            _ => {
                self.error(
                    path,
                    expr.span,
                    format!("HIR struct init has non-ADT type {:?}", expr.ty),
                );
            }
        }
    }

    fn validate_perform(
        &mut self,
        perform_path: &hir::OwnedPath,
        args: &[hir::Expr],
        expr: &hir::Expr,
        path: &PathBuf,
    ) {
        if perform_path.len() != 2 {
            self.error(
                path,
                expr.span,
                format!(
                    "HIR perform path {:?} is not effect-qualified",
                    perform_path
                ),
            );
            return;
        }

        let effect_path = vec![perform_path[0].clone()];
        let Some(effect) = self.effects.get(&effect_path).cloned() else {
            self.error(
                path,
                expr.span,
                format!("HIR perform references unknown effect {:?}", effect_path),
            );
            return;
        };

        let op_name = &perform_path[1];
        let Some(op) = effect.operations.iter().find(|op| &op.name == op_name) else {
            self.error(
                path,
                expr.span,
                format!(
                    "HIR perform references unknown operation {:?}",
                    perform_path
                ),
            );
            return;
        };

        if args.len() != op.params.len() {
            self.error(
                path,
                expr.span,
                format!(
                    "HIR perform arity {} does not match operation arity {}",
                    args.len(),
                    op.params.len()
                ),
            );
        }
        let mut bindings = HashMap::new();
        for (arg, param) in args.iter().zip(&op.params) {
            if !Self::matches_with_generics(&arg.ty, &param.ty, &mut bindings) {
                self.error(
                    path,
                    arg.span,
                    format!(
                        "HIR perform argument type {:?} does not match {:?}",
                        arg.ty, param.ty
                    ),
                );
            }
        }
        if !Self::matches_with_generics(&expr.ty, &op.ret_type, &mut bindings) {
            self.error(
                path,
                expr.span,
                format!(
                    "HIR perform result type {:?} does not match operation return type {:?}",
                    expr.ty, op.ret_type
                ),
            );
        }
    }

    fn validate_binary(
        &mut self,
        op: hir::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
        expr: &hir::Expr,
        path: &PathBuf,
    ) {
        let both_numeric = TypeUnifier::is_numeric(&lhs.ty) && TypeUnifier::is_numeric(&rhs.ty);
        let numeric_common = both_numeric
            .then(|| TypeUnifier::unify_numeric(&lhs.ty, &rhs.ty))
            .flatten();

        match op {
            hir::BinaryOp::Add | hir::BinaryOp::Sub | hir::BinaryOp::Mul | hir::BinaryOp::Div => {
                if numeric_common.as_ref() != Some(&lhs.ty) || lhs.ty != rhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR arithmetic operands are not normalized: lhs={:?}, rhs={:?}",
                            lhs.ty, rhs.ty
                        ),
                    );
                }
                if expr.ty != lhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR arithmetic result type {:?} does not match operand type {:?}",
                            expr.ty, lhs.ty
                        ),
                    );
                }
            }
            hir::BinaryOp::Mod
            | hir::BinaryOp::BitShiftLeft
            | hir::BinaryOp::BitShiftRight
            | hir::BinaryOp::Xor => {
                if lhs.ty != rhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR integer operands are not normalized: lhs={:?}, rhs={:?}",
                            lhs.ty, rhs.ty
                        ),
                    );
                }
                if !Self::is_integer(&lhs.ty) {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR integer operator operand is not integer: {:?}", lhs.ty),
                    );
                }
                if expr.ty != lhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR integer operator result type {:?} does not match operand type {:?}",
                            expr.ty, lhs.ty
                        ),
                    );
                }
            }
            hir::BinaryOp::Eq | hir::BinaryOp::Ne => {
                if lhs.ty != rhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR equality operands are not normalized: lhs={:?}, rhs={:?}",
                            lhs.ty, rhs.ty
                        ),
                    );
                }
                if expr.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.error(
                        path,
                        expr.span,
                        "HIR equality result is not bool".to_string(),
                    );
                }
            }
            hir::BinaryOp::Lt | hir::BinaryOp::Lte | hir::BinaryOp::Gt | hir::BinaryOp::Gte => {
                if lhs.ty != rhs.ty {
                    self.error(
                        path,
                        expr.span,
                        format!(
                            "HIR comparison operands are not normalized: lhs={:?}, rhs={:?}",
                            lhs.ty, rhs.ty
                        ),
                    );
                }
                if !TypeUnifier::is_numeric(&lhs.ty) {
                    self.error(
                        path,
                        expr.span,
                        format!("HIR ordering operand is not numeric: {:?}", lhs.ty),
                    );
                }
                if expr.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.error(
                        path,
                        expr.span,
                        "HIR comparison result is not bool".to_string(),
                    );
                }
            }
            hir::BinaryOp::And | hir::BinaryOp::Or => {
                let bool_ty = hir::Ty::Primitive(hir::PrimitiveTy::Bool);
                if lhs.ty != bool_ty || rhs.ty != bool_ty || expr.ty != bool_ty {
                    self.error(
                        path,
                        expr.span,
                        "HIR boolean op is not bool-typed".to_string(),
                    );
                }
            }
            hir::BinaryOp::Assign => {
                self.error(
                    path,
                    expr.span,
                    "HIR expression-level assignment is not supported".to_string(),
                );
            }
        }
    }

    fn validate_pattern(
        &mut self,
        pattern: &hir::HirPattern,
        scrutinee_ty: &hir::Ty,
        path: &PathBuf,
        span: SimpleSpan,
    ) {
        self.validate_ty(&pattern.ty, path, span);
        match &pattern.kind {
            hir::HirPatternKind::Literal(pty, _) => {
                let lit_ty = hir::Ty::Primitive(pty.clone());
                if pattern.ty != lit_ty {
                    self.error(
                        path,
                        span,
                        format!(
                            "HIR literal pattern primitive {:?} does not match pattern type {:?}",
                            pty, pattern.ty
                        ),
                    );
                }
                if !Self::is_normalized_to(&pattern.ty, scrutinee_ty) {
                    self.error(
                        path,
                        span,
                        format!(
                            "HIR literal pattern type {:?} is not compatible with scrutinee type {:?}",
                            pattern.ty, scrutinee_ty
                        ),
                    );
                }
            }
            hir::HirPatternKind::Identifier(_) | hir::HirPatternKind::Wildcard => {
                if pattern.ty != *scrutinee_ty {
                    self.error(
                        path,
                        span,
                        format!(
                            "HIR binding pattern type {:?} does not match scrutinee type {:?}",
                            pattern.ty, scrutinee_ty
                        ),
                    );
                }
            }
            hir::HirPatternKind::Path { args, .. } => {
                for arg in args {
                    self.validate_pattern(arg, &arg.ty, path, span);
                }
            }
        }
    }

    fn validate_ty(&mut self, ty: &hir::Ty, path: &PathBuf, span: SimpleSpan) {
        match ty {
            hir::Ty::Special(_) | hir::Ty::Primitive(_) => {}
            hir::Ty::Adt(hir::AdtTy::Struct { name, generics }) => {
                if !self.structs.contains_key(name) {
                    self.error(
                        path,
                        span,
                        format!("HIR references unknown struct {:?}", name),
                    );
                }
                for generic in generics {
                    self.validate_ty(generic, path, span);
                }
            }
            hir::Ty::Adt(hir::AdtTy::Enum { name, generics }) => {
                if !self.enums.contains_key(name) {
                    self.error(
                        path,
                        span,
                        format!("HIR references unknown enum {:?}", name),
                    );
                }
                for generic in generics {
                    self.validate_ty(generic, path, span);
                }
            }
            hir::Ty::Adt(hir::AdtTy::Effect { name, generics }) => {
                if !self.effects.contains_key(name) {
                    self.error(
                        path,
                        span,
                        format!("HIR references unknown effect {:?}", name),
                    );
                }
                for generic in generics {
                    self.validate_ty(generic, path, span);
                }
            }
            hir::Ty::Array(elem) => self.validate_ty(elem, path, span),
            hir::Ty::Map { value, .. } => self.validate_ty(value, path, span),
            hir::Ty::Function {
                param_types,
                ret_type,
                effects,
            } => {
                for param in param_types {
                    self.validate_ty(param, path, span);
                }
                self.validate_ty(ret_type, path, span);
                for effect in effects {
                    self.validate_ty(effect, path, span);
                }
            }
            hir::Ty::Handler { effects } => {
                for effect in effects {
                    self.validate_ty(effect, path, span);
                }
            }
            hir::Ty::Generic(name) if name == "!" => {
                self.error(
                    path,
                    span,
                    "HIR contains unresolved generic `!`; use SpecialTy::Never".to_string(),
                );
            }
            hir::Ty::Generic(name)
                if name.starts_with('_') || name == "fn" || name == "<effect>" =>
            {
                self.error(
                    path,
                    span,
                    format!("HIR contains unresolved recovery generic `{}`", name),
                );
            }
            hir::Ty::Generic(_) => {}
        }
    }

    fn error(&mut self, path: &PathBuf, span: SimpleSpan, message: String) {
        self.errors.push(HirValidationError {
            message,
            path: path.clone(),
            span,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span() -> SimpleSpan {
        SimpleSpan::new((), 0..0)
    }

    fn path() -> PathBuf {
        PathBuf::from("test.bst")
    }

    fn i32_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::I32)
    }

    fn str_ty() -> hir::Ty {
        hir::Ty::Primitive(hir::PrimitiveTy::Str)
    }

    fn test_struct() -> hir::Item {
        hir::Item::Struct(hir::HirStructDef {
            name: "Person".to_string(),
            fields: vec![hir::HirField {
                name: "age".to_string(),
                ty: i32_ty(),
                name_span: None,
            }],
            is_public: false,
            defined_in: path(),
            span: span(),
            context_id: None,
        })
    }

    fn test_enum() -> hir::Item {
        hir::Item::Enum(hir::HirEnumDef {
            name: "UserType".to_string(),
            variants: vec![hir::HirEnumVariant {
                name: "B2B".to_string(),
                payload: Some(vec![hir::Ty::Adt(hir::AdtTy::Struct {
                    name: vec!["Person".to_string()],
                    generics: vec![],
                })]),
                name_span: None,
            }],
            is_public: false,
            defined_in: path(),
            span: span(),
            context_id: None,
        })
    }

    fn function_with_expr(expr: hir::Expr) -> hir::Item {
        hir::Item::Fn(hir::HirFunction {
            signature: hir::HirFunctionSignature {
                name: "main".to_string(),
                params: vec![],
                ret_type: hir::Ty::Special(hir::SpecialTy::Unit),
                effects: vec![],
            },
            body: hir::HirBlock {
                stmts: vec![hir::Stmt::Expr { span: span(), expr }],
                last_expr: None,
                ty: hir::Ty::Special(hir::SpecialTy::Unit),
            },
            is_public: false,
            defined_in: path(),
            span: span(),
            context_id: None,
        })
    }

    fn validation_messages(items: Vec<hir::Item>) -> Vec<String> {
        validate_program(&items)
            .unwrap_err()
            .into_iter()
            .map(|err| err.message)
            .collect()
    }

    #[test]
    fn rejects_unknown_enum_variant() {
        let expr = hir::Expr {
            ty: hir::Ty::Adt(hir::AdtTy::Enum {
                name: vec!["UserType".to_string()],
                generics: vec![],
            }),
            kind: hir::ExprKind::StructInit {
                path: vec!["UserType".to_string(), "Typo".to_string()],
                fields: vec![],
            },
            span: span(),
            resolution: None,
        };

        let messages =
            validation_messages(vec![test_struct(), test_enum(), function_with_expr(expr)]);
        assert!(messages.iter().any(|msg| msg.contains("unknown variant")));
    }

    #[test]
    fn rejects_duplicate_struct_fields() {
        let value = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::Literal(hir::PrimitiveTy::I32, "1".to_string()),
            span: span(),
            resolution: None,
        };
        let expr = hir::Expr {
            ty: hir::Ty::Adt(hir::AdtTy::Struct {
                name: vec!["Person".to_string()],
                generics: vec![],
            }),
            kind: hir::ExprKind::StructInit {
                path: vec!["Person".to_string()],
                fields: vec![
                    ("age".to_string(), value.clone()),
                    ("age".to_string(), value),
                ],
            },
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![test_struct(), function_with_expr(expr)]);
        assert!(messages.iter().any(|msg| msg.contains("duplicate field")));
    }

    #[test]
    fn rejects_field_access_without_valid_field() {
        let receiver = hir::Expr {
            ty: hir::Ty::Adt(hir::AdtTy::Struct {
                name: vec!["Person".to_string()],
                generics: vec![],
            }),
            kind: hir::ExprKind::Path(vec!["person".to_string()]),
            span: span(),
            resolution: None,
        };
        let expr = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::FieldAccess {
                receiver: Box::new(receiver),
                field: "missing".to_string(),
            },
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![test_struct(), function_with_expr(expr)]);
        assert!(messages.iter().any(|msg| msg.contains("does not exist")));
    }

    #[test]
    fn rejects_invalid_cast() {
        let inner = hir::Expr {
            ty: str_ty(),
            kind: hir::ExprKind::Literal(hir::PrimitiveTy::Str, "x".to_string()),
            span: span(),
            resolution: None,
        };
        let expr = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::Cast {
                expr: Box::new(inner),
            },
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![function_with_expr(expr)]);
        assert!(messages.iter().any(|msg| msg.contains("cast from")));
    }

    #[test]
    fn rejects_expression_level_assignment() {
        let lhs = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::Path(vec!["x".to_string()]),
            span: span(),
            resolution: None,
        };
        let rhs = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::Literal(hir::PrimitiveTy::I32, "1".to_string()),
            span: span(),
            resolution: None,
        };
        let expr = hir::Expr {
            ty: i32_ty(),
            kind: hir::ExprKind::Binary {
                op: hir::BinaryOp::Assign,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![function_with_expr(expr)]);
        assert!(
            messages
                .iter()
                .any(|msg| msg.contains("expression-level assignment"))
        );
    }

    #[test]
    fn rejects_handler_expression_with_non_handler_type() {
        let expr = hir::Expr {
            ty: hir::Ty::Special(hir::SpecialTy::Unit),
            kind: hir::ExprKind::Handler(hir::HirHandlerBody::Path(vec![
                "PanicHandler".to_string(),
            ])),
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![function_with_expr(expr)]);
        assert!(
            messages
                .iter()
                .any(|msg| msg.contains("handler expression has non-handler type"))
        );
    }

    #[test]
    fn rejects_handle_with_non_handler_value() {
        let handler = hir::Expr {
            ty: hir::Ty::Special(hir::SpecialTy::Unit),
            kind: hir::ExprKind::Literal(hir::PrimitiveTy::I32, "1".to_string()),
            span: span(),
            resolution: None,
        };
        let expr = hir::Expr {
            ty: hir::Ty::Special(hir::SpecialTy::Unit),
            kind: hir::ExprKind::Handle {
                body: hir::HirBlock {
                    stmts: vec![],
                    last_expr: None,
                    ty: hir::Ty::Special(hir::SpecialTy::Unit),
                },
                handler: Box::new(handler),
            },
            span: span(),
            resolution: None,
        };

        let messages = validation_messages(vec![function_with_expr(expr)]);
        assert!(
            messages
                .iter()
                .any(|msg| msg.contains("handle uses non-handler type"))
        );
    }
}
