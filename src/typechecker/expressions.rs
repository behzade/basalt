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
            ast::Expr::Unary { op, rhs } => self.check_unary(op, rhs)?,
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
            ast::Expr::Map(pairs) => self.check_map(pairs)?,
            ast::Expr::StructInit {
                path,
                generics,
                fields,
            } => self.check_struct_init(path, generics, fields)?,
            ast::Expr::Match { scrutinee, arms } => self.check_match(scrutinee, arms, type_hint)?,
            ast::Expr::While { cond, body } => self.check_while(cond, body)?,
            ast::Expr::Perform { path, args } => self.check_perform(path, args)?,
            ast::Expr::Handle { body, handler } => self.check_handle(body, handler)?,
            _ => {
                return Ok(hir::Expr {
                    kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                    ty: Ty::Error,
                });
            }
        };

        // Unify the inferred type with the type hint
        self.unify(&ty, type_hint)?;

        Ok(hir::Expr { kind, ty })
    }

    fn check_literal(
        &self,
        lit: &ast::Literal<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let ty = match lit {
            ast::Literal::Bool(_) => Ty::Bool,
            ast::Literal::I64(value) => {
                // Use i32 for small integers, i64 for larger ones
                if *value <= i32::MAX as i64 && *value >= i32::MIN as i64 {
                    Ty::I32
                } else {
                    Ty::I64
                }
            }
            ast::Literal::F64(_) => Ty::F64,
            ast::Literal::Str(_) => Ty::Str,
            ast::Literal::Unit => Ty::Unit,
        };
        Ok((hir::ExprKind::Literal(lit.clone()), ty))
    }

    fn check_path(
        &mut self,
        path: &[&'src str],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
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
                    return Err(TypeError::UnknownModule { namespace, module });
                }
            }
        }

        // Check for enum variant first.
        if resolved_path.len() == 2 {
            if let Some(enum_def) = self.context.get_enum(resolved_path[0]) {
                if enum_def
                    .variants
                    .iter()
                    .any(|(v, _)| v == &resolved_path[1])
                {
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
        let name = resolved_path
            .first()
            .ok_or(TypeError::UnknownVariable(""))?;

        // Check for regular functions
        if let Some(func_def) = self.context.get_function(name) {
            let ret_ty = func_def
                .ret_type
                .as_ref()
                .map_or(Ty::Unit, |t| self.lower_type(t));
            return Ok((
                hir::ExprKind::Path(resolved_path),
                Ty::Function {
                    param_types: func_def
                        .params
                        .iter()
                        .map(|(_, t)| self.lower_type(t))
                        .collect(),
                    ret_type: Box::new(ret_ty),
                },
            ));
        }

        // Check for extern functions
        if let Some(extern_item) = self.context.get_extern_function(name) {
            if let ast::Item::ExternBlock { functions, .. } = extern_item {
                // Find the function with the matching name
                if let Some(function) = functions.iter().find(|f| f.name == *name) {
                    let ret_ty = function.ret_type.as_ref().map_or(Ty::Unit, |t| self.lower_type(t));
                    return Ok((
                        hir::ExprKind::Path(resolved_path),
                        Ty::Function {
                            param_types: function.params.iter().map(|(_, t)| self.lower_type(t)).collect(),
                            ret_type: Box::new(ret_ty),
                        },
                    ));
                }
            }
        }

        // Check for trait methods
        if let Some(trait_method) = self.context.get_trait_method(name) {
            let ret_ty = trait_method
                .ret_type
                .as_ref()
                .map_or(Ty::Unit, |t| self.lower_type(t));
            return Ok((
                hir::ExprKind::Path(resolved_path),
                Ty::Function {
                    param_types: trait_method
                        .params
                        .iter()
                        .map(|(_, t)| self.lower_type(t))
                        .collect(),
                    ret_type: Box::new(ret_ty),
                },
            ));
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

    fn check_unary(
        &mut self,
        op: &ast::UnaryOp,
        rhs: &ast::Expr<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_rhs = self.check_expr(rhs)?;

        let ty = match op {
            ast::UnaryOp::Neg => {
                // Negation only works on numeric types
                if !matches!(hir_rhs.ty, Ty::I64 | Ty::F64) {
                    return Err(TypeError::InvalidOperator {
                        op: "-".to_string(),
                        ty: hir_rhs.ty,
                    });
                }
                hir_rhs.ty.clone()
            }
            ast::UnaryOp::Not => {
                // Logical negation only works on boolean types
                if !matches!(hir_rhs.ty, Ty::Bool) {
                    return Err(TypeError::InvalidOperator {
                        op: "!".to_string(),
                        ty: hir_rhs.ty,
                    });
                }
                Ty::Bool
            }
        };

        let kind = hir::ExprKind::Unary {
            op: *op,
            rhs: Box::new(hir_rhs),
        };
        Ok((kind, ty))
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
            ast::BinaryOp::Add | ast::BinaryOp::Sub | ast::BinaryOp::Mul | ast::BinaryOp::Div => {
                self.unify(&hir_lhs.ty, &hir_rhs.ty)?;
                let resolved_lhs = self.resolve_type(&hir_lhs.ty);
                if !matches!(resolved_lhs, Ty::I32 | Ty::I64 | Ty::F64 | Ty::Str) {
                    return Err(TypeError::InvalidOperator {
                        op: op.to_string(),
                        ty: resolved_lhs,
                    });
                }
                resolved_lhs
            }
            ast::BinaryOp::Eq | ast::BinaryOp::Ne | ast::BinaryOp::Lt | ast::BinaryOp::Gt => {
                self.unify(&hir_lhs.ty, &hir_rhs.ty)?;
                Ty::Bool
            }
            ast::BinaryOp::Assign => {
                // Assignment should return the type of the right-hand side
                // For now, just return the right-hand side type
                hir_rhs.ty.clone()
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

        for stmt in stmts {
            // Check if this is a return statement
            if let ast::Stmt::Return(Some(expr)) = stmt {
                let hir_expr = self.check_expr_with_hint(expr, type_hint)?;
                self.unify(&hir_expr.ty, type_hint)?;
                block_ty = hir_expr.ty.clone();
                hir_stmts.push(hir::Stmt::Return(Some(hir_expr)));
            } else {
                let hir_stmt = self.check_stmt(stmt)?;
                hir_stmts.push(hir_stmt);
            }
        }

        // Check the last expression if present
        let hir_last_expr = if let Some(expr) = last_expr {
            let hir_expr = self.check_expr_with_hint(expr, type_hint)?;
            self.unify(&hir_expr.ty, type_hint)?;
            block_ty = hir_expr.ty.clone();
            Some(Box::new(hir_expr))
        } else {
            None
        };

        self.context.leave_scope();

        Ok((
            hir::ExprKind::Block {
                stmts: hir_stmts,
                last_expr: hir_last_expr,
            },
            block_ty,
        ))
    }

    fn check_call(
        &mut self,
        fun: &ast::Expr<'src>,
        args: &[ast::Expr<'src>],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_fun = self.check_expr(fun)?;

        // Check for array indexing: get(array, index)
        if let hir::ExprKind::Path(path) = &hir_fun.kind {
            if path == &["get"] && args.len() == 2 {
                let array_expr = self.check_expr(&args[0])?;
                let index_expr = self.check_expr(&args[1])?;

                // Check that the first argument is an array
                if let Ty::Array(element_ty) = &array_expr.ty {
                    // For array access, the index should be an integer
                    self.unify(&index_expr.ty, &Ty::I64)?;
                    let kind = hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: vec![array_expr.clone(), index_expr.clone()],
                    };
                    return Ok((kind, element_ty.as_ref().clone()));
                } else if let Ty::Map { key: key_ty, value: value_ty } = &array_expr.ty {
                    // For map access, the key should match the map's key type
                    self.unify(&index_expr.ty, key_ty)?;
                    let kind = hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: vec![array_expr.clone(), index_expr.clone()],
                    };
                    return Ok((kind, value_ty.as_ref().clone()));
                } else {
                    return Err(TypeError::MismatchedTypes {
                        expected: Ty::Array(Box::new(Ty::Infer(0))),
                        found: array_expr.ty,
                    });
                }
            }

            // Check for struct field access: get_field(struct, field_name)
            if path == &["get_field"] && args.len() == 2 {
                let struct_expr = self.check_expr(&args[0])?;
                let field_name_expr = self.check_expr(&args[1])?;

                // Check that the field name is a string
                self.unify(&field_name_expr.ty, &Ty::Str)?;

                // Check that the first argument is a struct (ADT)
                if let Ty::Adt { name, .. } = &struct_expr.ty {
                    // Look up the struct definition to get the field type
                    let struct_name = name.first().ok_or(TypeError::UnknownStruct(""))?;
                    if let Some(struct_def) = self.context.get_struct(struct_name) {
                        // Extract the field name from the string literal
                        if let ast::Expr::Literal(ast::Literal::Str(field_name)) = &args[1] {
                            // Find the field in the struct definition
                            if let Some((_, field_ty_ast)) = struct_def.fields.iter().find(|(n, _)| n == field_name) {
                                // Convert the AST type to HIR type
                                let field_ty = self.lower_type(field_ty_ast);
                                let kind = hir::ExprKind::Call {
                                    fun: Box::new(hir_fun),
                                    args: vec![struct_expr.clone(), field_name_expr.clone()],
                                };
                                return Ok((kind, field_ty));
                            } else {
                                return Err(TypeError::UnknownStructField {
                                    struct_name,
                                    field_name,
                                });
                            }
                        } else {
                            // If the field name is not a string literal, we can't determine the field type at compile time
                            let field_ty = self.new_infer_ty();
                            let kind = hir::ExprKind::Call {
                                fun: Box::new(hir_fun),
                                args: vec![struct_expr.clone(), field_name_expr.clone()],
                            };
                            return Ok((kind, field_ty));
                        }
                    } else {
                        return Err(TypeError::UnknownStruct(struct_name));
                    }
                } else {
                    return Err(TypeError::MismatchedTypes {
                        expected: Ty::Adt { name: vec!["struct"], generics: vec![] },
                        found: struct_expr.ty,
                    });
                }
            }
        }

        // This is a simplification. A real implementation would unify against
        // a function type, but for now we check for function names.
        if let hir::ExprKind::Path(path) = &hir_fun.kind {
            // Check for module-qualified function calls (e.g., Std::Fmt::println)
            if path.len() >= 3 {
                if let Some(_module_type) = self.resolve_module_symbol(path) {
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
                    if let Some(variant_info) =
                        enum_def.variants.iter().find(|(name, _)| name == &path[1])
                    {
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
                            for (generic_param, infer_ty) in
                                enum_def.generics.iter().zip(enum_generics.iter())
                            {
                                substitution.insert(*generic_param, infer_ty.clone());
                            }
                            let substituted_variant_ty =
                                self.substitute_generics(variant_type, &substitution);
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
                let mut substitution = HashMap::new();

                // Check arguments and collect type information for generic inference
                for (arg, (_, param_ty)) in args.iter().zip(func_def.params.iter()) {
                    let hir_arg = self.check_expr(arg)?;
                    hir_args.push(hir_arg.clone());

                    // If the parameter type is a generic parameter, try to infer it from the argument
                    if let ast::Type { path: param_path, generics: param_generics } = param_ty {
                        if param_generics.is_empty() && param_path.len() == 1 {
                            let param_name = param_path[0];
                            // Check if this is a generic parameter of the function
                            if func_def.generics.contains(&param_name) {
                                // Infer the type from the argument
                                substitution.insert(param_name, hir_arg.ty.clone());
                            }
                        }
                    }
                }

                // Apply substitution to return type
                let ret_ty = if let Some(ret_type) = &func_def.ret_type {
                    if substitution.is_empty() {
                        self.lower_type(ret_type)
                    } else {
                        self.substitute_generics(ret_type, &substitution)
                    }
                } else {
                    Ty::Unit
                };

                return Ok((
                    hir::ExprKind::Call {
                        fun: Box::new(hir_fun),
                        args: hir_args,
                    },
                    ret_ty,
                ));
            }

            // Then check for extern functions
            if let Some(extern_item) = self.context.get_extern_function(path[0]) {
                if let ast::Item::ExternBlock { functions, .. } = extern_item {
                    // Find the function with the matching name
                    if let Some(function) = functions.iter().find(|f| f.name == path[0]) {
                        let params = function.params.clone(); // Clone to avoid borrow conflict
                        let ret_type = function.ret_type.as_ref().map_or(
                            ast::Type { path: vec!["none"], generics: vec![] },
                            |t| t.clone()
                        ); // Clone to avoid borrow conflict

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

    fn check_map(
        &mut self,
        pairs: &[(ast::Expr<'src>, ast::Expr<'src>)],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let key_ty = self.new_infer_ty();
        let value_ty = self.new_infer_ty();

        let mut hir_pairs = Vec::new();
        for (key_expr, value_expr) in pairs {
            let hir_key = self.check_expr_with_hint(key_expr, &key_ty)?;
            let hir_value = self.check_expr_with_hint(value_expr, &value_ty)?;

            self.unify(&hir_key.ty, &key_ty)?;
            self.unify(&hir_value.ty, &value_ty)?;

            hir_pairs.push((hir_key, hir_value));
        }

        let map_ty = Ty::Map {
            key: Box::new(self.resolve_type(&key_ty)),
            value: Box::new(self.resolve_type(&value_ty)),
        };
        Ok((hir::ExprKind::Map(hir_pairs), map_ty))
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
        _type_hint: &Ty<'src>,
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

    fn check_perform(
        &mut self,
        path: &[&'src str],
        args: &[ast::Expr<'src>],
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        // Check each argument
        let mut hir_args = Vec::new();
        for arg in args {
            let hir_arg = self.check_expr(arg)?;
            hir_args.push(hir_arg);
        }

        // Look up the effect operation signature
        if path.len() >= 2 {
            let effect_name = path[0];
            let operation_name = path[1];

            // Try to find the effect definition
            if let Some(effect_def) = self.context.get_effect(effect_name) {
                // Find the operation in the effect
                if let Some(operation) = effect_def
                    .operations
                    .iter()
                    .find(|op| op.name == operation_name)
                {
                    // Check argument count
                    if args.len() != operation.params.len() {
                        return Err(TypeError::WrongArgumentCount {
                            expected: operation.params.len(),
                            found: args.len(),
                        });
                    }

                    // Convert operation types to HIR types first
                    let hir_param_types: Vec<Ty<'src>> = operation
                        .params
                        .iter()
                        .map(|t| self.lower_type(t))
                        .collect();
                    let hir_ret_type = self.lower_type(&operation.ret_type);

                    // Check argument types
                    for (arg, param_ty) in hir_args.iter().zip(hir_param_types.iter()) {
                        self.unify(&arg.ty, param_ty)?;
                    }

                    return Ok((
                        hir::ExprKind::Perform {
                            path: path.to_vec(),
                            args: hir_args,
                        },
                        hir_ret_type,
                    ));
                }
            }
        }

        // Fallback: assume unit type if we can't find the effect operation
        Ok((
            hir::ExprKind::Perform {
                path: path.to_vec(),
                args: hir_args,
            },
            Ty::Unit,
        ))
    }

    fn check_handle(
        &mut self,
        body: &ast::Expr<'src>,
        handler: &ast::HandlerBody<'src>,
    ) -> Result<(hir::ExprKind<'src>, Ty<'src>), TypeError<'src>> {
        let hir_body = self.check_expr(body)?;

        let hir_handler = match handler {
            ast::HandlerBody::Path(path) => hir::HandlerBody::Path(path.clone()),
            ast::HandlerBody::Inline(_functions) => {
                // For now, just use the path version
                // In a full implementation, we'd check the functions
                hir::HandlerBody::Path(vec!["inline_handler"])
            }
        };

        // For now, assume handle returns the same type as the body
        // In a full implementation, we'd check effect row compatibility
        Ok((
            hir::ExprKind::Handle {
                body: Box::new(hir_body.clone()),
                handler: hir_handler,
            },
            hir_body.ty,
        ))
    }
}

