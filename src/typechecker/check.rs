//! typechecker/check.rs
//!
//! This module contains the core logic for traversing the AST and lowering it
//! to the HIR. It's the "second pass" of the type checker, responsible for
//! validating type correctness for all statements and expressions.

use super::{TypeChecker, TypeError};
use crate::{ast, hir, hir::Ty};
use std::collections::HashMap;

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
            ast::Item::Struct(struct_def) => {
                let hir_struct = hir::StructDef {
                    name: struct_def.name,
                    generics: struct_def.generics.clone(),
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
            // Pass through other items without full validation for now.
            ast::Item::Import { path, alias } => Ok(hir::Item::Import {
                path: path.clone(),
                alias: *alias,
            }),
            ast::Item::ExternFn {
                name,
                params,
                ret_type,
            } => Ok(hir::Item::ExternFn {
                name: *name,
                params: params
                    .iter()
                    .map(|(n, t)| (*n, self.lower_type(t)))
                    .collect(),
                ret_type: self.lower_type(ret_type),
            }),
            _ => Ok(hir::Item::Stmt(hir::Stmt::Expr(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Unit,
            }))),
        }
    }

    /// Checks a function definition, including its body, and lowers it to HIR.
    fn check_function(
        &mut self,
        func: &ast::Function<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        self.context.enter_scope();

        let mut hir_params = Vec::new();
        for (name_opt, ty) in &func.params {
            let hir_ty = self.lower_type(ty);
            if let Some(name) = name_opt {
                self.context.add_variable(name, hir_ty.clone());
            }
            hir_params.push((*name_opt, hir_ty));
        }

        let expected_ret_ty = func
            .ret_type
            .as_ref()
            .map_or(Ty::Unit, |rt| self.lower_type(rt));

        // Pass the expected return type to the body checker.
        let body = self.check_expr_with_hint(&func.body, &expected_ret_ty)?;

        // Unify the actual body's return type with the function's declared return type.
        if let Err(_) = self.unify(&body.ty, &expected_ret_ty) {
            return Err(TypeError::MismatchedTypes {
                expected: self.resolve_type(&expected_ret_ty),
                found: self.resolve_type(&body.ty),
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

                self.context
                    .add_variable(name, self.resolve_type(&final_ty));

                Ok(hir::Stmt::Let {
                    name,
                    is_mut: *is_mut,
                    value_ty: self.resolve_type(&final_ty),
                    value: hir_value,
                })
            }
            ast::Stmt::Expr(expr) => {
                let hir_expr = self.check_expr(expr)?;
                Ok(hir::Stmt::Expr(hir_expr))
            }
            ast::Stmt::Return(expr) => {
                // The actual check happens inside check_expr_with_hint for a block.
                // Here we just lower the expression inside the return.
                let hir_expr = if let Some(e) = expr {
                    Some(self.check_expr(e)?)
                } else {
                    None
                };
                Ok(hir::Stmt::Return(hir_expr))
            }
            ast::Stmt::Assign(lhs, rhs) => {
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
            ast::Stmt::Error => Ok(hir::Stmt::Expr(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Error,
            })),
        }
    }

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
                // Check for enum variant first.
                if path.len() == 2 {
                    if let Some(enum_def) = self.context.get_enum(path[0]) {
                        if enum_def.variants.iter().any(|(v, _)| v == &path[1]) {
                            // This is an enum variant, not a variable. Its type is the enum itself.
                            let enum_ty = self.lower_type(&ast::Type {
                                path: vec![path[0]],
                                generics: vec![],
                            });
                            return Ok(hir::Expr {
                                kind: hir::ExprKind::Path(path.clone()),
                                ty: enum_ty,
                            });
                        }
                    }
                }

                // Check for functions and extern functions
                let name = path.first().ok_or(TypeError::UnknownVariable(""))?;
                
                // Check for regular functions
                if let Some(func_def) = self.context.get_function(name) {
                    let ret_ty = func_def
                        .ret_type
                        .as_ref()
                        .map_or(Ty::Unit, |t| self.lower_type(t));
                    return Ok(hir::Expr {
                        kind: hir::ExprKind::Path(path.clone()),
                        ty: Ty::Function {
                            param_types: func_def.params.iter().map(|(_, t)| self.lower_type(t)).collect(),
                            ret_type: Box::new(ret_ty),
                        },
                    });
                }
                
                // Check for extern functions
                if let Some(extern_item) = self.context.get_extern_function(name) {
                    if let ast::Item::ExternFn { params, ret_type, .. } = extern_item {
                        let ret_ty = self.lower_type(ret_type);
                        return Ok(hir::Expr {
                            kind: hir::ExprKind::Path(path.clone()),
                            ty: Ty::Function {
                                param_types: params.iter().map(|(_, t)| self.lower_type(t)).collect(),
                                ret_type: Box::new(ret_ty),
                            },
                        });
                    }
                }

                // Otherwise, assume it's a variable.
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
                (kind, ty)
            }
            ast::Expr::If {
                cond,
                then_block,
                else_block,
            } => {
                let hir_cond = self.check_expr(cond)?;
                self.unify(&hir_cond.ty, &Ty::Bool)?;

                let hir_then = self.check_expr_with_hint(then_block, type_hint)?;

                let (hir_else, final_ty) = if let Some(else_b) = else_block {
                    let hir_else = self.check_expr_with_hint(else_b, &hir_then.ty)?;
                    self.unify(&hir_then.ty, &hir_else.ty)?;
                    (Some(Box::new(hir_else)), hir_then.ty.clone())
                } else {
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
                let mut block_ty = Ty::Unit; // Default to unit
                let mut has_return = false;

                for stmt in stmts {
                    // If we find a return, its type defines the rest of the block.
                    if let ast::Stmt::Return(Some(expr)) = stmt {
                        let hir_expr = self.check_expr_with_hint(expr, type_hint)?;
                        self.unify(&hir_expr.ty, type_hint)?;
                        block_ty = hir_expr.ty.clone();
                        hir_stmts.push(hir::Stmt::Return(Some(hir_expr)));
                        has_return = true;
                        break; // No more statements matter
                    }
                    hir_stmts.push(self.check_stmt(stmt)?);
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
                (kind, final_ty)
            }
            ast::Expr::Call { fun, args } => {
                let hir_fun = self.check_expr(fun)?;
                let fun_ty = self.resolve_type(&hir_fun.ty);

                // This is a simplification. A real implementation would unify against
                // a function type, but for now we check for function names.
                if let hir::ExprKind::Path(path) = &hir_fun.kind {
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

                                // Check each argument and collect their types
                                let mut hir_args = Vec::new();
                                for arg in args {
                                    let hir_arg = self.check_expr(arg)?;
                                    hir_args.push(hir_arg);
                                }

                                // For now, use a simple enum type without generics
                                let enum_ty = Ty::Adt {
                                    name: vec![path[0]],
                                    generics: vec![],
                                };

                                let kind = hir::ExprKind::Call {
                                    fun: Box::new(hir_fun),
                                    args: hir_args,
                                };
                                return Ok(hir::Expr { kind, ty: enum_ty });
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
                        return Ok(hir::Expr { kind, ty: ret_ty });
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
                            return Ok(hir::Expr { kind, ty: ret_ty });
                        }
                    }
                }
                // Fallback for unimplemented call types
                (
                    hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: vec![],
                    },
                    Ty::Error,
                )
            }
            ast::Expr::Array(elements) => {
                let inner_ty = self.new_infer_ty();
                let mut hir_elements = Vec::new();
                for el in elements {
                    let hir_el = self.check_expr_with_hint(el, &inner_ty)?;
                    self.unify(&hir_el.ty, &inner_ty)?;
                    hir_elements.push(hir_el);
                }
                let array_ty = Ty::Array(Box::new(self.resolve_type(&inner_ty)));
                (hir::ExprKind::Array(hir_elements), array_ty)
            }
            ast::Expr::StructInit { path, generics, fields } => {
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
                    name: path.clone(),
                    generics: generics.iter().map(|t| self.lower_type(t)).collect(),
                };
                (
                    hir::ExprKind::StructInit {
                        path: path.clone(),
                        fields: hir_fields,
                    },
                    ty,
                )
            }
            ast::Expr::Match { scrutinee, arms } => {
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
                (kind, final_ty)
            }
            _ => (hir::ExprKind::Literal(ast::Literal::Bool(true)), Ty::Error),
        };

        Ok(hir::Expr { kind, ty })
    }

    /// Checks a pattern and converts it to a `hir::Pattern`.
    /// This function handles pattern matching and adds bindings to the current scope.
    fn check_pattern(
        &mut self,
        pattern: &ast::Pattern<'src>,
        expected_ty: &hir::Ty<'src>,
    ) -> Result<hir::Pattern<'src>, TypeError<'src>> {
        // Unify the pattern's expected type with the scrutinee type
        let pattern_ty = expected_ty.clone();

        let kind = match (&pattern.path[..], &pattern.args[..]) {
            // Wildcard pattern: `_`
            (["_"], []) => {
                hir::PatternKind::Wildcard
            }
            // Binding pattern: `x` (single identifier)
            ([name], []) => {
                // Add the variable to the current scope
                self.context.add_variable(name, pattern_ty.clone());
                hir::PatternKind::Binding {
                    name,
                    is_mut: false, // For now, assume all bindings are immutable
                }
            }
            // ADT variant pattern: `Option::Some(x)` or `Some(x)`
            (path, args) => {
                // Handle both qualified (Option::Some) and unqualified (Some) paths
                let (enum_name, variant_name) = if path.len() == 2 {
                    (path[0], path[1])
                } else if path.len() == 1 {
                    // For unqualified paths like `Some(x)`, search through all known enums
                    // to find the one containing this variant
                    let variant_name = path[0];
                    let (enum_name, _) = self.context
                        .find_enum_by_variant(variant_name)
                        .ok_or(TypeError::UnknownEnumVariant {
                            enum_name: "unknown",
                            variant_name,
                        })?;
                    (enum_name, variant_name)
                } else {
                    return Err(TypeError::InvalidPattern {
                        pattern: format!("{:?}", pattern),
                    });
                };

                // Look up the enum definition
                let enum_def = self
                    .context
                    .get_enum(enum_name)
                    .ok_or(TypeError::UnknownEnum(enum_name))?;

                // Find the variant and get its field types
                let variant_info = enum_def
                    .variants
                    .iter()
                    .find(|(name, _)| name == &variant_name)
                    .ok_or(TypeError::UnknownEnumVariant {
                        enum_name,
                        variant_name,
                    })?;
                let empty_vec = Vec::new();
                let variant_types = variant_info.1.as_ref().unwrap_or(&empty_vec);

                // Check that the number of pattern arguments matches the variant fields
                if args.len() != variant_types.len() {
                    return Err(TypeError::WrongArgumentCount {
                        expected: variant_types.len(),
                        found: args.len(),
                    });
                }

                // Convert variant types to HIR types first to avoid borrow issues
                let hir_variant_types: Vec<hir::Ty<'src>> = variant_types
                    .iter()
                    .map(|ty| self.lower_type(ty))
                    .collect();

                // Recursively check each sub-pattern
                let mut fields = Vec::new();
                for (arg_name, field_ty) in args.iter().zip(hir_variant_types.iter()) {
                    let field_pattern = ast::Pattern {
                        path: vec![arg_name],
                        args: vec![],
                    };
                    let hir_field_pattern = self.check_pattern(&field_pattern, field_ty)?;
                    fields.push(hir_field_pattern);
                }

                hir::PatternKind::AdtVariant {
                    path: path.to_vec(),
                    fields,
                }
            }
        };

        Ok(hir::Pattern {
            kind,
            ty: pattern_ty,
        })
    }

    /// Lowers an `ast::Type` to an `hir::Ty`.
    pub fn lower_type(&self, ast_ty: &ast::Type<'src>) -> hir::Ty<'src> {
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

    /// Substitutes generic type parameters in an `ast::Type` with concrete types.
    fn substitute_generics(&self, ast_ty: &ast::Type<'src>, substitution: &HashMap<&'src str, hir::Ty<'src>>) -> hir::Ty<'src> {
        let name = ast_ty.path.first().unwrap_or(&"");
        
        // Check if this is a generic parameter that needs substitution
        if substitution.contains_key(name) {
            return substitution[name].clone();
        }
        
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
                    .map_or(Ty::Error, |t| self.substitute_generics(t, substitution));
                Ty::Array(Box::new(inner))
            }
            "Map" => {
                let key = ast_ty
                    .generics
                    .get(0)
                    .map_or(Ty::Error, |t| self.substitute_generics(t, substitution));
                let value = ast_ty
                    .generics
                    .get(1)
                    .map_or(Ty::Error, |t| self.substitute_generics(t, substitution));
                Ty::Map {
                    key: Box::new(key),
                    value: Box::new(value),
                }
            }
            _ => {
                let mut generics = Vec::new();
                for generic_param in &ast_ty.generics {
                    generics.push(self.substitute_generics(generic_param, substitution));
                }
                Ty::Adt {
                    name: ast_ty.path.clone(),
                    generics,
                }
            }
        }
    }
}

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
