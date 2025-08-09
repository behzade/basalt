use crate::ast_owned::*;
use crate::hir;
use crate::type_unifier::TypeUnifier;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;

impl Typechecker {
    pub(crate) fn lower_stmt(&mut self, stmt: SpannedStmt, context: ItemContext) -> Result<hir::Stmt, ()> {
        match stmt.item {
            OwnedStmt::Let { is_mut, name, ty, value } => {
                let (hir_value_opt, var_ty) = if let Some(annotated_ty) = ty.clone() {
                    let resolved_ty = self.resolve_type(&annotated_ty, context.clone())?;
                    match (value.as_ref().map(|v| &v.item), &resolved_ty) {
                        (Some(OwnedExpr::Map(entries)), hir::Ty::Adt(hir::AdtTy::Struct { name: struct_path, .. })) => {
                            let mut lowered_fields = Vec::new();
                            for (k_expr, v_expr) in entries.clone() {
                                let key = match k_expr.item.clone() { OwnedExpr::Literal(OwnedLiteral::Str(s)) => s, _ => "<key>".to_string() };
                                let v_expected_ty = self.lookup_struct_field_type(&struct_path, &key).cloned();
                                let v = if let Some(exp_ty) = v_expected_ty { self.lower_expr_with_expected(v_expr, exp_ty, context.clone())? } else { self.lower_expr(v_expr, context.clone())? };
                                lowered_fields.push((key, v));
                            }
                            let init_expr = hir::Expr { ty: resolved_ty.clone(), kind: hir::ExprKind::StructInit { path: struct_path.clone(), fields: lowered_fields } };
                            (Some(init_expr), resolved_ty.clone())
                        }
                        (Some(vexpr), _) => {
                            let mut lowered = self.lower_expr(Spanned { item: vexpr.clone(), span: stmt.span }, context.clone())?;
                            if lowered.ty != resolved_ty {
                                if TypeUnifier::is_numeric(&lowered.ty) && TypeUnifier::is_numeric(&resolved_ty) {
                                    lowered = hir::Expr { ty: resolved_ty.clone(), kind: hir::ExprKind::Cast { expr: Box::new(lowered) } };
                                } else {
                                    self.errors.push(TypeError { message: format!(
                                        "Mismatched types for variable '{}': expected {} but found {}",
                                        name, Typechecker::format_ty(&resolved_ty), Typechecker::format_ty(&lowered.ty)
                                    ), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                                }
                            }
                            (Some(lowered), resolved_ty.clone())
                        }
                        (None, _) => (None, resolved_ty.clone()),
                    }
                } else {
                    match value {
                        Some(v) => { let lowered = self.lower_expr(v, context.clone())?; (Some(lowered.clone()), lowered.ty.clone()) }
                        None => (None, hir::Ty::Special(hir::SpecialTy::Unit)),
                    }
                };

                let symbol = Symbol::Variable { ty: var_ty.clone(), is_mut, initialized: hir_value_opt.is_some() };
                self.add_symbol_to_current_scope(name.clone(), symbol);

                Ok(hir::Stmt::Let { name, value: hir_value_opt, ty: var_ty, is_mut })
            }
            OwnedStmt::Assign(lhs, rhs) => {
                let lhs_hir = match &lhs.item {
                    OwnedExpr::Path(path) => {
                        let name = match path.last() { Some(n) => n.clone(), None => "".to_string() };
                        if let Some(Symbol::Variable { ty, .. }) = self.lookup_symbol(&name).cloned() { hir::Expr { kind: hir::ExprKind::Path(path.clone()), ty } } else { self.lower_expr(lhs.clone(), context.clone())? }
                    }
                    _ => self.lower_expr(lhs.clone(), context.clone())?,
                };
                let rhs_hir = self.lower_expr(rhs, context.clone())?;
                if lhs_hir.ty != rhs_hir.ty {
                    self.errors.push(TypeError { message: format!("Assignment type mismatch: lhs={:?}, rhs={:?}", lhs_hir.ty, rhs_hir.ty), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                }
                if let hir::ExprKind::Path(p) = &lhs_hir.kind { if let Some(var_name) = p.last() { self.mark_variable_initialized(var_name); } }
                Ok(hir::Stmt::Assign(lhs_hir, rhs_hir))
            }
            OwnedStmt::Return(expr_opt) => {
                let expr_hir_opt = if let Some(e) = expr_opt { Some(self.lower_expr(e, context.clone())?) } else { None };
                if let Some(expected) = &self.current_fn_return_type {
                    let actual = expr_hir_opt.as_ref().map(|e| e.ty.clone()).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    if &actual != expected {
                        self.errors.push(TypeError { message: format!("Return type mismatch: expected {:?}, found {:?}", expected, actual), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                    }
                }
                Ok(hir::Stmt::Return(expr_hir_opt))
            }
            OwnedStmt::Expr(e) => { let e_hir = self.lower_expr(e, context.clone())?; Ok(hir::Stmt::Expr(e_hir)) }
            OwnedStmt::Error => Ok(hir::Stmt::Error),
        }
    }

    pub(crate) fn lower_match_arm(&mut self, pat: SpannedPattern, expr: SpannedExpr, scrutinee_ty: &hir::Ty, context: ItemContext) -> Result<(hir::HirPattern, hir::Expr), ()> {
        let (hir_pat, bound_types): (hir::HirPattern, Vec<(String, hir::Ty)>) = match pat.item {
            OwnedPattern::Wildcard => (hir::HirPattern { kind: hir::HirPatternKind::Wildcard, ty: scrutinee_ty.clone() }, vec![]),
            OwnedPattern::Identifier(name) => (hir::HirPattern { kind: hir::HirPatternKind::Identifier(name.clone()), ty: scrutinee_ty.clone() }, vec![(name, scrutinee_ty.clone())]),
            OwnedPattern::Path { path, args } => {
                let variant_name = path.last().cloned().unwrap_or_default();
                let (union_path, payload) = match scrutinee_ty {
                    hir::Ty::Adt(hir::AdtTy::Enum { name, .. }) => {
                        let mut found: Option<(hir::OwnedPath, Option<Vec<hir::Ty>>)> = None;
                        if let Some(vs) = self.union_variants.get(name) { for (vn, pl) in vs { if vn == &variant_name { found = Some((name.clone(), pl.clone())); break; } } }
                        found.unwrap_or((name.clone(), None))
                    }
                    _ => (vec![], None),
                };
                let mut bound = Vec::new();
                let mut subpatterns = Vec::new();
                if let Some(pl) = payload.clone() { if let Some(first) = pl.get(0) { if let Some(arg0) = args.get(0) { match &arg0.item { OwnedPattern::Identifier(n) => { bound.push((n.clone(), first.clone())); subpatterns.push(hir::HirPattern { kind: hir::HirPatternKind::Identifier(n.clone()), ty: first.clone() }); } _ => { subpatterns.push(hir::HirPattern { kind: hir::HirPatternKind::Wildcard, ty: first.clone() }); } } } } }
                (hir::HirPattern { kind: hir::HirPatternKind::Path { path: vec![variant_name], args: subpatterns }, ty: scrutinee_ty.clone() }, bound)
            }
            OwnedPattern::Literal(lit) => { let (pty, s) = self.lower_literal(lit); (hir::HirPattern { kind: hir::HirPatternKind::Literal(pty.clone(), s), ty: hir::Ty::Primitive(pty) }, vec![]) }
        };
        for (name, ty) in &bound_types { self.add_symbol_to_current_scope(name.clone(), Symbol::Variable { ty: ty.clone(), is_mut: false, initialized: true }); }
        let arm_expr = self.lower_expr(expr, context)?;
        Ok((hir_pat, arm_expr))
    }
}


