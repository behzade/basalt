//! typechecker/expressions.rs
//!
//! This module contains the logic for checking and lowering expressions
//! from AST to HIR.

use super::{TypeChecker, TypeError};
use crate::{ast, hir, hir::Ty};
use std::collections::HashMap;

impl<'src> TypeChecker<'src> {
    /// Entry point for checking an expression without a type hint.
    pub fn check_expr(
        &mut self,
        expr: &ast::Expr<'src>,
    ) -> Result<hir::Expr<'src>, TypeError<'src>> {
        let infer_ty = self.new_infer_ty();
        self.check_expr_with_hint(expr, &infer_ty)
    }

    /// Checks an expression, using a `type_hint` to guide inference.
    pub fn check_expr_with_hint(
        &mut self,
        expr: &ast::Expr<'src>,
        type_hint: &Ty<'src>,
    ) -> Result<hir::Expr<'src>, TypeError<'src>> {
        let (kind, ty) = match expr {
            ast::Expr::Literal(lit) => self.check_literal(lit)?,
            ast::Expr::Path(path) => self.check_path(path)?,
            ast::Expr::Binary { op, lhs, rhs } => self.check_binary(op, lhs, rhs)?,
            ast::Expr::If {
                cond,
                then_block,
                else_block,
            } => self.check_if(cond, then_block, else_block, type_hint)?,
            ast::Expr::Block { stmts, last_expr } => {
                self.check_block(stmts, last_expr, type_hint)?
            }
            ast::Expr::Call { fun, args } => self.check_call(fun, args)?,
            ast::Expr::Array(elements) => self.check_array(elements)?,
            ast::Expr::StructInit { path, generics, fields } => {
                self.check_struct_init(path, generics, fields)?
            }
            ast::Expr::Match { scrutinee, arms } => {
                self.check_match(scrutinee, arms, type_hint)?
            }
            ast::Expr::While { cond, body } => {
                self.check_while(cond, body)?
            }
            _ => return Ok(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Error,
            }),
        };

        Ok(hir::Expr { kind, ty })
    }

    fn check_literal(&self, lit: &ast::Literal<'src>) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let ty = match lit {
            ast::Literal::Bool(_) => Ty::Bool,
            ast::Literal::I64(_) => Ty::I64,
            ast::Literal::F64(_) => Ty::F64,
            ast::Literal::Str(_) => Ty::Str,
            ast::Literal::Unit => Ty::Unit,
        };
        Ok((hir::ExprKind::Literal(lit.clone()), ty))
    }

    fn check_path(&mut self, path: &[&'src str]) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        // First, try to resolve the path using import resolution
        let resolved_path = self.resolve_path(path);
        
        // Check for module-qualified paths (e.g., Fmt::println)
        if resolved_path.len() >= 2 {
            // Try to resolve as a module symbol
            if let Some(module_type) = self.resolve_module_symbol(&resolved_path) {
                let ty = self.lower_type(&module_type);
                return Ok((hir::ExprKind::Path(resolved_path), ty));
            }
            
            // If we couldn't resolve it as a module symbol, check if it looks like a module path
            if resolved_path.len() >= 3 {
                let namespace = resolved_path[0];
                let module = resolved_path[1];
                let symbol = resolved_path[2];
                
                // Check if the module exists but the symbol doesn't
                let module_path = format!("{}::{}", namespace, module);
                if self.context.get_module_symbols(&module_path).is_some() {
                    return Err(TypeError::UnknownModuleSymbol {
                        namespace,
                        module,
                        symbol,
                    });
                } else {
                    return Err(TypeError::UnknownModule {
                        namespace,
                        module,
                    });
                }
            }
        }
        
        // Check for enum variant first.
        if resolved_path.len() == 2 {
            if let Some(enum_def) = self.context.get_enum(resolved_path[0]) {
                if enum_def.variants.iter().any(|(v, _)| v == &resolved_path[1]) {
                    // This is an enum variant, not a variable. Its type is the enum itself.
                    let enum_ty = self.lower_type(&ast::Type {
                        path: vec![resolved_path[0]],
                        generics: vec![],
                    });
                    return Ok((hir::ExprKind::Path(resolved_path), enum_ty));
                }
            }
        }

        // Check for functions and extern functions
        let name = resolved_path.first().ok_or(TypeError::UnknownVariable(""))?;
        
        // Check for regular functions
        if let Some(func_def) = self.context.get_function(name) {
            let ret_ty = func_def
                .ret_type
                .as_ref()
                .map_or(Ty::Unit, |t| self.lower_type(t));
            return Ok((
                hir::ExprKind::Path(resolved_path),
                Ty::Function {
                    param_types: func_def.params.iter().map(|(_, t)| self.lower_type(t)).collect(),
                    ret_type: Box::new(ret_ty),
                },
            ));
        }
        
        // Check for extern functions
        if let Some(extern_item) = self.context.get_extern_function(name) {
            if let ast::Item::ExternFn { params, ret_type, .. } = extern_item {
                let ret_ty = self.lower_type(ret_type);
                return Ok((
                    hir::ExprKind::Path(resolved_path),
                    Ty::Function {
                        param_types: params.iter().map(|(_, t)| self.lower_type(t)).collect(),
                        ret_type: Box::new(ret_ty),
                    },
                ));
            }
        }

        // Check if this looks like it should be an import (but only if it's not a variable)
        if self.context.get_variable(name).is_none() {
            if let Some(suggested_import) = self.suggest_import(name) {
                return Err(TypeError::MissingImport {
                    symbol: name,
                    suggested_import: Some(suggested_import),
                });
            }
        }

        // Otherwise, assume it's a variable.
        let ty = self
            .context
            .get_variable(name)
            .ok_or(TypeError::UnknownVariable(name))?
            .clone();
        Ok((hir::ExprKind::Path(resolved_path), ty))
    }

    fn check_binary(
        &mut self,
        op: &ast::BinaryOp,
        lhs: &ast::Expr<'src>,
        rhs: &ast::Expr<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_lhs = self.check_expr(lhs)?;
        let hir_rhs = self.check_expr(rhs)?;

        let ty = match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div => {
                self.unify(&hir_lhs.ty, &hir_rhs.ty)?;
                let resolved_lhs = self.resolve_type(&hir_lhs.ty);
                if !matches!(resolved_lhs, Ty::I64 | Ty::F64 | Ty::Str) {
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
        Ok((kind, ty))
    }

    fn check_if(
        &mut self,
        cond: &ast::Expr<'src>,
        then_block: &ast::Expr<'src>,
        else_block: &Option<Box<ast::Expr<'src>>>,
        type_hint: &Ty<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_cond = self.check_expr(cond)?;
        self.unify(&hir_cond.ty, &Ty::Bool)?;

        let hir_then = self.check_expr_with_hint(then_block, type_hint)?;

        let (hir_else, final_ty) = if let Some(else_b) = else_block {
            let hir_else = self.check_expr_with_hint(else_b, &hir_then.ty)?;
            self.unify(&hir_then.ty, &hir_else.ty)?;
            (Some(Box::new(hir_else)), hir_then.ty.clone())
        } else {
            // If there's no else block, the if statement can return the type of the then block
            // This allows for early returns in the then block
            (None, hir_then.ty.clone())
        };

        let kind = hir::ExprKind::If {
            cond: Box::new(hir_cond),
            then_block: Box::new(hir_then),
            else_block: hir_else,
        };
        Ok((kind, final_ty))
    }

    fn check_block(
        &mut self,
        stmts: &[ast::Stmt<'src>],
        last_expr: &Option<Box<ast::Expr<'src>>>,
        type_hint: &Ty<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        self.context.enter_scope();
        let mut hir_stmts = Vec::new();
        let mut block_ty = Ty::Unit; // Default to unit
        let mut has_return = false;

        for stmt in stmts {
            // Check if this is a return statement
            if let ast::Stmt::Return(Some(expr)) = stmt {
                let hir_expr = self.check_expr_with_hint(expr, type_hint)?;
                self.unify(&hir_expr.ty, type_hint)?;
                block_ty = hir_expr.ty.clone();
                hir_stmts.push(hir::Stmt::Return(Some(hir_expr)));
                has_return = true;
                // Don't break here - continue processing to find the last reachable return
            } else {
                hir_stmts.push(self.check_stmt(stmt)?);
            }
        }

        let (hir_last, final_ty) = if has_return {
            (None, block_ty)
        } else if let Some(last) = last_expr {
            let hir_last = self.check_expr_with_hint(last, type_hint)?;
            (Some(Box::new(hir_last.clone())), hir_last.ty)
        } else {
            (None, Ty::Unit)
        };

        self.context.leave_scope();
        let kind = hir::ExprKind::Block {
            stmts: hir_stmts,
            last_expr: hir_last,
        };
        Ok((kind, final_ty))
    }

    fn check_call(
        &mut self,
        fun: &ast::Expr<'src>,
        args: &[ast::Expr<'src>],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_fun = self.check_expr(fun)?;

        // This is a simplification. A real implementation would unify against
        // a function type, but for now we check for function names.
        if let hir::ExprKind::Path(path) = &hir_fun.kind {
            // Check for module-qualified function calls (e.g., Std::Fmt::println)
            if path.len() >= 3 {
                if let Some(module_type) = self.resolve_module_symbol(path) {
                    // For now, assume it's a function that returns unit
                    // In a real implementation, we'd extract the actual function signature
                    let mut hir_args = Vec::new();
                    for arg in args {
                        let hir_arg = self.check_expr(arg)?;
                        hir_args.push(hir_arg);
                    }
                    
                    let kind = hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: hir_args,
                    };
                    return Ok((kind, Ty::Unit));
                }
            }
            
            // First check for enum variant construction (e.g., Option::Some(42))
            if path.len() == 2 {
                if let Some(enum_def) = self.context.get_enum(path[0]) {
                    // Check if the second part is a variant of this enum
                    if let Some(variant_info) = enum_def.variants.iter().find(|(name, _)| name == &path[1]) {
                        let empty_vec = Vec::new();
                        let variant_types = variant_info.1.as_ref().unwrap_or(&empty_vec);
                        
                        // Check that the number of arguments matches the variant fields
                        if args.len() != variant_types.len() {
                            return Err(TypeError::WrongArgumentCount {
                                expected: variant_types.len(),
                                found: args.len(),
                            });
                        }

                        // Clone the enum definition to avoid borrow conflicts
                        let enum_def = enum_def.clone();
                        let variant_types = variant_types.to_vec();

                        // Check each argument and collect their types
                        let mut hir_args = Vec::new();
                        for arg in args {
                            let hir_arg = self.check_expr(arg)?;
                            hir_args.push(hir_arg);
                        }

                        // Create inference variables for the enum's generic parameters
                        let mut enum_generics = Vec::new();
                        for _ in 0..enum_def.generics.len() {
                            enum_generics.push(self.new_infer_ty());
                        }

                        // Create the enum type with inference variables
                        let enum_ty = Ty::Adt {
                            name: vec![path[0]],
                            generics: enum_generics.clone(),
                        };

                        // Unify the argument types with the variant field types
                        for (arg, variant_type) in hir_args.iter().zip(variant_types.iter()) {
                            // Substitute generic parameters in the variant type
                            let mut substitution = HashMap::new();
                            for (generic_param, infer_ty) in enum_def.generics.iter().zip(enum_generics.iter()) {
                                substitution.insert(*generic_param, infer_ty.clone());
                            }
                            let substituted_variant_ty = self.substitute_generics(variant_type, &substitution);
                            self.unify(&arg.ty, &substituted_variant_ty)?;
                        }

                        let kind = hir::ExprKind::Call {
                            fun: Box::new(hir_fun),
                            args: hir_args,
                        };
                        return Ok((kind, enum_ty));
                    }
                } else {
                    println!("DEBUG: Enum not found: {}", path[0]);
                }
            }

            // Then check for regular functions
            if let Some(func_def) = self.context.get_function(path[0]) {
                let func_def = func_def.clone(); // Clone to avoid borrow conflict
                if func_def.params.len() != args.len() {
                    return Err(TypeError::WrongArgumentCount {
                        expected: func_def.params.len(),
                        found: args.len(),
                    });
                }

                let mut hir_args = Vec::new();
                for (arg, (_, param_ty)) in args.iter().zip(func_def.params.iter()) {
                    let lower_param_ty = self.lower_type(param_ty);
                    let hir_arg = self.check_expr_with_hint(arg, &lower_param_ty)?;
                    self.unify(&hir_arg.ty, &lower_param_ty)?;
                    hir_args.push(hir_arg);
                }

                let ret_ty = func_def
                    .ret_type
                    .as_ref()
                    .map_or(Ty::Unit, |t| self.lower_type(t));
                let kind = hir::ExprKind::Call {
                    fun: Box::new(hir_fun),
                    args: hir_args,
                };
                return Ok((kind, ret_ty));
            }
            
            // Then check for extern functions
            if let Some(extern_item) = self.context.get_extern_function(path[0]) {
                if let ast::Item::ExternFn { params, ret_type, .. } = extern_item {
                    let params = params.clone(); // Clone to avoid borrow conflict
                    let ret_type = ret_type.clone(); // Clone to avoid borrow conflict
                    
                    if params.len() != args.len() {
                        return Err(TypeError::WrongArgumentCount {
                            expected: params.len(),
                            found: args.len(),
                        });
                    }

                    let mut hir_args = Vec::new();
                    for (arg, (_, param_ty)) in args.iter().zip(params.iter()) {
                        let lower_param_ty = self.lower_type(param_ty);
                        let hir_arg = self.check_expr_with_hint(arg, &lower_param_ty)?;
                        self.unify(&hir_arg.ty, &lower_param_ty)?;
                        hir_args.push(hir_arg);
                    }

                    let ret_ty = self.lower_type(&ret_type);
                    let kind = hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: hir_args,
                    };
                    return Ok((kind, ret_ty));
                }
            }
        }
        
        // Fallback for unimplemented call types
        Ok((
            hir::ExprKind::Call {
                fun: Box::new(hir_fun),
                args: vec![],
            },
            Ty::Error,
        ))
    }

    fn check_array(
        &mut self,
        elements: &[ast::Expr<'src>],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let inner_ty = self.new_infer_ty();
        let mut hir_elements = Vec::new();
        for el in elements {
            let hir_el = self.check_expr_with_hint(el, &inner_ty)?;
            self.unify(&hir_el.ty, &inner_ty)?;
            hir_elements.push(hir_el);
        }
        let array_ty = Ty::Array(Box::new(self.resolve_type(&inner_ty)));
        Ok((hir::ExprKind::Array(hir_elements), array_ty))
    }

    fn check_struct_init(
        &mut self,
        path: &[&'src str],
        generics: &[ast::Type<'src>],
        fields: &[(&'src str, ast::Expr<'src>)],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let struct_name = path.first().ok_or(TypeError::UnknownStruct(""))?;
        let struct_def = self
            .context
            .get_struct(struct_name)
            .ok_or(TypeError::UnknownStruct(struct_name))?
            .clone();

        // Create a substitution mapping from generic parameters to concrete types
        let mut substitution = HashMap::new();
        for (generic_param, concrete_type) in struct_def.generics.iter().zip(generics.iter()) {
            substitution.insert(*generic_param, self.lower_type(concrete_type));
        }

        let mut hir_fields = HashMap::new();
        for (field_name, field_expr) in fields {
            let (_def_name, field_ty_ast) = struct_def
                .fields
                .iter()
                .find(|(n, _)| n == field_name)
                .ok_or(TypeError::UnknownStructField {
                    struct_name,
                    field_name: *field_name,
                })?;
            
            // Apply generic substitution to the field type
            let field_ty = self.substitute_generics(field_ty_ast, &substitution);
            let hir_expr = self.check_expr_with_hint(field_expr, &field_ty)?;
            self.unify(&hir_expr.ty, &field_ty)?;
            hir_fields.insert(*field_name, hir_expr);
        }

        // Check for missing fields
        for (field_name, _) in &struct_def.fields {
            if !hir_fields.contains_key(field_name) {
                return Err(TypeError::MissingStructField {
                    struct_name,
                    field_name: *field_name,
                });
            }
        }

        let ty = Ty::Adt {
            name: path.to_vec(),
            generics: generics.iter().map(|t| self.lower_type(t)).collect(),
        };
        Ok((
            hir::ExprKind::StructInit {
                path: path.to_vec(),
                fields: hir_fields,
            },
            ty,
        ))
    }

    fn check_match(
        &mut self,
        scrutinee: &ast::Expr<'src>,
        arms: &[(ast::Pattern<'src>, ast::Expr<'src>)],
        type_hint: &Ty<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        // Type-check the scrutinee expression
        let hir_scrutinee = self.check_expr(scrutinee)?;
        let scrutinee_ty = self.resolve_type(&hir_scrutinee.ty);

        // Initialize result type variable for the match expression
        let overall_result_ty = self.new_infer_ty();
        let mut hir_arms = Vec::new();

        // Process each match arm
        for (ast_pattern, arm_expr) in arms {
            // Enter a new scope for this arm
            self.context.enter_scope();

            // Check the pattern and add bindings to scope
            let hir_pattern = self.check_pattern(ast_pattern, &scrutinee_ty)?;

            // Check the arm expression with the overall result type as hint
            let hir_arm_expr = self.check_expr_with_hint(arm_expr, &overall_result_ty)?;

            // Unify the arm expression type with the overall result type
            self.unify(&hir_arm_expr.ty, &overall_result_ty)?;

            // Leave the scope for this arm
            self.context.leave_scope();

            // Add the processed arm to our collection
            hir_arms.push((hir_pattern, hir_arm_expr));
        }

        // The final type is the resolved overall result type
        let final_ty = self.resolve_type(&overall_result_ty);

        let kind = hir::ExprKind::Match {
            scrutinee: Box::new(hir_scrutinee),
            arms: hir_arms,
        };
        Ok((kind, final_ty))
    }

    fn check_while(
        &mut self,
        cond: &ast::Expr<'src>,
        body: &ast::Expr<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        // Check the condition - it must be boolean
        let hir_cond = self.check_expr(cond)?;
        self.unify(&hir_cond.ty, &Ty::Bool)?;

        // Check the body - while loops don't produce a value, so they return unit
        let hir_body = self.check_expr(body)?;

        let kind = hir::ExprKind::While {
            cond: Box::new(hir_cond),
            body: Box::new(hir_body),
        };

        // While loops return unit type
        Ok((kind, Ty::Unit))
    }
} 