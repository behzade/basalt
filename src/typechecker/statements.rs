//! typechecker/statements.rs
//!
//! This module contains the logic for checking and lowering statements
//! from AST to HIR.

use super::{TypeChecker, TypeError};
use crate::{ast, hir, hir::Ty};

impl<'src> TypeChecker<'src> {
    /// Checks a single statement and lowers it to HIR.
    pub fn check_stmt(
        &mut self,
        stmt: &ast::Stmt<'src>,
    ) -> Result<hir::Stmt<'src>, TypeError<'src>> {
        match stmt {
            ast::Stmt::Let {
                is_mut,
                name,
                ty,
                value,
            } => self.check_let_stmt(is_mut, name, ty, value),
            ast::Stmt::Expr(expr) => {
                let hir_expr = self.check_expr(expr)?;
                Ok(hir::Stmt::Expr(hir_expr))
            }
            ast::Stmt::Return(expr) => self.check_return_stmt(expr),
            ast::Stmt::Assign(lhs, rhs) => self.check_assign_stmt(lhs, rhs),
            ast::Stmt::Error => Ok(hir::Stmt::Expr(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Error,
            })),
        }
    }

    fn check_let_stmt(
        &mut self,
        is_mut: &bool,
        name: &'src str,
        ty: &Option<ast::Type<'src>>,
        value: &ast::Expr<'src>,
    ) -> Result<hir::Stmt<'src>, TypeError<'src>> {
        let expected_ty = ty.as_ref().map(|t| self.lower_type(t));
        let infer_ty = self.new_infer_ty();
        let hint_ty = expected_ty.as_ref().unwrap_or(&infer_ty);
        let hir_value = self.check_expr_with_hint(value, hint_ty)?;
        let value_ty = self.resolve_type(&hir_value.ty);

        let final_ty = if let Some(annotated_ty) = &expected_ty {
            if self.unify(&value_ty, annotated_ty).is_err() {
                return Err(TypeError::MismatchedTypes {
                    expected: self.resolve_type(annotated_ty),
                    found: value_ty,
                });
            }
            annotated_ty.clone()
        } else {
            value_ty
        };

        self.context.add_variable(name, final_ty.clone());
        Ok(hir::Stmt::Let {
            is_mut: *is_mut,
            name,
            value_ty: final_ty,
            value: hir_value,
        })
    }

    fn check_return_stmt(
        &mut self,
        expr: &Option<ast::Expr<'src>>,
    ) -> Result<hir::Stmt<'src>, TypeError<'src>> {
        // The actual check happens inside check_expr_with_hint for a block.
        // Here we just lower the expression inside the return.
        let hir_expr = if let Some(e) = expr {
            Some(self.check_expr(e)?)
        } else {
            None
        };
        Ok(hir::Stmt::Return(hir_expr))
    }

    fn check_assign_stmt(
        &mut self,
        lhs: &ast::Expr<'src>,
        rhs: &ast::Expr<'src>,
    ) -> Result<hir::Stmt<'src>, TypeError<'src>> {
        let hir_lhs = self.check_expr(lhs)?;
        let hir_rhs = self.check_expr(rhs)?;

        if self.unify(&hir_lhs.ty, &hir_rhs.ty).is_err() {
            return Err(TypeError::MismatchedTypes {
                expected: self.resolve_type(&hir_lhs.ty),
                found: self.resolve_type(&hir_rhs.ty),
            });
        }

        Ok(hir::Stmt::Assign(hir_lhs, hir_rhs))
    }
}

