use crate::ast_owned::*;
use crate::hir;
use crate::type_unifier::TypeUnifier;
use crate::typechecker::checker::Typechecker;
use crate::typechecker::errors::{ItemContext, TypeError};
use crate::typechecker::symbols::Symbol;
use crate::hir::{HirSymbolDecl, HirSymbolKind};

impl Typechecker {
    pub(crate) fn lower_stmt(&mut self, stmt: SpannedStmt, context: ItemContext) -> Result<hir::Stmt, ()> {
        match stmt.item {
            OwnedStmt::Let { is_mut, name, ty, value } => {
                let (hir_value_opt, var_ty) = if let Some(annotated_ty) = ty.clone() {
                    let resolved_ty = self.resolve_type(&annotated_ty, context.clone())?;
                    match (value.as_ref().map(|v| &v.item), &resolved_ty) {
                        // Disallow anonymous struct map sugar; require explicit Person { ... }
                        (Some(vexpr), _) => {
                            let mut lowered = self.lower_expr(Spanned { item: vexpr.clone(), span: stmt.span }, context.clone())?;
                            if lowered.ty != resolved_ty {
                                if TypeUnifier::is_numeric(&lowered.ty) && TypeUnifier::is_numeric(&resolved_ty) {
                                    lowered = hir::Expr { ty: resolved_ty.clone(), kind: hir::ExprKind::Cast { expr: Box::new(lowered) }, span: stmt.span, resolution: None };
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

                // Prevent redeclaration in the same scope
                if let Some(existing) = self.lookup_symbol(&name).cloned() {
                    // Only block if existing is from the same innermost scope: detect by peeking last scope
                    if let Some(last) = self.scopes.last() {
                        if last.contains_key(&name) {
                            self.errors.push(TypeError { message: format!("Variable '{}' already defined in this scope", name), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                        }
                    }
                }
                let symbol = Symbol::Variable { ty: var_ty.clone(), is_mut, initialized: hir_value_opt.is_some(), decl_span: Some(stmt.span) };
                self.add_symbol_to_current_scope(name.clone(), symbol);

                // Persist local variable into the current function/block context if any
                if let Some(cid) = self.current_context() {
                    self.add_symbol_to_context(cid, HirSymbolDecl {
                        name: name.clone(),
                        kind: HirSymbolKind::Variable,
                        ty: Some(var_ty.clone()),
                        is_mut: Some(is_mut),
                        span: stmt.span,
                        name_span: None,
                    });
                }

                Ok(hir::Stmt::Let { name, value: hir_value_opt, ty: var_ty, is_mut, span: stmt.span, name_span: None })
            }
            OwnedStmt::Assign(lhs, rhs) => {
                let lhs_hir = match &lhs.item {
                    OwnedExpr::Path(path) => {
                        let name = match path.last() { Some(n) => n.clone(), None => "".to_string() };
                        if let Some(Symbol::Variable { ty, .. }) = self.lookup_symbol(&name).cloned() { hir::Expr { kind: hir::ExprKind::Path(path.clone()), ty, span: lhs.span, resolution: Some(hir::Resolution::Local { name: name.clone(), decl_span: None }) } } else { self.lower_expr(lhs.clone(), context.clone())? }
                    }
                    _ => self.lower_expr(lhs.clone(), context.clone())?,
                };
                let rhs_hir = self.lower_expr(rhs, context.clone())?;
                if lhs_hir.ty != rhs_hir.ty {
                    self.errors.push(TypeError { message: format!("Assignment type mismatch: lhs={}, rhs={}", Typechecker::format_ty(&lhs_hir.ty), Typechecker::format_ty(&rhs_hir.ty)), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                }
                if let hir::ExprKind::Path(p) = &lhs_hir.kind { if let Some(var_name) = p.last() { self.mark_variable_initialized(var_name); } }
                Ok(hir::Stmt::Assign { lhs: lhs_hir, rhs: rhs_hir, span: stmt.span })
            }
            OwnedStmt::Return(expr_opt) => {
                let expr_hir_opt = if let Some(e) = expr_opt { Some(self.lower_expr(e, context.clone())?) } else { None };
                if let Some(expected) = &self.current_fn_return_type {
                    let actual = expr_hir_opt.as_ref().map(|e| e.ty.clone()).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    if &actual != expected {
                        self.errors.push(TypeError { message: format!("Return type mismatch: expected {}, found {}", Typechecker::format_ty(expected), Typechecker::format_ty(&actual)), context: ItemContext { span: stmt.span, path: context.path.clone() } });
                    }
                }
                Ok(hir::Stmt::Return { value: expr_hir_opt, span: stmt.span })
            }
            OwnedStmt::Expr(e) => { let e_hir = self.lower_expr(e, context.clone())?; Ok(hir::Stmt::Expr { expr: e_hir, span: stmt.span }) }
            OwnedStmt::Error => Ok(hir::Stmt::Error { span: stmt.span }),
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
        for (name, ty) in &bound_types { self.add_symbol_to_current_scope(name.clone(), Symbol::Variable { ty: ty.clone(), is_mut: false, initialized: true, decl_span: None }); }
        let arm_expr = self.lower_expr(expr, context)?;
        Ok((hir_pat, arm_expr))
    }
}


