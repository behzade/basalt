use crate::ast::BinaryOp;
use crate::ast_owned::*;
use crate::hir;
use crate::type_unifier::TypeUnifier;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;

impl Typechecker {
    fn cast_numeric_expr(expr: hir::Expr, target_ty: hir::Ty) -> hir::Expr {
        if expr.ty == target_ty {
            return expr;
        }
        let span = expr.span;
        hir::Expr {
            kind: hir::ExprKind::Cast {
                expr: Box::new(expr),
            },
            ty: target_ty,
            span,
            resolution: None,
        }
    }

    fn branch_result_ty(a: &hir::Ty, b: &hir::Ty) -> Option<hir::Ty> {
        if a == b {
            Some(a.clone())
        } else if matches!(a, hir::Ty::Special(hir::SpecialTy::Never)) {
            Some(b.clone())
        } else if matches!(b, hir::Ty::Special(hir::SpecialTy::Never)) {
            Some(a.clone())
        } else {
            None
        }
    }

    fn merge_effects(mut base: Vec<hir::Ty>, extra: &[hir::Ty]) -> Vec<hir::Ty> {
        for effect in extra {
            if !base
                .iter()
                .any(|existing| Typechecker::same_effect_name(existing, effect))
            {
                base.push(effect.clone());
            }
        }
        base
    }

    fn effect_from_perform_path(&self, path: &[String]) -> Option<hir::Ty> {
        let effect_name = path.first()?;
        let effect_path = vec![effect_name.clone()];
        if matches!(
            self.type_definitions.get(&effect_path),
            Some(hir::Item::Effect(_))
        ) {
            Some(hir::Ty::Adt(hir::AdtTy::Effect {
                name: effect_path,
                generics: vec![],
            }))
        } else {
            None
        }
    }

    fn expr_effects(&self, expr: &hir::Expr) -> Vec<hir::Ty> {
        match &expr.kind {
            hir::ExprKind::Call { fun, .. } => match &fun.ty {
                hir::Ty::Function { effects, .. } => effects.clone(),
                _ => vec![],
            },
            hir::ExprKind::Perform { path, .. } => self
                .effect_from_perform_path(path)
                .map(|effect| vec![effect])
                .unwrap_or_default(),
            hir::ExprKind::Block(block) => self.block_effects(block),
            hir::ExprKind::If {
                then_block,
                else_block,
                ..
            } => {
                let mut effects = self.block_effects(then_block);
                if let Some(else_expr) = else_block {
                    effects = Self::merge_effects(effects, &self.expr_effects(else_expr));
                }
                effects
            }
            hir::ExprKind::Match { arms, .. } => arms.iter().fold(vec![], |effects, (_, arm)| {
                Self::merge_effects(effects, &self.expr_effects(arm))
            }),
            hir::ExprKind::While { body, .. } => self.block_effects(body),
            hir::ExprKind::Handle { body, handler } => {
                let mut effects = self.block_effects(body);
                if let hir::Ty::Handler {
                    effects: handled_effects,
                } = &handler.ty
                {
                    effects.retain(|effect| {
                        !handled_effects
                            .iter()
                            .any(|handled| Typechecker::same_effect_name(effect, handled))
                    });
                }
                effects
            }
            _ => vec![],
        }
    }

    fn block_effects(&self, block: &hir::HirBlock) -> Vec<hir::Ty> {
        let mut effects = vec![];
        for stmt in &block.stmts {
            let stmt_effects = match stmt {
                hir::Stmt::Let { value, .. } => value
                    .as_ref()
                    .map(|expr| self.expr_effects(expr))
                    .unwrap_or_default(),
                hir::Stmt::Return { value, .. } => value
                    .as_ref()
                    .map(|expr| self.expr_effects(expr))
                    .unwrap_or_default(),
                hir::Stmt::Assign { rhs, .. } => self.expr_effects(rhs),
                hir::Stmt::Expr { expr, .. } => self.expr_effects(expr),
                hir::Stmt::Error { .. } => vec![],
            };
            effects = Self::merge_effects(effects, &stmt_effects);
        }
        if let Some(last) = &block.last_expr {
            effects = Self::merge_effects(effects, &self.expr_effects(last));
        }
        effects
    }

    fn lower_handler_body_expr(
        &mut self,
        handler: OwnedHandlerBody,
        context: ItemContext,
        span: crate::token::SimpleSpan,
    ) -> Result<hir::Expr, ()> {
        match handler {
            OwnedHandlerBody::Path(path) => {
                let Some(name) = path.last().cloned() else {
                    self.errors.push(TypeError {
                        message: "Empty handler path".to_string(),
                        context,
                    });
                    return Err(());
                };
                let effects = if let Some(effects) = self.handler_values.get(&name).cloned() {
                    effects
                } else if let Some(Symbol::Variable { ty, .. }) = self.lookup_symbol(&name).cloned()
                {
                    if let hir::Ty::Handler { effects } = ty {
                        effects
                    } else {
                        self.errors.push(TypeError {
                            message: format!("`{}` is not a handler value", name),
                            context,
                        });
                        return Err(());
                    }
                } else {
                    self.errors.push(TypeError {
                        message: format!("Unknown handler `{}`", name),
                        context,
                    });
                    return Err(());
                };
                let body = if let Some((base, handlers)) = self.handler_aliases.get(&name).cloned()
                {
                    hir::HirHandlerBody::Composed {
                        base: Box::new(hir::HirHandlerBody::Path(vec![base])),
                        handlers: handlers
                            .into_iter()
                            .filter(|name| !name.is_empty())
                            .map(|name| hir::HirHandlerBody::Path(vec![name]))
                            .collect(),
                    }
                } else {
                    hir::HirHandlerBody::Path(path)
                };
                Ok(hir::Expr {
                    kind: hir::ExprKind::Handler(body),
                    ty: hir::Ty::Handler { effects },
                    span,
                    resolution: None,
                })
            }
            OwnedHandlerBody::Inline(funcs) => {
                let mut lowered = Vec::new();
                for f in funcs {
                    lowered.push(self.lower_function(f, context.clone())?);
                }
                Ok(hir::Expr {
                    kind: hir::ExprKind::Handler(hir::HirHandlerBody::Inline(lowered)),
                    ty: hir::Ty::Handler { effects: vec![] },
                    span,
                    resolution: None,
                })
            }
        }
    }

    fn lower_handler_value_path_expr(
        &mut self,
        path: Vec<String>,
        span: crate::token::SimpleSpan,
    ) -> Option<hir::Expr> {
        let name = path.last()?.clone();
        let effects = self.handler_values.get(&name).cloned()?;
        let body = if let Some((base, handlers)) = self.handler_aliases.get(&name).cloned() {
            hir::HirHandlerBody::Composed {
                base: Box::new(hir::HirHandlerBody::Path(vec![base])),
                handlers: handlers
                    .into_iter()
                    .filter(|name| !name.is_empty())
                    .map(|name| hir::HirHandlerBody::Path(vec![name]))
                    .collect(),
            }
        } else {
            hir::HirHandlerBody::Path(path)
        };
        Some(hir::Expr {
            kind: hir::ExprKind::Handler(body),
            ty: hir::Ty::Handler { effects },
            span,
            resolution: None,
        })
    }

    pub(crate) fn lower_expr(
        &mut self,
        expr: SpannedExpr,
        context: ItemContext,
    ) -> Result<hir::Expr, ()> {
        match expr.item {
            OwnedExpr::Literal(lit) => match lit.clone() {
                OwnedLiteral::Unit => Ok(hir::Expr {
                    ty: hir::Ty::Special(hir::SpecialTy::Unit),
                    kind: hir::ExprKind::Block(hir::HirBlock {
                        stmts: vec![],
                        last_expr: None,
                        ty: hir::Ty::Special(hir::SpecialTy::Unit),
                    }),
                    span: expr.span,
                    resolution: None,
                }),
                _ => {
                    let (ty, val_str) = self.lower_literal(lit);
                    Ok(hir::Expr {
                        kind: hir::ExprKind::Literal(ty.clone(), val_str),
                        ty: hir::Ty::Primitive(ty),
                        span: expr.span,
                        resolution: None,
                    })
                }
            },
            OwnedExpr::Path(path) => {
                if let Some(handler_expr) =
                    self.lower_handler_value_path_expr(path.clone(), expr.span)
                {
                    return Ok(handler_expr);
                }
                let name = path.last().cloned().expect("Path cannot be empty");
                let sym_opt = self.lookup_symbol(&name).cloned();
                match sym_opt {
                    Some(Symbol::Variable {
                        ty,
                        initialized,
                        decl_span,
                        ..
                    }) => {
                        if !initialized {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Use of variable '{}' before initialization",
                                    name
                                ),
                                context: context.clone(),
                            });
                        }
                        Ok(hir::Expr {
                            kind: hir::ExprKind::Path(path),
                            ty: ty.clone(),
                            span: expr.span,
                            resolution: Some(hir::Resolution::Local {
                                name: name.clone(),
                                decl_span,
                            }),
                        })
                    }
                    Some(Symbol::Function {
                        signature,
                        is_public,
                        defined_in,
                        decl_span: _,
                    }) => {
                        if !is_public && &defined_in != &context.path {
                            self.errors.push(TypeError {
                                message: format!("Function `{}` is private", name),
                                context: context.clone(),
                            });
                            return Err(());
                        }
                        Ok(hir::Expr {
                            kind: hir::ExprKind::Path(path),
                            ty: hir::Ty::Function {
                                param_types: signature
                                    .params
                                    .iter()
                                    .map(|p| p.ty.clone())
                                    .collect(),
                                ret_type: Box::new(signature.ret_type.clone()),
                                effects: signature.effects.clone(),
                            },
                            span: expr.span,
                            resolution: None,
                        })
                    }
                    None => {
                        if let Some(names) = self.imports_by_file.get(&context.path) {
                            if names.contains(&name) {
                                return Ok(hir::Expr {
                                    kind: hir::ExprKind::Path(path),
                                    ty: hir::Ty::Special(hir::SpecialTy::Unit),
                                    span: expr.span,
                                    resolution: None,
                                });
                            }
                        }
                        self.errors.push(TypeError {
                            message: format!("Cannot find value `{}` in this scope", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    _ => {
                        self.errors.push(TypeError {
                            message: format!("`{}` is not a value", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                }
            }
            OwnedExpr::Unary { op, rhs } => {
                let hir_rhs = self.lower_expr(*rhs, context.clone())?;
                let (result_ty, hir_op) = match op {
                    crate::ast::UnaryOp::Neg => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::I8)
                        | hir::Ty::Primitive(hir::PrimitiveTy::I16)
                        | hir::Ty::Primitive(hir::PrimitiveTy::I32)
                        | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                        | hir::Ty::Primitive(hir::PrimitiveTy::F32)
                        | hir::Ty::Primitive(hir::PrimitiveTy::F64) => {
                            (hir_rhs.ty.clone(), hir::UnaryOp::Negate)
                        }
                        other => {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Unary '-' not supported for type {}",
                                    Typechecker::format_ty(&other)
                                ),
                                context: context.clone(),
                            });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Negate)
                        }
                    },
                    crate::ast::UnaryOp::Not => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::Bool) => (
                            hir::Ty::Primitive(hir::PrimitiveTy::Bool),
                            hir::UnaryOp::Not,
                        ),
                        other => {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Unary '!' not supported for type {}",
                                    Typechecker::format_ty(&other)
                                ),
                                context: context.clone(),
                            });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Not)
                        }
                    },
                };
                Ok(hir::Expr {
                    kind: hir::ExprKind::Unary {
                        op: hir_op,
                        rhs: Box::new(hir_rhs),
                    },
                    ty: result_ty,
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Binary { op, lhs, rhs } => {
                let (hir_lhs, hir_rhs) = match (&lhs.item, &rhs.item) {
                    (OwnedExpr::Literal(l), other) if self.is_numeric_literal(l) => {
                        let other_hir = self.lower_expr(*rhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_lhs = self.lower_expr_with_expected(
                                Spanned {
                                    item: OwnedExpr::Literal(l.clone()),
                                    span: lhs.span,
                                },
                                other_hir.ty.clone(),
                                context.clone(),
                            )?;
                            (coerced_lhs, other_hir)
                        } else {
                            (self.lower_expr(*lhs, context.clone())?, other_hir)
                        }
                    }
                    (other, OwnedExpr::Literal(r)) if self.is_numeric_literal(r) => {
                        let other_hir = self.lower_expr(*lhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_rhs = self.lower_expr_with_expected(
                                Spanned {
                                    item: OwnedExpr::Literal(r.clone()),
                                    span: rhs.span,
                                },
                                other_hir.ty.clone(),
                                context.clone(),
                            )?;
                            (other_hir, coerced_rhs)
                        } else {
                            (other_hir, self.lower_expr(*rhs, context.clone())?)
                        }
                    }
                    _ => (
                        self.lower_expr(*lhs, context.clone())?,
                        self.lower_expr(*rhs, context.clone())?,
                    ),
                };

                let op_kind = self.lower_binary_op(op);
                let both_numeric =
                    TypeUnifier::is_numeric(&hir_lhs.ty) && TypeUnifier::is_numeric(&hir_rhs.ty);
                let is_comparison = matches!(
                    op_kind,
                    hir::BinaryOp::Eq
                        | hir::BinaryOp::Ne
                        | hir::BinaryOp::Lt
                        | hir::BinaryOp::Lte
                        | hir::BinaryOp::Gt
                        | hir::BinaryOp::Gte
                );
                let is_arithmetic = matches!(
                    op_kind,
                    hir::BinaryOp::Add
                        | hir::BinaryOp::Sub
                        | hir::BinaryOp::Mul
                        | hir::BinaryOp::Div
                        | hir::BinaryOp::Mod
                        | hir::BinaryOp::BitShiftLeft
                        | hir::BinaryOp::BitShiftRight
                        | hir::BinaryOp::Xor
                );
                let common_numeric_ty = if both_numeric && (is_comparison || is_arithmetic) {
                    TypeUnifier::unify_numeric(&hir_lhs.ty, &hir_rhs.ty)
                } else {
                    None
                };

                if hir_lhs.ty != hir_rhs.ty {
                    if common_numeric_ty.is_none() {
                        self.errors.push(TypeError { message: format!(
                            "Binary operation between mismatched types: expected {} but found {}",
                            Typechecker::format_ty(&hir_lhs.ty), Typechecker::format_ty(&hir_rhs.ty)
                        ), context: context.clone() });
                    }
                }

                let result_ty = match op_kind {
                    hir::BinaryOp::Add
                    | hir::BinaryOp::Sub
                    | hir::BinaryOp::Mul
                    | hir::BinaryOp::Div
                    | hir::BinaryOp::Mod
                    | hir::BinaryOp::BitShiftLeft
                    | hir::BinaryOp::BitShiftRight
                    | hir::BinaryOp::Xor => common_numeric_ty.clone().unwrap_or(hir_lhs.ty.clone()),
                    hir::BinaryOp::Assign => hir_lhs.ty.clone(),
                    hir::BinaryOp::Eq
                    | hir::BinaryOp::Ne
                    | hir::BinaryOp::Lt
                    | hir::BinaryOp::Lte
                    | hir::BinaryOp::Gt
                    | hir::BinaryOp::Gte
                    | hir::BinaryOp::And
                    | hir::BinaryOp::Or => hir::Ty::Primitive(hir::PrimitiveTy::Bool),
                };

                let (hir_lhs, hir_rhs) = if let Some(common_ty) = common_numeric_ty {
                    (
                        Self::cast_numeric_expr(hir_lhs, common_ty.clone()),
                        Self::cast_numeric_expr(hir_rhs, common_ty),
                    )
                } else {
                    (hir_lhs, hir_rhs)
                };

                Ok(hir::Expr {
                    kind: hir::ExprKind::Binary {
                        op: self.lower_binary_op(op),
                        lhs: Box::new(hir_lhs),
                        rhs: Box::new(hir_rhs),
                    },
                    ty: result_ty,
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::FieldAccess { receiver, field } => {
                if let OwnedExpr::Path(path) = &receiver.item {
                    if let Some(base) = path.last() {
                        match self.lookup_symbol(base).cloned() {
                            Some(_) => {}
                            None => {
                                if let Some(Symbol::Function {
                                    signature,
                                    is_public,
                                    defined_in,
                                    decl_span: _,
                                }) = self.lookup_symbol(&field).cloned()
                                {
                                    if !is_public && defined_in != context.path {
                                        self.errors.push(TypeError {
                                            message: format!("Function `{}` is private", field),
                                            context: context.clone(),
                                        });
                                        return Err(());
                                    }
                                    return Ok(hir::Expr {
                                        kind: hir::ExprKind::Path(vec![field]),
                                        ty: hir::Ty::Function {
                                            param_types: signature
                                                .params
                                                .iter()
                                                .map(|p| p.ty.clone())
                                                .collect(),
                                            ret_type: Box::new(signature.ret_type.clone()),
                                            effects: signature.effects.clone(),
                                        },
                                        span: expr.span,
                                        resolution: None,
                                    });
                                }
                                self.errors.push(TypeError {
                                    message: format!("Cannot find value `{}` in this scope", base),
                                    context: context.clone(),
                                });
                                return Err(());
                            }
                        }
                    }
                }

                let recv_hir = self.lower_expr(*receiver, context.clone())?;
                if let hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) = recv_hir.ty.clone() {
                    if let Some(ty) = self.lookup_struct_field_type(&name, &field).cloned() {
                        return Ok(hir::Expr {
                            kind: hir::ExprKind::FieldAccess {
                                receiver: Box::new(recv_hir),
                                field: field.clone(),
                            },
                            ty,
                            span: expr.span,
                            resolution: Some(hir::Resolution::Field { owner: name, field }),
                        });
                    }
                }
                self.errors.push(TypeError {
                    message: "Unknown field access on non-record type or missing field".to_string(),
                    context: context.clone(),
                });
                Err(())
            }
            OwnedExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                // UFCS desugaring: m.f(x,y) => f(m,x,y) or module::f(m,x,y) when needed
                let recv_hir = self.lower_expr(*receiver, context.clone())?;
                // Candidate function names to try: simple `method` and alias-qualified `alias::method`
                let mut candidate_names: Vec<String> = vec![method.clone()];
                if let hir::Ty::Adt(hir::AdtTy::Struct {
                    name: type_path, ..
                })
                | hir::Ty::Adt(hir::AdtTy::Enum {
                    name: type_path, ..
                }) = &recv_hir.ty
                {
                    if type_path.len() >= 1 {
                        // Try to find an import alias matching the module path of the type (all but last segment)
                        let module_path: Vec<String> = type_path
                            .iter()
                            .cloned()
                            .take(type_path.len().saturating_sub(1))
                            .collect();
                        if let Some(alias_map) = self.import_alias_map.get(&context.path) {
                            for (alias, mod_path) in alias_map.iter() {
                                if mod_path == &module_path {
                                    candidate_names.push(format!("{}::{}", alias, method));
                                }
                            }
                        }
                    }
                }

                // Try to resolve a visible function with a compatible first parameter type
                for cand in candidate_names {
                    if let Some(Symbol::Function {
                        signature,
                        is_public,
                        defined_in,
                        decl_span,
                    }) = self.lookup_symbol(&cand).cloned()
                    {
                        if !is_public && defined_in != context.path {
                            self.errors.push(TypeError {
                                message: format!("Function `{}` is private", cand),
                                context: context.clone(),
                            });
                            return Err(());
                        }
                        // Ensure function has at least one param and receiver type matches first param
                        if let Some(first_param) = signature.params.get(0) {
                            // Simple equality check; could be relaxed with unification if needed
                            if first_param.ty == recv_hir.ty {
                                // Lower remaining args against subsequent parameters
                                let mut lowered_args: Vec<hir::Expr> =
                                    Vec::with_capacity(1 + args.len());
                                lowered_args.push(recv_hir.clone());
                                for (idx, arg) in args.into_iter().enumerate() {
                                    let exp = signature.params.get(idx + 1).map(|p| p.ty.clone());
                                    let lowered = if let Some(expected) = exp {
                                        self.lower_expr_with_expected(
                                            arg,
                                            expected,
                                            context.clone(),
                                        )?
                                    } else {
                                        self.lower_expr(arg, context.clone())?
                                    };
                                    lowered_args.push(lowered);
                                }
                                // Build function expr and call
                                let mut fun_path: Vec<String> = vec![method.clone()];
                                if cand.contains("::") {
                                    // preserve qualified lookup spelling for resolution
                                    let parts: Vec<String> =
                                        cand.split("::").map(|s| s.to_string()).collect();
                                    fun_path = parts;
                                }
                                let mut fun_expr = hir::Expr {
                                    kind: hir::ExprKind::Path(fun_path),
                                    ty: hir::Ty::Function {
                                        param_types: signature
                                            .params
                                            .iter()
                                            .map(|p| p.ty.clone())
                                            .collect(),
                                        ret_type: Box::new(signature.ret_type.clone()),
                                        effects: signature.effects.clone(),
                                    },
                                    span: expr.span,
                                    resolution: None,
                                };
                                if let Some(sp) = decl_span {
                                    fun_expr.resolution = Some(hir::Resolution::Function {
                                        defined_in: defined_in.clone(),
                                        span: sp,
                                    });
                                }
                                return Ok(hir::Expr {
                                    ty: signature.ret_type.clone(),
                                    kind: hir::ExprKind::Call {
                                        fun: Box::new(fun_expr),
                                        args: lowered_args,
                                    },
                                    span: expr.span,
                                    resolution: None,
                                });
                            }
                        }
                    }
                }

                self.errors.push(TypeError {
                    message: format!(
                        "Unknown method `{}` for receiver type {}",
                        method,
                        Typechecker::format_ty(&recv_hir.ty)
                    ),
                    context: context.clone(),
                });
                Err(())
            }
            OwnedExpr::Call { fun, args } => {
                match fun.item.clone() {
                    OwnedExpr::Path(path) => {
                        let name = path.last().expect("Path cannot be empty").clone();

                        let mut is_module_qualified = false;
                        let mut adjusted_args: Vec<SpannedExpr> = args.clone();
                        if let Some(first) = args.get(0) {
                            if let OwnedExpr::Path(p) = &first.item {
                                if p.len() == 1 {
                                    let base = p.last().unwrap();
                                    let is_import_alias = self
                                        .imports_by_file
                                        .get(&context.path)
                                        .map(|s| s.contains(base))
                                        .unwrap_or(false);
                                    let is_value_symbol = self.lookup_symbol(base).is_some();
                                    if is_import_alias && !is_value_symbol {
                                        is_module_qualified = true;
                                        adjusted_args = args.iter().cloned().skip(1).collect();
                                    }
                                }
                            }
                        }

                        let mut signature_opt: Option<hir::HirFunctionSignature> = None;
                        // Handle module-qualified names via import aliases: alias::name
                        if path.len() == 2 {
                            let alias = &path[0];
                            if let Some(alias_map) = self.import_alias_map.get(&context.path) {
                                if let Some(_mod_path) = alias_map.get(alias) {
                                    let qual = format!("{}::{}", alias, name);
                                    if let Some(Symbol::Function {
                                        signature,
                                        is_public,
                                        defined_in,
                                        ..
                                    }) = self.lookup_symbol(&qual)
                                    {
                                        if !is_public && *defined_in != context.path {
                                            self.errors.push(TypeError {
                                                message: format!("Function `{}` is private", qual),
                                                context: context.clone(),
                                            });
                                            return Err(());
                                        }
                                        signature_opt = Some(signature.clone());
                                    }
                                }
                            }
                        }
                        if signature_opt.is_none() {
                            signature_opt = match self.lookup_symbol(&name) {
                                Some(Symbol::Function {
                                    signature,
                                    is_public,
                                    defined_in,
                                    decl_span: _,
                                }) => {
                                    if !is_public && *defined_in != context.path {
                                        self.errors.push(TypeError {
                                            message: format!("Function `{}` is private", name),
                                            context: context.clone(),
                                        });
                                        None
                                    } else {
                                        Some(signature.clone())
                                    }
                                }
                                // try module-qualified name like fmt::x; enforce visibility
                                None if path.len() == 2 => {
                                    let qual = format!("{}::{}", path[0], path[1]);
                                    match self.lookup_symbol(&qual) {
                                        Some(Symbol::Function {
                                            signature,
                                            is_public,
                                            defined_in,
                                            ..
                                        }) => {
                                            if !is_public && *defined_in != context.path {
                                                self.errors.push(TypeError {
                                                    message: format!(
                                                        "Function `{}` is private",
                                                        qual
                                                    ),
                                                    context: context.clone(),
                                                });
                                                None
                                            } else {
                                                Some(signature.clone())
                                            }
                                        }
                                        _ => None,
                                    }
                                }
                                _ => None,
                            };
                        }

                        if let Some(signature) = signature_opt {
                            let mut lowered_args = Vec::new();
                            for (idx, arg) in adjusted_args.into_iter().enumerate() {
                                let arg_expr = if let Some(p) = signature.params.get(idx) {
                                    self.lower_expr_with_expected(
                                        arg,
                                        p.ty.clone(),
                                        context.clone(),
                                    )?
                                } else {
                                    self.lower_expr(arg, context.clone())?
                                };
                                lowered_args.push(arg_expr);
                            }
                            if lowered_args.len() != signature.params.len() {
                                self.errors.push(TypeError {
                                    message: format!(
                                        "Function `{}` expects {} args, found {}",
                                        signature.name,
                                        signature.params.len(),
                                        lowered_args.len()
                                    ),
                                    context: context.clone(),
                                });
                            }
                            // Attach semantic resolution for top-level function or method calls
                            let mut fun_resolution: Option<hir::Resolution> = None;
                            if is_module_qualified {
                                if let Some(Symbol::Function {
                                    signature: sig,
                                    is_public,
                                    defined_in,
                                    decl_span,
                                }) = self.lookup_symbol(&name).cloned()
                                {
                                    if is_public {
                                        if let Some(sp) = decl_span {
                                            fun_resolution = Some(hir::Resolution::Function {
                                                defined_in,
                                                span: sp,
                                            });
                                        }
                                    }
                                }
                            }
                            let mut fun_expr = hir::Expr {
                                kind: hir::ExprKind::Path(path),
                                ty: hir::Ty::Function {
                                    param_types: signature
                                        .params
                                        .iter()
                                        .map(|p| p.ty.clone())
                                        .collect(),
                                    ret_type: Box::new(signature.ret_type.clone()),
                                    effects: signature.effects.clone(),
                                },
                                span: fun.span,
                                resolution: None,
                            };
                            if let Some(res) = fun_resolution.clone() {
                                fun_expr.resolution = Some(res);
                            }
                            return Ok(hir::Expr {
                                ty: signature.ret_type.clone(),
                                kind: hir::ExprKind::Call {
                                    fun: Box::new(fun_expr),
                                    args: lowered_args,
                                },
                                span: expr.span,
                                resolution: fun_resolution,
                            });
                        }

                        // Fallback: calling a variable of function type (e.g., `task()`)
                        if signature_opt.is_none() {
                            if let Ok(fun_hir) = self.lower_expr(*fun.clone(), context.clone()) {
                                if let hir::Ty::Function {
                                    param_types,
                                    ret_type,
                                    effects,
                                } = fun_hir.ty.clone()
                                {
                                    let mut lowered_args = Vec::new();
                                    for (idx, arg) in adjusted_args.into_iter().enumerate() {
                                        let arg_expr = if let Some(p) = param_types.get(idx) {
                                            self.lower_expr_with_expected(
                                                arg,
                                                p.clone(),
                                                context.clone(),
                                            )?
                                        } else {
                                            self.lower_expr(arg, context.clone())?
                                        };
                                        lowered_args.push(arg_expr);
                                    }
                                    if lowered_args.len() != param_types.len() {
                                        self.errors.push(TypeError {
                                            message: format!(
                                                "Function value expects {} args, found {}",
                                                param_types.len(),
                                                lowered_args.len()
                                            ),
                                            context: context.clone(),
                                        });
                                    }
                                    return Ok(hir::Expr {
                                        ty: (*ret_type).clone(),
                                        kind: hir::ExprKind::Call {
                                            fun: Box::new(fun_hir),
                                            args: lowered_args,
                                        },
                                        span: expr.span,
                                        resolution: None,
                                    });
                                }
                            }
                        }

                        if let Some((union_path, payload_types)) = self.find_union_variant(&name) {
                            let ret_ty = hir::Ty::Adt(hir::AdtTy::Enum {
                                name: union_path.clone(),
                                generics: vec![],
                            });
                            let expected_payload_ty =
                                payload_types.as_ref().and_then(|v| v.get(0)).cloned();
                            let mut lowered_args = Vec::new();
                            if let Some(exp_ty) = expected_payload_ty.clone() {
                                if let Some(first) = adjusted_args.get(0) {
                                    let lowered = self.lower_expr_with_expected(
                                        first.clone(),
                                        exp_ty.clone(),
                                        context.clone(),
                                    )?;
                                    lowered_args.push(lowered);
                                } else {
                                    self.errors.push(TypeError {
                                        message: format!("Variant `{}` expects 1 argument", name),
                                        context: context.clone(),
                                    });
                                }
                            }
                            let fun_expr = hir::Expr {
                                kind: hir::ExprKind::Path(vec![name]),
                                ty: hir::Ty::Function {
                                    param_types: expected_payload_ty
                                        .map(|t| vec![t])
                                        .unwrap_or_default(),
                                    ret_type: Box::new(ret_ty.clone()),
                                    effects: vec![],
                                },
                                span: fun.span,
                                resolution: None,
                            };
                            return Ok(hir::Expr {
                                ty: ret_ty,
                                kind: hir::ExprKind::Call {
                                    fun: Box::new(fun_expr),
                                    args: lowered_args,
                                },
                                span: expr.span,
                                resolution: None,
                            });
                        }

                        self.errors.push(TypeError {
                            message: format!("Unknown function or constructor `{}`", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    _ => {
                        let fun_hir = self.lower_expr(*fun, context.clone())?;
                        let params = match &fun_hir.ty {
                            hir::Ty::Function { param_types, .. } => param_types.clone(),
                            other => {
                                self.errors.push(TypeError {
                                    message: format!(
                                        "Attempted to call a non-function value of type {}",
                                        Typechecker::format_ty(&other)
                                    ),
                                    context: context.clone(),
                                });
                                vec![]
                            }
                        };
                        let mut lowered_args = Vec::new();
                        for (idx, arg) in args.into_iter().enumerate() {
                            let arg_expr = if let Some(expected) = params.get(idx) {
                                self.lower_expr_with_expected(
                                    arg,
                                    expected.clone(),
                                    context.clone(),
                                )?
                            } else {
                                self.lower_expr(arg, context.clone())?
                            };
                            lowered_args.push(arg_expr);
                        }
                        let ret_ty = match &fun_hir.ty {
                            hir::Ty::Function { ret_type, .. } => (*ret_type.clone()).clone(),
                            _ => hir::Ty::Special(hir::SpecialTy::Unit),
                        };
                        Ok(hir::Expr {
                            ty: ret_ty,
                            kind: hir::ExprKind::Call {
                                fun: Box::new(fun_hir),
                                args: lowered_args,
                            },
                            span: expr.span,
                            resolution: None,
                        })
                    }
                }
            }
            OwnedExpr::FnLiteral {
                params,
                ret_type,
                effects,
                body,
            } => {
                // Lower a function literal into an explicit HirFnLiteral
                let mut lowered_params: Vec<hir::HirParam> = Vec::new();
                let mut param_types: Vec<hir::Ty> = Vec::new();
                for (name_opt, ty) in params {
                    let t = self.resolve_type(&ty, context.clone())?;
                    param_types.push(t.clone());
                    lowered_params.push(hir::HirParam {
                        name: name_opt.unwrap_or("_".to_string()),
                        ty: t,
                        span: None,
                    });
                }
                let ret_ty = if let Some(rt) = ret_type {
                    self.resolve_type(&rt, context.clone())?
                } else {
                    hir::Ty::Special(hir::SpecialTy::Unit)
                };
                let mut eff_tys: Vec<hir::Ty> = Vec::new();
                for e in effects {
                    eff_tys.push(self.resolve_type(&e, context.clone())?);
                }
                self.current_effects_stack.push(eff_tys.clone());
                let body_hir_expr_result = self.lower_expr(*body, context.clone());
                let _ = self.current_effects_stack.pop();
                let body_hir_expr = body_hir_expr_result?;
                let body_block = match body_hir_expr.kind {
                    hir::ExprKind::Block(b) => b,
                    other => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(hir::Expr {
                            kind: other,
                            ty: body_hir_expr.ty.clone(),
                            span: body_hir_expr.span,
                            resolution: None,
                        })),
                        ty: body_hir_expr.ty.clone(),
                    },
                };
                let lit = hir::HirFnLiteral {
                    params: lowered_params,
                    ret_type: ret_ty.clone(),
                    effects: eff_tys.clone(),
                    body: body_block,
                };
                let fn_ty = hir::Ty::Function {
                    param_types,
                    ret_type: Box::new(ret_ty),
                    effects: eff_tys,
                };
                Ok(hir::Expr {
                    kind: hir::ExprKind::FnLiteral(lit),
                    ty: fn_ty,
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Block { stmts, last_expr } => {
                self.enter_scope();
                let mut hir_stmts: Vec<hir::Stmt> = Vec::new();
                for s in stmts.into_iter() {
                    if let Ok(stmt) = self.lower_stmt(s, context.clone()) {
                        hir_stmts.push(stmt);
                    }
                }
                let mut inferred_last_expr: Option<Box<hir::Expr>> = None;
                if last_expr.is_none() {
                    if let Some(hir::Stmt::Expr { expr: e, .. }) = hir_stmts.last() {
                        inferred_last_expr = Some(Box::new(e.clone()));
                    }
                    if inferred_last_expr.is_some() {
                        hir_stmts.pop();
                    }
                }
                let (hir_last_expr, block_ty) = match (last_expr, inferred_last_expr) {
                    (Some(expr), _) => {
                        let hir_expr = self.lower_expr(*expr, context.clone())?;
                        let ty = hir_expr.ty.clone();
                        (Some(Box::new(hir_expr)), ty)
                    }
                    (None, Some(inf)) => {
                        let ty = inf.ty.clone();
                        (Some(inf), ty)
                    }
                    (None, None) => (None, hir::Ty::Special(hir::SpecialTy::Unit)),
                };
                self.leave_scope();
                let block = hir::HirBlock {
                    stmts: hir_stmts,
                    last_expr: hir_last_expr,
                    ty: block_ty.clone(),
                };
                Ok(hir::Expr {
                    kind: hir::ExprKind::Block(block),
                    ty: block_ty,
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.errors.push(TypeError {
                        message: "If condition must be a bool".to_string(),
                        context: context.clone(),
                    });
                }
                let then_hir = self.lower_expr(*then_block, context.clone())?;
                let else_hir_opt = if let Some(e) = else_block {
                    Some(Box::new(self.lower_expr(*e, context.clone())?))
                } else {
                    None
                };
                let then_block = match then_hir.kind.clone() {
                    hir::ExprKind::Block(b) => b,
                    _ => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(then_hir.clone())),
                        ty: then_hir.ty.clone(),
                    },
                };
                let result_ty = if let Some(ref else_hir) = else_hir_opt {
                    match Self::branch_result_ty(&then_hir.ty, &else_hir.ty) {
                        Some(ty) => ty,
                        None => {
                            self.errors.push(TypeError {
                                message: format!(
                                    "If branches must have same type: then={}, else={}",
                                    Typechecker::format_ty(&then_hir.ty),
                                    Typechecker::format_ty(&else_hir.ty)
                                ),
                                context: context.clone(),
                            });
                            else_hir.ty.clone()
                        }
                    }
                } else {
                    hir::Ty::Special(hir::SpecialTy::Unit)
                };
                Ok(hir::Expr {
                    ty: result_ty,
                    kind: hir::ExprKind::If {
                        cond: Box::new(cond_hir),
                        then_block,
                        else_block: else_hir_opt,
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::While { cond, body } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.errors.push(TypeError {
                        message: "While condition must be a bool".to_string(),
                        context: context.clone(),
                    });
                }
                let body_hir = self.lower_expr(*body, context.clone())?;
                let body_block = match body_hir.kind {
                    hir::ExprKind::Block(b) => b,
                    _ => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(body_hir.clone())),
                        ty: body_hir.ty.clone(),
                    },
                };
                Ok(hir::Expr {
                    ty: hir::Ty::Special(hir::SpecialTy::Unit),
                    kind: hir::ExprKind::While {
                        cond: Box::new(cond_hir),
                        body: body_block,
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Match { scrutinee, arms } => {
                let scrutinee_hir = self.lower_expr(*scrutinee, context.clone())?;
                let mut lowered_arms = Vec::new();
                let mut result_ty: Option<hir::Ty> = None;
                for (pat, arm_expr) in arms {
                    self.enter_scope();
                    let (hir_pat, hir_arm_expr) =
                        self.lower_match_arm(pat, arm_expr, &scrutinee_hir.ty, context.clone())?;
                    if let Some(ref ty) = result_ty {
                        match Self::branch_result_ty(ty, &hir_arm_expr.ty) {
                            Some(common) => result_ty = Some(common),
                            None => {
                                self.errors.push(TypeError {
                                    message: format!(
                                        "Match arms must have the same type: expected {}, found {}",
                                        Typechecker::format_ty(ty),
                                        Typechecker::format_ty(&hir_arm_expr.ty)
                                    ),
                                    context: context.clone(),
                                });
                            }
                        }
                    } else {
                        result_ty = Some(hir_arm_expr.ty.clone());
                    }
                    lowered_arms.push((hir_pat, hir_arm_expr));
                    self.leave_scope();
                }
                let result_ty = result_ty.unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                Ok(hir::Expr {
                    ty: result_ty,
                    kind: hir::ExprKind::Match {
                        scrutinee: Box::new(scrutinee_hir),
                        arms: lowered_arms,
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::UnionInit {
                path,
                variant,
                fields,
            } => {
                let mut init_path = path;
                init_path.push(variant);
                self.lower_expr(
                    Spanned {
                        item: OwnedExpr::StructInit {
                            path: init_path,
                            generics: vec![],
                            fields,
                        },
                        span: expr.span,
                    },
                    context,
                )
            }
            OwnedExpr::Perform { path, args } => {
                // Enforce that this perform is allowed in the current function's effect list
                let effect_name = path.get(0).cloned().unwrap_or_default();
                if let Some(current_allowed) = self.current_effects_stack.last() {
                    let mut effect_allowed = false;
                    for eff in current_allowed {
                        match eff {
                            hir::Ty::Adt(hir::AdtTy::Effect { name, .. }) => {
                                if name.last().map(|s| s == &effect_name).unwrap_or(false) {
                                    effect_allowed = true;
                                    break;
                                }
                            }
                            hir::Ty::Generic(n) => {
                                if n == &effect_name {
                                    effect_allowed = true;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    if !effect_allowed {
                        self.errors.push(TypeError { message: format!("Effect `{}` is not allowed here; add it to the function's effect list", effect_name), context: context.clone() });
                    }
                }
                let Some((mut ret_ty, param_tys)) = self.resolve_effect_op(&path) else {
                    self.errors.push(TypeError {
                        message: format!("Unknown effect operation `{}`", path.join(".")),
                        context: context.clone(),
                    });
                    return Err(());
                };
                let mut lowered_args = Vec::new();
                for (idx, a) in args.into_iter().enumerate() {
                    let lowered = if let Some(param_ty) = param_tys.get(idx) {
                        self.lower_expr_with_expected(a.clone(), param_ty.clone(), context.clone())
                    } else {
                        self.lower_expr(a.clone(), context.clone())
                    };
                    match lowered {
                        Ok(e) => lowered_args.push(e),
                        Err(_) => lowered_args.push(hir::Expr {
                            kind: hir::ExprKind::Error,
                            ty: hir::Ty::Generic("_unknown".to_string()),
                            span: a.span,
                            resolution: None,
                        }),
                    }
                }
                if lowered_args.len() != param_tys.len() {
                    self.errors.push(TypeError {
                        message: format!(
                            "Effect operation `{}` expects {} args, found {}",
                            path.join("."),
                            param_tys.len(),
                            lowered_args.len()
                        ),
                        context: context.clone(),
                    });
                }
                // Specialize Async.await(task: () -> T) -> T based on argument
                if path.len() == 2 && path[0] == "Async" && path[1] == "await" {
                    if let Some(task_expr) = lowered_args.get(0) {
                        if let hir::Ty::Function { ret_type, .. } = &task_expr.ty {
                            ret_ty = (*ret_type.clone()).clone();
                        }
                    }
                }
                Ok(hir::Expr {
                    ty: ret_ty,
                    kind: hir::ExprKind::Perform {
                        path,
                        args: lowered_args,
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Handle { body, handler } => {
                let handler_hir =
                    self.lower_handler_body_expr(handler, context.clone(), expr.span)?;
                let handler_effects = match &handler_hir.ty {
                    hir::Ty::Handler { effects } => effects.clone(),
                    _ => vec![],
                };

                let pushed_effect_scope =
                    if let Some(current_allowed) = self.current_effects_stack.last().cloned() {
                        let extended = Self::merge_effects(current_allowed, &handler_effects);
                        self.current_effects_stack.push(extended);
                        true
                    } else {
                        false
                    };

                let body_result = if let OwnedExpr::Path(path) = body.item.clone() {
                    match self.lower_handler_value_path_expr(path, body.span) {
                        Some(handler_value) => Ok(handler_value),
                        None => self.lower_expr(*body, context.clone()),
                    }
                } else {
                    self.lower_expr(*body, context.clone())
                };
                if pushed_effect_scope {
                    let _ = self.current_effects_stack.pop();
                }
                let body_hir = body_result?;

                let body_effects = self.expr_effects(&body_hir);
                for required in &body_effects {
                    if !handler_effects
                        .iter()
                        .any(|handled| Typechecker::same_effect_name(required, handled))
                    {
                        self.errors.push(TypeError {
                            message: format!(
                                "Handler does not cover effect `{}`",
                                Typechecker::format_ty(required)
                            ),
                            context: context.clone(),
                        });
                    }
                }

                let result_ty = if let hir::Ty::Handler {
                    effects: body_handler_effects,
                } = body_hir.ty.clone()
                {
                    hir::Ty::Handler {
                        effects: body_handler_effects
                            .into_iter()
                            .filter(|effect| {
                                !handler_effects
                                    .iter()
                                    .any(|handled| Typechecker::same_effect_name(effect, handled))
                            })
                            .collect(),
                    }
                } else {
                    body_hir.ty.clone()
                };

                let body_block = match body_hir.kind.clone() {
                    hir::ExprKind::Block(b) => b,
                    _ => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(body_hir)),
                        ty: result_ty.clone(),
                    },
                };

                Ok(hir::Expr {
                    ty: result_ty,
                    kind: hir::ExprKind::Handle {
                        body: body_block,
                        handler: Box::new(handler_hir),
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Cast { expr: inner, ty } => {
                let inner_hir = self.lower_expr(*inner, context.clone())?;
                let target_ty = self.resolve_type(&ty, context.clone())?;
                Ok(hir::Expr {
                    ty: target_ty.clone(),
                    kind: hir::ExprKind::Cast {
                        expr: Box::new(inner_hir),
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::StructInit { path, fields, .. } => {
                if let Some((enum_path, variant, variants)) =
                    path.split_last().and_then(|(variant, rest)| {
                        if rest.is_empty() {
                            None
                        } else {
                            let enum_path = rest.to_vec();
                            self.union_variants
                                .get(&enum_path)
                                .map(|variants| (enum_path, variant.clone(), variants.clone()))
                        }
                    })
                {
                    let Some((_, payload)) = variants.iter().find(|(name, _)| name == &variant)
                    else {
                        self.errors.push(TypeError {
                            message: format!(
                                "Unknown variant `{}` for enum `{}`",
                                variant,
                                enum_path.join("::")
                            ),
                            context: context.clone(),
                        });
                        return Err(());
                    };
                    let payload = payload.clone().unwrap_or_default();
                    let field_defs = if let [
                        hir::Ty::Adt(hir::AdtTy::Struct {
                            name: payload_struct,
                            ..
                        }),
                    ] = payload.as_slice()
                    {
                        match self.type_definitions.get(payload_struct) {
                            Some(hir::Item::Struct(struct_def)) => struct_def.fields.clone(),
                            _ => vec![],
                        }
                    } else {
                        vec![]
                    };

                    let mut lowered_fields = Vec::new();
                    for (name, expr) in fields {
                        let e = if let Some(field_def) =
                            field_defs.iter().find(|field| field.name == name)
                        {
                            self.lower_expr_with_expected(
                                expr,
                                field_def.ty.clone(),
                                context.clone(),
                            )?
                        } else {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Unknown field `{}` for variant `{}`",
                                    name,
                                    path.join("::")
                                ),
                                context: context.clone(),
                            });
                            self.lower_expr(expr, context.clone())?
                        };
                        lowered_fields.push((name, e));
                    }
                    for field_def in &field_defs {
                        if !lowered_fields
                            .iter()
                            .any(|(field_name, _)| field_name == &field_def.name)
                        {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Missing field `{}` for variant `{}`",
                                    field_def.name,
                                    path.join("::")
                                ),
                                context: context.clone(),
                            });
                        }
                    }

                    return Ok(hir::Expr {
                        ty: hir::Ty::Adt(hir::AdtTy::Enum {
                            name: enum_path,
                            generics: vec![],
                        }),
                        kind: hir::ExprKind::StructInit {
                            path,
                            fields: lowered_fields,
                        },
                        span: expr.span,
                        resolution: None,
                    });
                }

                let Some(hir::Item::Struct(struct_def)) = self.type_definitions.get(&path).cloned()
                else {
                    self.errors.push(TypeError {
                        message: format!("Unknown struct `{}`", path.join("::")),
                        context: context.clone(),
                    });
                    return Err(());
                };
                let adt_ty = hir::Ty::Adt(hir::AdtTy::Struct {
                    name: path.clone(),
                    generics: vec![],
                });
                let mut lowered_fields = Vec::new();
                for (name, expr) in fields {
                    let e = if let Some(field_def) =
                        struct_def.fields.iter().find(|field| field.name == name)
                    {
                        self.lower_expr_with_expected(expr, field_def.ty.clone(), context.clone())?
                    } else {
                        self.errors.push(TypeError {
                            message: format!(
                                "Unknown field `{}` for struct `{}`",
                                name,
                                path.join("::")
                            ),
                            context: context.clone(),
                        });
                        self.lower_expr(expr, context.clone())?
                    };
                    lowered_fields.push((name, e));
                }
                for field_def in &struct_def.fields {
                    if !lowered_fields
                        .iter()
                        .any(|(field_name, _)| field_name == &field_def.name)
                    {
                        self.errors.push(TypeError {
                            message: format!(
                                "Missing field `{}` for struct `{}`",
                                field_def.name,
                                path.join("::")
                            ),
                            context: context.clone(),
                        });
                    }
                }
                Ok(hir::Expr {
                    ty: adt_ty.clone(),
                    kind: hir::ExprKind::StructInit {
                        path,
                        fields: lowered_fields,
                    },
                    span: expr.span,
                    resolution: None,
                })
            }
            OwnedExpr::Error => Ok(hir::Expr {
                kind: hir::ExprKind::Error,
                ty: hir::Ty::Special(hir::SpecialTy::Unit),
                span: expr.span,
                resolution: None,
            }),
        }
    }

    pub(crate) fn lower_expr_with_expected(
        &mut self,
        expr: SpannedExpr,
        expected: hir::Ty,
        context: ItemContext,
    ) -> Result<hir::Expr, ()> {
        match (expr.item.clone(), expected.clone()) {
            (OwnedExpr::Call { fun, args }, hir::Ty::Adt(hir::AdtTy::Enum { name, generics })) => {
                if let OwnedExpr::Path(path) = &fun.item {
                    if path.len() == 1 {
                        let variant_name = &path[0];
                        if let Some(payload_types) =
                            self.instantiated_union_payload(&name, &generics, variant_name)
                        {
                            let ret_ty = hir::Ty::Adt(hir::AdtTy::Enum {
                                name: name.clone(),
                                generics: generics.clone(),
                            });
                            let expected_payload_ty =
                                payload_types.as_ref().and_then(|v| v.get(0)).cloned();
                            let mut lowered_args = Vec::new();
                            if let Some(exp_ty) = expected_payload_ty.clone() {
                                if let Some(first) = args.get(0) {
                                    lowered_args.push(self.lower_expr_with_expected(
                                        first.clone(),
                                        exp_ty.clone(),
                                        context.clone(),
                                    )?);
                                } else {
                                    self.errors.push(TypeError {
                                        message: format!(
                                            "Variant `{}` expects 1 argument",
                                            variant_name
                                        ),
                                        context: context.clone(),
                                    });
                                }
                            }
                            let fun_expr = hir::Expr {
                                kind: hir::ExprKind::Path(vec![variant_name.clone()]),
                                ty: hir::Ty::Function {
                                    param_types: expected_payload_ty
                                        .map(|ty| vec![ty])
                                        .unwrap_or_default(),
                                    ret_type: Box::new(ret_ty.clone()),
                                    effects: vec![],
                                },
                                span: fun.span,
                                resolution: None,
                            };
                            return Ok(hir::Expr {
                                ty: ret_ty,
                                kind: hir::ExprKind::Call {
                                    fun: Box::new(fun_expr),
                                    args: lowered_args,
                                },
                                span: expr.span,
                                resolution: None,
                            });
                        }
                    }
                }
                self.lower_expr(expr, context)
            }
            (
                OwnedExpr::StructInit {
                    path,
                    generics: init_generics,
                    fields,
                },
                hir::Ty::Adt(hir::AdtTy::Enum { name, generics }),
            ) if path.len() == name.len() + 1 && path.starts_with(&name) => {
                let variant_name = path.last().cloned().unwrap_or_default();
                if self
                    .instantiated_union_payload(&name, &generics, &variant_name)
                    .is_some()
                {
                    let mut lowered = self.lower_expr(
                        Spanned {
                            item: OwnedExpr::StructInit {
                                path,
                                generics: init_generics,
                                fields,
                            },
                            span: expr.span,
                        },
                        context,
                    )?;
                    lowered.ty = hir::Ty::Adt(hir::AdtTy::Enum { name, generics });
                    return Ok(lowered);
                }
                self.lower_expr(expr, context)
            }
            (OwnedExpr::Literal(lit), hir::Ty::Primitive(exp_prim)) => {
                if let Some((pty, s)) = self.coerce_numeric_literal(&lit, exp_prim.clone()) {
                    return Ok(hir::Expr {
                        ty: hir::Ty::Primitive(pty.clone()),
                        kind: hir::ExprKind::Literal(pty, s),
                        span: expr.span,
                        resolution: None,
                    });
                }
                self.lower_expr(
                    Spanned {
                        item: OwnedExpr::Literal(lit),
                        span: expr.span,
                    },
                    context,
                )
            }
            (
                OwnedExpr::Block {
                    mut stmts,
                    last_expr,
                },
                expected_ty,
            ) => {
                self.enter_scope();
                let mut hir_stmts: Vec<hir::Stmt> = Vec::new();
                let mut trailing_expr_opt: Option<SpannedExpr> = None;
                if last_expr.is_none() {
                    if let Some(last) = stmts.last() {
                        if let OwnedStmt::Expr(e) = &last.item {
                            trailing_expr_opt = Some(e.clone());
                            stmts.pop();
                        }
                    }
                }
                for s in stmts.into_iter() {
                    if let Ok(stmt) = self.lower_stmt(s, context.clone()) {
                        hir_stmts.push(stmt);
                    }
                }
                let (hir_last_expr, block_ty) = if let Some(expr) = last_expr {
                    let lowered =
                        self.lower_expr_with_expected(*expr, expected_ty.clone(), context.clone())?;
                    (Some(Box::new(lowered.clone())), lowered.ty)
                } else if let Some(expr) = trailing_expr_opt {
                    let lowered =
                        self.lower_expr_with_expected(expr, expected_ty.clone(), context.clone())?;
                    (Some(Box::new(lowered.clone())), lowered.ty)
                } else {
                    (None, hir::Ty::Special(hir::SpecialTy::Unit))
                };
                self.leave_scope();
                Ok(hir::Expr {
                    ty: block_ty.clone(),
                    kind: hir::ExprKind::Block(hir::HirBlock {
                        stmts: hir_stmts,
                        last_expr: hir_last_expr,
                        ty: block_ty,
                    }),
                    span: expr.span,
                    resolution: None,
                })
            }
            _ => {
                let lowered = self.lower_expr(expr, context.clone())?;
                if lowered.ty == expected
                    || matches!(lowered.ty, hir::Ty::Special(hir::SpecialTy::Never))
                {
                    Ok(lowered)
                } else if TypeUnifier::is_assignable(&lowered.ty, &expected) {
                    Ok(Self::cast_numeric_expr(lowered, expected))
                } else {
                    Ok(lowered)
                }
            }
        }
    }
}
