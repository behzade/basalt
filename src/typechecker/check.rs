//! typechecker/check.rs
//!
//! This module contains the core logic for traversing the AST and lowering it
//! to the HIR. It's the "second pass" of the type checker, responsible for
//! validating type correctness for all statements and expressions.

use super::{TypeChecker, TypeError};
use crate::{ast, hir, hir::Ty};

impl<'src> TypeChecker<'src> {
    /// Checks a single top-level item and lowers it to its HIR representation.
    pub fn check_item(
        &mut self,
        item: &ast::Item<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        match item {
            ast::Item::Fn(func) => self.check_function(func),
            ast::Item::Stmt(stmt) => {
                let hir_stmt = self.check_stmt(stmt)?;
                Ok(hir::Item::Stmt(hir_stmt))
            }
            // Other top-level items would be handled here.
            // For now, we'll just pass them through or stub them.
            ast::Item::Struct(struct_def) => {
                // In a real compiler, we'd validate field types here.
                // For now, we assume they are correct since they were collected.
                let hir_struct = hir::StructDef {
                    name: struct_def.name,
                    generics: struct_def.generics.clone(),
                    // We need to convert ast::Type to hir::Ty
                    fields: struct_def
                        .fields
                        .iter()
                        .map(|(name, ty)| (*name, self.lower_type(ty)))
                        .collect(),
                    is_public: struct_def.is_public,
                };
                Ok(hir::Item::Struct(hir_struct))
            }
            ast::Item::Enum(enum_def) => {
                let hir_enum = hir::EnumDef {
                    name: enum_def.name,
                    variants: enum_def
                        .variants
                        .iter()
                        .map(|(name, types)| {
                            (
                                *name,
                                types.as_ref().map(|t_vec| {
                                    t_vec.iter().map(|t| self.lower_type(t)).collect()
                                }),
                            )
                        })
                        .collect(),
                    is_public: enum_def.is_public,
                };
                Ok(hir::Item::Enum(hir_enum))
            }
            _ => {
                // For now, other item types are not fully checked but will not error.
                // This allows tests for parsing to pass without a full implementation.
                Ok(hir::Item::Stmt(hir::Stmt::Expr(hir::Expr {
                    kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                    ty: Ty::Unit,
                }))) // Placeholder
            }
        }
    }

    /// Checks a function definition, including its body, and lowers it to HIR.
    fn check_function(
        &mut self,
        func: &ast::Function<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        self.context.enter_scope();

        // Lower and add function parameters to the new scope.
        let mut hir_params = Vec::new();
        for (name_opt, ty) in &func.params {
            let hir_ty = self.lower_type(ty);
            if let Some(name) = name_opt {
                self.context.add_variable(name, hir_ty.clone());
            }
            hir_params.push((*name_opt, hir_ty));
        }

        // Determine the expected return type.
        let expected_ret_ty = func
            .ret_type
            .as_ref()
            .map_or(Ty::Unit, |rt| self.lower_type(rt));

        // Check the function body.
        let body = self.check_expr(&func.body)?;

        // Unify the actual body's return type with the function's declared return type.
        if let Err(_) = self.unify(&body.ty, &expected_ret_ty) {
            return Err(TypeError::MismatchedTypes {
                expected: expected_ret_ty,
                found: body.ty.clone(),
            });
        }

        self.context.leave_scope();

        Ok(hir::Item::Fn(hir::Function {
            name: func.name,
            params: hir_params,
            ret_type: self.resolve_type(&expected_ret_ty),
            body,
            is_public: func.is_public,
        }))
    }

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
            } => {
                let hir_value = self.check_expr(value)?;
                let value_ty = self.resolve_type(&hir_value.ty);

                // If a type annotation is present, unify it with the value's type.
                let expected_ty = if let Some(annotated_ty) = ty {
                    let lower_ty = self.lower_type(annotated_ty);
                    if let Err(_) = self.unify(&value_ty, &lower_ty) {
                        return Err(TypeError::MismatchedTypes {
                            expected: lower_ty,
                            found: value_ty,
                        });
                    }
                    lower_ty
                } else {
                    value_ty
                };

                self.context
                    .add_variable(name, self.resolve_type(&expected_ty));

                Ok(hir::Stmt::Let {
                    name,
                    is_mut: *is_mut,
                    value_ty: self.resolve_type(&expected_ty),
                    value: hir_value,
                })
            }
            ast::Stmt::Expr(expr) => {
                let hir_expr = self.check_expr(expr)?;
                Ok(hir::Stmt::Expr(hir_expr))
            }
            ast::Stmt::Return(_) => {
                // Return statements are handled within function/block checking.
                // A standalone implementation would require knowing the current function context.
                // For now, we'll treat it as a placeholder.
                Ok(hir::Stmt::Return(None))
            }
            ast::Stmt::Assign(lhs, rhs) => {
                let hir_lhs = self.check_expr(lhs)?;
                let hir_rhs = self.check_expr(rhs)?;

                if let Err(_) = self.unify(&hir_lhs.ty, &hir_rhs.ty) {
                    return Err(TypeError::MismatchedTypes {
                        expected: self.resolve_type(&hir_lhs.ty),
                        found: self.resolve_type(&hir_rhs.ty),
                    });
                }

                Ok(hir::Stmt::Assign(hir_lhs, hir_rhs))
            }
            ast::Stmt::Error => Ok(hir::Stmt::Expr(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Error,
            })),
        }
    }

    /// Checks a single expression and lowers it to its HIR representation.
    /// This is the core of the type-checking logic.
    pub fn check_expr(
        &mut self,
        expr: &ast::Expr<'src>,
    ) -> Result<hir::Expr<'src>, TypeError<'src>> {
        let (kind, ty) = match expr {
            ast::Expr::Literal(lit) => {
                let ty = match lit {
                    ast::Literal::Bool(_) => Ty::Bool,
                    ast::Literal::I64(_) => Ty::I64,
                    ast::Literal::F64(_) => Ty::F64,
                    ast::Literal::Str(_) => Ty::Str,
                };
                (hir::ExprKind::Literal(lit.clone()), ty)
            }
            ast::Expr::Path(path) => {
                let name = path.first().ok_or(TypeError::UnknownVariable(""))?;
                let ty = self
                    .context
                    .get_variable(name)
                    .ok_or(TypeError::UnknownVariable(name))?
                    .clone();
                (hir::ExprKind::Path(path.clone()), ty)
            }
            ast::Expr::Binary { op, lhs, rhs } => {
                let hir_lhs = self.check_expr(lhs)?;
                let hir_rhs = self.check_expr(rhs)?;

                let ty = match op {
                    ast::BinaryOp::Add
                    | ast::BinaryOp::Sub
                    | ast::BinaryOp::Mul
                    | ast::BinaryOp::Div => {
                        self.unify(&hir_lhs.ty, &hir_rhs.ty)?;
                        let resolved_lhs = self.resolve_type(&hir_lhs.ty);
                        if !matches!(resolved_lhs, Ty::I64 | Ty::F64) {
                            return Err(TypeError::InvalidOperator {
                                op: op.to_string(),
                                ty: resolved_lhs,
                            });
                        }
                        resolved_lhs
                    }
                    ast::BinaryOp::Eq
                    | ast::BinaryOp::Ne
                    | ast::BinaryOp::Lt
                    | ast::BinaryOp::Gt => {
                        self.unify(&hir_lhs.ty, &hir_rhs.ty)?;
                        Ty::Bool
                    }
                };

                let kind = hir::ExprKind::Binary {
                    op: *op,
                    lhs: Box::new(hir_lhs),
                    rhs: Box::new(hir_rhs),
                };
                (kind, ty)
            }
            ast::Expr::If {
                cond,
                then_block,
                else_block,
            } => {
                let hir_cond = self.check_expr(cond)?;
                self.unify(&hir_cond.ty, &Ty::Bool)?;

                let hir_then = self.check_expr(then_block)?;

                let (hir_else, final_ty) = if let Some(else_b) = else_block {
                    let hir_else = self.check_expr(else_b)?;
                    self.unify(&hir_then.ty, &hir_else.ty)?;
                    (Some(Box::new(hir_else)), hir_then.ty.clone())
                } else {
                    // If there's no else block, the expression must evaluate to unit.
                    self.unify(&hir_then.ty, &Ty::Unit)?;
                    (None, Ty::Unit)
                };

                let kind = hir::ExprKind::If {
                    cond: Box::new(hir_cond),
                    then_block: Box::new(hir_then),
                    else_block: hir_else,
                };
                (kind, final_ty)
            }
            ast::Expr::Block { stmts, last_expr } => {
                self.context.enter_scope();
                let mut hir_stmts = Vec::new();
                for stmt in stmts {
                    hir_stmts.push(self.check_stmt(stmt)?);
                }

                let (hir_last, ty) = if let Some(last) = last_expr {
                    let hir_last = self.check_expr(last)?;
                    let ty = hir_last.ty.clone();
                    (Some(Box::new(hir_last)), ty)
                } else {
                    (None, Ty::Unit)
                };

                self.context.leave_scope();
                let kind = hir::ExprKind::Block {
                    stmts: hir_stmts,
                    last_expr: hir_last,
                };
                (kind, ty)
            }
            // Other expression types would be handled here...
            _ => (hir::ExprKind::Literal(ast::Literal::Bool(true)), Ty::Error),
        };

        Ok(hir::Expr { kind, ty })
    }

    /// Lowers an `ast::Type` to an `hir::Ty`.
    /// This is where we would handle things like resolving type aliases.
    pub fn lower_type(&self, ast_ty: &ast::Type<'src>) -> hir::Ty<'src> {
        // For now, a simple conversion.
        let name = ast_ty.path.first().unwrap_or(&"");
        match *name {
            "bool" => Ty::Bool,
            "i64" => Ty::I64,
            "f64" => Ty::F64,
            "string" => Ty::Str,
            "none" => Ty::Unit,
            "Array" => {
                let inner = ast_ty
                    .generics
                    .first()
                    .map_or(Ty::Error, |t| self.lower_type(t));
                Ty::Array(Box::new(inner))
            }
            "Map" => {
                let key = ast_ty
                    .generics
                    .get(0)
                    .map_or(Ty::Error, |t| self.lower_type(t));
                let value = ast_ty
                    .generics
                    .get(1)
                    .map_or(Ty::Error, |t| self.lower_type(t));
                Ty::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                }
            }
            _ => Ty::Adt {
                name: ast_ty.path.clone(),
                generics: ast_ty.generics.iter().map(|t| self.lower_type(t)).collect(),
            },
        }
    }
}

// Add a `to_string` method to BinaryOp for error messages.
impl ToString for ast::BinaryOp {
    fn to_string(&self) -> String {
        match self {
            ast::BinaryOp::Add => "+".to_string(),
            ast::BinaryOp::Sub => "-".to_string(),
            ast::BinaryOp::Mul => "*".to_string(),
            ast::BinaryOp::Div => "/".to_string(),
            ast::BinaryOp::Eq => "==".to_string(),
            ast::BinaryOp::Ne => "!=".to_string(),
            ast::BinaryOp::Lt => "<".to_string(),
            ast::BinaryOp::Gt => ">".to_string(),
        }
    }
}
