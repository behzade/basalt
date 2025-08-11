use crate::ast::BinaryOp;
use crate::ast_owned::*;
use crate::hir;
use crate::type_unifier::TypeUnifier;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;

impl Typechecker {
    pub(crate) fn lower_expr(&mut self, expr: SpannedExpr, context: ItemContext) -> Result<hir::Expr, ()> {
        match expr.item {
            OwnedExpr::Literal(lit) => {
                match lit.clone() {
                    OwnedLiteral::Unit => Ok(hir::Expr {
                        ty: hir::Ty::Special(hir::SpecialTy::Unit),
                        kind: hir::ExprKind::Block(hir::HirBlock { stmts: vec![], last_expr: None, ty: hir::Ty::Special(hir::SpecialTy::Unit) }),
                        span: expr.span,
                        resolution: None,
                    }),
                    _ => {
                        let (ty, val_str) = self.lower_literal(lit);
                        Ok(hir::Expr { kind: hir::ExprKind::Literal(ty.clone(), val_str), ty: hir::Ty::Primitive(ty), span: expr.span, resolution: None })
                    }
                }
            }
            OwnedExpr::Path(path) => {
                let name = path.last().cloned().expect("Path cannot be empty");
                let sym_opt = self.lookup_symbol(&name).cloned();
                match sym_opt {
                    Some(Symbol::Variable { ty, initialized, decl_span, .. }) => {
                        if !initialized {
                            self.errors.push(TypeError { message: format!("Use of variable '{}' before initialization", name), context: context.clone() });
                        }
                        Ok(hir::Expr { kind: hir::ExprKind::Path(path), ty: ty.clone(), span: expr.span, resolution: Some(hir::Resolution::Local { name: name.clone(), decl_span }) })
                    }
                    Some(Symbol::Function { signature, is_public, defined_in, decl_span: _ }) => {
                        if !is_public && &defined_in != &context.path {
                            self.errors.push(TypeError { message: format!("Function `{}` is private", name), context: context.clone() });
                            return Err(());
                        }
                        Ok(hir::Expr { kind: hir::ExprKind::Path(path), ty: hir::Ty::Function { param_types: signature.params.iter().map(|p| p.ty.clone()).collect(), ret_type: Box::new(signature.ret_type.clone()), effects: signature.effects.clone() }, span: expr.span, resolution: None })
                    }
                    None => {
                        if let Some(names) = self.imports_by_file.get(&context.path) {
                            if names.contains(&name) {
                                return Ok(hir::Expr { kind: hir::ExprKind::Path(path), ty: hir::Ty::Special(hir::SpecialTy::Unit), span: expr.span, resolution: None });
                            }
                        }
                        self.errors.push(TypeError { message: format!("Cannot find value `{}` in this scope", name), context: context.clone() });
                        Err(())
                    }
                    _ => {
                        self.errors.push(TypeError { message: format!("`{}` is not a value", name), context: context.clone() });
                        Err(())
                    }
                }
            }
            OwnedExpr::Unary { op, rhs } => {
                let hir_rhs = self.lower_expr(*rhs, context.clone())?;
                let (result_ty, hir_op) = match op {
                    crate::ast::UnaryOp::Neg => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::I32)
                        | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                        | hir::Ty::Primitive(hir::PrimitiveTy::F64) => (hir_rhs.ty.clone(), hir::UnaryOp::Negate),
                        other => {
                            self.errors.push(TypeError { message: format!("Unary '-' not supported for type {}", Typechecker::format_ty(&other)), context: context.clone() });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Negate)
                        }
                    },
                    crate::ast::UnaryOp::Not => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::Bool) => (hir::Ty::Primitive(hir::PrimitiveTy::Bool), hir::UnaryOp::Not),
                        other => {
                            self.errors.push(TypeError { message: format!("Unary '!' not supported for type {}", Typechecker::format_ty(&other)), context: context.clone() });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Not)
                        }
                    },
                };
                Ok(hir::Expr { kind: hir::ExprKind::Unary { op: hir_op, rhs: Box::new(hir_rhs) }, ty: result_ty, span: expr.span, resolution: None })
            }
            OwnedExpr::Binary { op, lhs, rhs } => {
                let (hir_lhs, hir_rhs) = match (&lhs.item, &rhs.item) {
                    (OwnedExpr::Literal(l), other) if self.is_numeric_literal(l) => {
                        let other_hir = self.lower_expr(*rhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_lhs = self.lower_expr_with_expected(Spanned { item: OwnedExpr::Literal(l.clone()), span: lhs.span }, other_hir.ty.clone(), context.clone())?;
                            (coerced_lhs, other_hir)
                        } else {
                            (self.lower_expr(*lhs, context.clone())?, other_hir)
                        }
                    }
                    (other, OwnedExpr::Literal(r)) if self.is_numeric_literal(r) => {
                        let other_hir = self.lower_expr(*lhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_rhs = self.lower_expr_with_expected(Spanned { item: OwnedExpr::Literal(r.clone()), span: rhs.span }, other_hir.ty.clone(), context.clone())?;
                            (other_hir, coerced_rhs)
                        } else {
                            (other_hir, self.lower_expr(*rhs, context.clone())?)
                        }
                    }
                    _ => (self.lower_expr(*lhs, context.clone())?, self.lower_expr(*rhs, context.clone())?),
                };

                if hir_lhs.ty != hir_rhs.ty {
                    let op_kind = self.lower_binary_op(op);
                    let both_numeric = TypeUnifier::is_numeric(&hir_lhs.ty) && TypeUnifier::is_numeric(&hir_rhs.ty);
                    let is_comparison = matches!(op_kind, hir::BinaryOp::Eq | hir::BinaryOp::Ne | hir::BinaryOp::Lt | hir::BinaryOp::Lte | hir::BinaryOp::Gt | hir::BinaryOp::Gte);
                    let is_arithmetic = matches!(op_kind, hir::BinaryOp::Add | hir::BinaryOp::Sub | hir::BinaryOp::Mul | hir::BinaryOp::Div | hir::BinaryOp::Mod | hir::BinaryOp::BitShiftLeft | hir::BinaryOp::BitShiftRight | hir::BinaryOp::Xor);
                    if !(both_numeric && (is_comparison || is_arithmetic)) {
                        self.errors.push(TypeError { message: format!(
                            "Binary operation between mismatched types: expected {} but found {}",
                            Typechecker::format_ty(&hir_lhs.ty), Typechecker::format_ty(&hir_rhs.ty)
                        ), context: context.clone() });
                    }
                }

                let result_ty = match self.lower_binary_op(op) {
                    hir::BinaryOp::Add | hir::BinaryOp::Sub | hir::BinaryOp::Mul | hir::BinaryOp::Div | hir::BinaryOp::Mod | hir::BinaryOp::BitShiftLeft | hir::BinaryOp::BitShiftRight | hir::BinaryOp::Xor => TypeUnifier::unify_numeric(&hir_lhs.ty, &hir_rhs.ty).unwrap_or(hir_lhs.ty.clone()),
                    hir::BinaryOp::Assign => hir_lhs.ty.clone(),
                    hir::BinaryOp::Eq | hir::BinaryOp::Ne | hir::BinaryOp::Lt | hir::BinaryOp::Lte | hir::BinaryOp::Gt | hir::BinaryOp::Gte | hir::BinaryOp::And | hir::BinaryOp::Or => hir::Ty::Primitive(hir::PrimitiveTy::Bool),
                };

                Ok(hir::Expr { kind: hir::ExprKind::Binary { op: self.lower_binary_op(op), lhs: Box::new(hir_lhs), rhs: Box::new(hir_rhs) }, ty: result_ty, span: expr.span, resolution: None })
            }
            OwnedExpr::FieldAccess { receiver, field } => {
                if let OwnedExpr::Path(path) = &receiver.item {
                    if let Some(base) = path.last() {
                        match self.lookup_symbol(base).cloned() {
                            Some(_) => {}
                            None => {
                                if let Some(Symbol::Function { signature, is_public, defined_in, decl_span: _ }) = self.lookup_symbol(&field).cloned() {
                                    if !is_public && defined_in != context.path {
                                        self.errors.push(TypeError { message: format!("Function `{}` is private", field), context: context.clone() });
                                        return Err(());
                                    }
                                    return Ok(hir::Expr { kind: hir::ExprKind::Path(vec![field]), ty: hir::Ty::Function { param_types: signature.params.iter().map(|p| p.ty.clone()).collect(), ret_type: Box::new(signature.ret_type.clone()), effects: signature.effects.clone() }, span: expr.span, resolution: None });
                                }
                                self.errors.push(TypeError { message: format!("Cannot find value `{}` in this scope", base), context: context.clone() });
                                return Err(());
                            }
                        }
                    }
                }

                let recv_hir = self.lower_expr(*receiver, context.clone())?;
                if let hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) = recv_hir.ty.clone() {
                    if let Some(ty) = self.lookup_struct_field_type(&name, &field).cloned() {
                        return Ok(hir::Expr { kind: hir::ExprKind::FieldAccess { receiver: Box::new(recv_hir), field: field.clone() }, ty, span: expr.span, resolution: Some(hir::Resolution::Field { owner: name, field }) });
                    }
                }
                self.errors.push(TypeError { message: "Unknown field access on non-record type or missing field".to_string(), context: context.clone() });
                Err(())
            }
            OwnedExpr::MethodCall { receiver, method, args } => {
                let recv_hir = self.lower_expr(*receiver, context.clone())?;
                // Clone method signature info to avoid holding an immutable borrow of self during arg lowering
                let method_info: Option<(hir::HirFunctionSignature, bool, std::path::PathBuf, crate::token::SimpleSpan)> = match &recv_hir.ty {
                    hir::Ty::Adt(hir::AdtTy::Struct { name: ty_name, .. }) | hir::Ty::Adt(hir::AdtTy::Enum { name: ty_name, .. }) => {
                        self.impl_methods.get(ty_name).and_then(|methods| methods.get(&method).cloned())
                    }
                    _ => None,
                };
                if let Some((sig, is_public, defined_in, span)) = method_info {
                    if !is_public && defined_in != context.path {
                        self.errors.push(TypeError { message: format!("Method `{}` is private", method), context: context.clone() });
                        return Err(());
                    }
                    // Lower args according to signature (skip first param which is the receiver)
                    let mut lowered_args = Vec::new();
                    for (idx, arg) in args.into_iter().enumerate() {
                        let exp = sig.params.get(idx + 1).map(|p| p.ty.clone());
                        let lowered = if let Some(expected) = exp { self.lower_expr_with_expected(arg, expected, context.clone())? } else { self.lower_expr(arg, context.clone())? };
                        lowered_args.push(lowered);
                    }
                    // Return type is signature.ret_type
                    let ret_ty = sig.ret_type.clone();
                    let fun_expr = hir::Expr { kind: hir::ExprKind::Path(vec![method.clone()]), ty: hir::Ty::Function { param_types: sig.params.iter().map(|p| p.ty.clone()).collect(), ret_type: Box::new(sig.ret_type.clone()), effects: sig.effects.clone() }, span: expr.span, resolution: Some(hir::Resolution::Method { defined_in, span }) };
                    // Prepend receiver as first arg to align with signature
                    let call_args = {
                        let mut v = Vec::with_capacity(1 + lowered_args.len());
                        v.push(recv_hir);
                        v.extend(lowered_args);
                        v
                    };
                    Ok(hir::Expr { ty: ret_ty, kind: hir::ExprKind::Call { fun: Box::new(fun_expr), args: call_args }, span: expr.span, resolution: None })
                } else {
                    self.errors.push(TypeError { message: format!("Unknown method `{}` for receiver type {}", method, Typechecker::format_ty(&recv_hir.ty)), context: context.clone() });
                    Err(())
                }
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
                                if let Some(mod_path) = alias_map.get(alias) {
                                    // For now, we only support std::fmt and std::io qualified println, resolved via global registry
                                    let qual = format!("{}::{}", alias, name);
                                    if let Some(Symbol::Function { signature, .. }) = self.lookup_symbol(&qual) { signature_opt = Some(signature.clone()); }
                                }
                            }
                        }
                        if !is_module_qualified {
                            if let Some(first) = args.get(0) {
                                if let Ok(lhs_expr) = self.lower_expr(first.clone(), context.clone()) {
                                    if let hir::Ty::Adt(hir::AdtTy::Struct { name: ty_name, .. }) | hir::Ty::Adt(hir::AdtTy::Enum { name: ty_name, .. }) = lhs_expr.ty {
                                        if let Some(methods) = self.impl_methods.get(&ty_name) {
                                            if let Some((sig, is_public, defined_in, span)) = methods.get(&name) {
                                                if !*is_public && defined_in != &context.path {
                                                    self.errors.push(TypeError { message: format!("Method `{}` is private", name), context: context.clone() });
                                                    return Err(());
                                                }
                                                signature_opt = Some(sig.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if signature_opt.is_none() {
                            signature_opt = match self.lookup_symbol(&name) {
                                Some(Symbol::Function { signature, is_public, defined_in, decl_span: _ }) => {
                                    if !is_public && *defined_in != context.path {
                                        self.errors.push(TypeError { message: format!("Function `{}` is private", name), context: context.clone() });
                                        None
                                    } else {
                                        Some(signature.clone())
                                    }
                                }
                                // try module-qualified name like fmt::println
                                None if path.len() == 2 => {
                                    let qual = format!("{}::{}", path[0], path[1]);
                                    match self.lookup_symbol(&qual) { Some(Symbol::Function { signature, .. }) => Some(signature.clone()), _ => None }
                                }
                                _ => None,
                            };
                        }

                        if let Some(signature) = signature_opt {
                            let mut lowered_args = Vec::new();
                            for (idx, arg) in adjusted_args.into_iter().enumerate() {
                                let arg_expr = if let Some(p) = signature.params.get(idx) {
                                    self.lower_expr_with_expected(arg, p.ty.clone(), context.clone())?
                                } else {
                                    self.lower_expr(arg, context.clone())?
                                };
                                lowered_args.push(arg_expr);
                            }
                            if lowered_args.len() != signature.params.len() {
                                self.errors.push(TypeError { message: format!(
                                    "Function `{}` expects {} args, found {}", signature.name, signature.params.len(), lowered_args.len()
                                ), context: context.clone() });
                            }
                            // Attach semantic resolution for top-level function or method calls
                            let mut fun_resolution: Option<hir::Resolution> = None;
                            if is_module_qualified {
                                if let Some(Symbol::Function { signature: sig, is_public, defined_in, decl_span }) = self.lookup_symbol(&name).cloned() {
                                    if is_public { if let Some(sp) = decl_span { fun_resolution = Some(hir::Resolution::Function { defined_in, span: sp }); } }
                                }
                            } else if let Some(first) = args.get(0) {
                                if let Ok(lhs_expr) = self.lower_expr(first.clone(), context.clone()) {
                                    if let hir::Ty::Adt(hir::AdtTy::Struct { name: ty_name, .. }) | hir::Ty::Adt(hir::AdtTy::Enum { name: ty_name, .. }) = lhs_expr.ty {
                                        if let Some(methods) = self.impl_methods.get(&ty_name) {
                                            if let Some((_sig, is_public, defined_in, span)) = methods.get(&name) {
                                                if *is_public { fun_resolution = Some(hir::Resolution::Method { defined_in: defined_in.clone(), span: *span }); }
                                            }
                                        }
                                    }
                                }
                            }
                            let mut fun_expr = hir::Expr { kind: hir::ExprKind::Path(path), ty: hir::Ty::Function { param_types: signature.params.iter().map(|p| p.ty.clone()).collect(), ret_type: Box::new(signature.ret_type.clone()), effects: signature.effects.clone() }, span: fun.span, resolution: None };
                            if let Some(res) = fun_resolution.clone() { fun_expr.resolution = Some(res); }
                            return Ok(hir::Expr { ty: signature.ret_type.clone(), kind: hir::ExprKind::Call { fun: Box::new(fun_expr), args: lowered_args }, span: expr.span, resolution: fun_resolution });
                        }

                        if let Some((union_path, payload_types)) = self.find_union_variant(&name) {
                            let ret_ty = hir::Ty::Adt(hir::AdtTy::Enum { name: union_path.clone(), generics: vec![] });
                            let expected_payload_ty = payload_types.as_ref().and_then(|v| v.get(0)).cloned();
                            let mut lowered_args = Vec::new();
                            if let Some(exp_ty) = expected_payload_ty.clone() {
                                if let Some(first) = adjusted_args.get(0) {
                                    let lowered = self.lower_expr_with_expected(first.clone(), exp_ty.clone(), context.clone())?;
                                    lowered_args.push(lowered);
                                } else {
                                    self.errors.push(TypeError { message: format!("Variant `{}` expects 1 argument", name), context: context.clone() });
                                }
                            }
                            let fun_expr = hir::Expr { kind: hir::ExprKind::Path(vec![name]), ty: hir::Ty::Function { param_types: expected_payload_ty.map(|t| vec![t]).unwrap_or_default(), ret_type: Box::new(ret_ty.clone()), effects: vec![] }, span: fun.span, resolution: None };
                            return Ok(hir::Expr { ty: ret_ty, kind: hir::ExprKind::Call { fun: Box::new(fun_expr), args: lowered_args }, span: expr.span, resolution: None });
                        }

                        self.errors.push(TypeError { message: format!("Unknown function or constructor `{}`", name), context: context.clone() });
                        Err(())
                    }
                    _ => {
                        let fun_hir = self.lower_expr(*fun, context.clone())?;
                        let params = match &fun_hir.ty {
                            hir::Ty::Function { param_types, .. } => param_types.clone(),
                            other => {
                                self.errors.push(TypeError { message: format!("Attempted to call a non-function value of type {}", Typechecker::format_ty(&other)), context: context.clone() });
                                vec![]
                            }
                        };
                        let mut lowered_args = Vec::new();
                        for (idx, arg) in args.into_iter().enumerate() {
                            let arg_expr = if let Some(expected) = params.get(idx) { self.lower_expr_with_expected(arg, expected.clone(), context.clone())? } else { self.lower_expr(arg, context.clone())? };
                            lowered_args.push(arg_expr);
                        }
                        let ret_ty = match &fun_hir.ty { hir::Ty::Function { ret_type, .. } => (*ret_type.clone()).clone(), _ => hir::Ty::Special(hir::SpecialTy::Unit) };
                        Ok(hir::Expr { ty: ret_ty, kind: hir::ExprKind::Call { fun: Box::new(fun_hir), args: lowered_args }, span: expr.span, resolution: None })
                    }
                }
            }
            OwnedExpr::Block { stmts, last_expr } => {
                self.enter_scope();
                let mut hir_stmts: Vec<hir::Stmt> = Vec::new();
                for s in stmts.into_iter() { if let Ok(stmt) = self.lower_stmt(s, context.clone()) { hir_stmts.push(stmt); } }
                let mut inferred_last_expr: Option<Box<hir::Expr>> = None;
                if last_expr.is_none() {
                    if let Some(hir::Stmt::Expr { expr: e, .. }) = hir_stmts.last() { inferred_last_expr = Some(Box::new(e.clone())); }
                    if inferred_last_expr.is_some() { hir_stmts.pop(); }
                }
                let (hir_last_expr, block_ty) = match (last_expr, inferred_last_expr) {
                    (Some(expr), _) => { let hir_expr = self.lower_expr(*expr, context.clone())?; let ty = hir_expr.ty.clone(); (Some(Box::new(hir_expr)), ty) }
                    (None, Some(inf)) => { let ty = inf.ty.clone(); (Some(inf), ty) }
                    (None, None) => (None, hir::Ty::Special(hir::SpecialTy::Unit)),
                };
                self.leave_scope();
                let block = hir::HirBlock { stmts: hir_stmts, last_expr: hir_last_expr, ty: block_ty.clone() };
                Ok(hir::Expr { kind: hir::ExprKind::Block(block), ty: block_ty, span: expr.span, resolution: None })
            }
            OwnedExpr::If { cond, then_block, else_block } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.errors.push(TypeError { message: "If condition must be a bool".to_string(), context: context.clone() });
                }
                let then_hir = self.lower_expr(*then_block, context.clone())?;
                let else_hir_opt = if let Some(e) = else_block { Some(Box::new(self.lower_expr(*e, context.clone())?)) } else { None };
                let then_block = match then_hir.kind.clone() { hir::ExprKind::Block(b) => b, _ => hir::HirBlock { stmts: vec![], last_expr: Some(Box::new(then_hir.clone())), ty: then_hir.ty.clone() } };
                let result_ty = if let Some(ref else_hir) = else_hir_opt {
                    if then_hir.ty != else_hir.ty {
                        self.errors.push(TypeError { message: format!("If branches must have same type: then={}, else={}", Typechecker::format_ty(&then_hir.ty), Typechecker::format_ty(&else_hir.ty)), context: context.clone() });
                    }
                    else_hir.ty.clone()
                } else { hir::Ty::Special(hir::SpecialTy::Unit) };
                Ok(hir::Expr { ty: result_ty, kind: hir::ExprKind::If { cond: Box::new(cond_hir), then_block, else_block: else_hir_opt }, span: expr.span, resolution: None })
            }
            OwnedExpr::While { cond, body } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.errors.push(TypeError { message: "While condition must be a bool".to_string(), context: context.clone() });
                }
                let body_hir = self.lower_expr(*body, context.clone())?;
                let body_block = match body_hir.kind { hir::ExprKind::Block(b) => b, _ => hir::HirBlock { stmts: vec![], last_expr: Some(Box::new(body_hir.clone())), ty: body_hir.ty.clone() } };
                Ok(hir::Expr { ty: hir::Ty::Special(hir::SpecialTy::Unit), kind: hir::ExprKind::While { cond: Box::new(cond_hir), body: body_block }, span: expr.span, resolution: None })
            }
            OwnedExpr::Match { scrutinee, arms } => {
                let scrutinee_hir = self.lower_expr(*scrutinee, context.clone())?;
                let mut lowered_arms = Vec::new();
                let mut result_ty: Option<hir::Ty> = None;
                for (pat, arm_expr) in arms {
                    self.enter_scope();
                    let (hir_pat, hir_arm_expr) = self.lower_match_arm(pat, arm_expr, &scrutinee_hir.ty, context.clone())?;
                    if let Some(ref ty) = result_ty { if *ty != hir_arm_expr.ty { self.errors.push(TypeError { message: format!("Match arms must have the same type: expected {}, found {}", Typechecker::format_ty(ty), Typechecker::format_ty(&hir_arm_expr.ty)), context: context.clone() }); } } else { result_ty = Some(hir_arm_expr.ty.clone()); }
                    lowered_arms.push((hir_pat, hir_arm_expr));
                    self.leave_scope();
                }
                let result_ty = result_ty.unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                Ok(hir::Expr { ty: result_ty, kind: hir::ExprKind::Match { scrutinee: Box::new(scrutinee_hir), arms: lowered_arms }, span: expr.span, resolution: None })
            }
            OwnedExpr::Perform { path, args } => {
                let (ret_ty, _param_tys) = self.resolve_effect_op(&path).unwrap_or((hir::Ty::Special(hir::SpecialTy::Unit), vec![]));
                let mut lowered_args = Vec::new();
                for a in args { match self.lower_expr(a.clone(), context.clone()) { Ok(e) => lowered_args.push(e), Err(_) => lowered_args.push(hir::Expr { kind: hir::ExprKind::Error, ty: hir::Ty::Generic("_unknown".to_string()), span: a.span, resolution: None }), } }
                Ok(hir::Expr { ty: ret_ty, kind: hir::ExprKind::Perform { path, args: lowered_args }, span: expr.span, resolution: None })
            }
            OwnedExpr::Handle { body, handler } => {
                let body_hir = self.lower_expr(*body, context.clone())?;
                let handler_hir = match handler {
                    OwnedHandlerBody::Path(p) => hir::HirHandlerBody::Path(p),
                    OwnedHandlerBody::Inline(funcs) => {
                        let mut lowered = Vec::new();
                        for f in funcs { if let Ok(h) = self.lower_function(f, ItemContext { span: expr.span, path: context.path.clone() }) { lowered.push(h); } }
                        hir::HirHandlerBody::Inline(lowered)
                    }
                };
                Ok(hir::Expr { ty: body_hir.ty.clone(), kind: hir::ExprKind::Handle { body: match body_hir.kind.clone() { hir::ExprKind::Block(b) => b, _ => hir::HirBlock { stmts: vec![], last_expr: Some(Box::new(body_hir.clone())), ty: body_hir.ty.clone() } }, handler: handler_hir }, span: expr.span, resolution: None })
            }
            OwnedExpr::Cast { expr: inner, ty } => {
                let inner_hir = self.lower_expr(*inner, context.clone())?;
                let target_ty = self.resolve_type(&ty, context.clone())?;
                Ok(hir::Expr { ty: target_ty.clone(), kind: hir::ExprKind::Cast { expr: Box::new(inner_hir) }, span: expr.span, resolution: None })
            }
            OwnedExpr::StructInit { path, fields, .. } => {
                let adt_ty = hir::Ty::Adt(hir::AdtTy::Struct { name: path.clone(), generics: vec![] });
                let mut lowered_fields = Vec::new();
                for (name, expr) in fields { let e = self.lower_expr(expr, context.clone())?; lowered_fields.push((name, e)); }
                Ok(hir::Expr { ty: adt_ty.clone(), kind: hir::ExprKind::StructInit { path, fields: lowered_fields }, span: expr.span, resolution: None })
            }
            OwnedExpr::Array(_) | OwnedExpr::Map(_) => {
                self.errors.push(TypeError { message: "Cannot infer type for map/record literal without annotation".to_string(), context: context.clone() });
                Err(())
            }
            OwnedExpr::Error => Ok(hir::Expr { kind: hir::ExprKind::Error, ty: hir::Ty::Special(hir::SpecialTy::Unit), span: expr.span, resolution: None }),
        }
    }

    pub(crate) fn lower_expr_with_expected(&mut self, expr: SpannedExpr, expected: hir::Ty, context: ItemContext) -> Result<hir::Expr, ()> {
        match (expr.item.clone(), expected.clone()) {
            (OwnedExpr::Literal(lit), hir::Ty::Primitive(exp_prim)) => {
                if let Some((pty, s)) = self.coerce_numeric_literal(&lit, exp_prim.clone()) {
                    return Ok(hir::Expr { ty: hir::Ty::Primitive(pty.clone()), kind: hir::ExprKind::Literal(pty, s), span: expr.span, resolution: None });
                }
                self.lower_expr(Spanned { item: OwnedExpr::Literal(lit), span: expr.span }, context)
            }
            (OwnedExpr::Block { mut stmts, last_expr }, expected_ty) => {
                self.enter_scope();
                let mut hir_stmts: Vec<hir::Stmt> = Vec::new();
                let mut trailing_expr_opt: Option<SpannedExpr> = None;
                if last_expr.is_none() {
                    if let Some(last) = stmts.last() { if let OwnedStmt::Expr(e) = &last.item { trailing_expr_opt = Some(e.clone()); stmts.pop(); } }
                }
                for s in stmts.into_iter() { if let Ok(stmt) = self.lower_stmt(s, context.clone()) { hir_stmts.push(stmt); } }
                let (hir_last_expr, block_ty) = if let Some(expr) = last_expr { let lowered = self.lower_expr_with_expected(*expr, expected_ty.clone(), context.clone())?; (Some(Box::new(lowered.clone())), lowered.ty) } else if let Some(expr) = trailing_expr_opt { let lowered = self.lower_expr_with_expected(expr, expected_ty.clone(), context.clone())?; (Some(Box::new(lowered.clone())), lowered.ty) } else { (None, hir::Ty::Special(hir::SpecialTy::Unit)) };
                self.leave_scope();
                Ok(hir::Expr { ty: block_ty.clone(), kind: hir::ExprKind::Block(hir::HirBlock { stmts: hir_stmts, last_expr: hir_last_expr, ty: block_ty }), span: expr.span, resolution: None })
            }
            _ => self.lower_expr(expr, context),
        }
    }
}


