//! typechecker/items.rs
//!
//! This module contains the logic for checking and lowering top-level items
//! (functions, structs, enums, etc.) from AST to HIR.

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
            ast::Item::Struct(struct_def) => self.check_struct(struct_def),
            ast::Item::Enum(enum_def) => self.check_enum(enum_def),
            ast::Item::Trait(trait_def) => self.check_trait(trait_def),
            ast::Item::Impl(impl_block) => self.check_impl(impl_block),
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
            ast::Item::Effect(effect_def) => self.check_effect(effect_def),
            ast::Item::Handler(handler_def) => self.check_handler(handler_def),
            _ => Ok(hir::Item::Stmt(hir::Stmt::Expr(hir::Expr {
                kind: hir::ExprKind::Literal(ast::Literal::Bool(true)),
                ty: Ty::Unit,
            }))),
        }
    }

    /// Checks a function definition, including its body, and lowers it to HIR.
    pub fn check_function(
        &mut self,
        func: &ast::Function<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        self.context.enter_scope();

        // Add generic type parameters to the scope as inference variables
        for generic_param in &func.generics {
            let infer_ty = self.new_infer_ty();
            self.context.add_variable(generic_param, infer_ty);
        }

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

    /// Checks a struct definition and lowers it to HIR.
    pub fn check_struct(
        &mut self,
        struct_def: &ast::StructDef<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
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

    /// Checks an enum definition and lowers it to HIR.
    pub fn check_enum(
        &mut self,
        enum_def: &ast::EnumDef<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        let hir_enum = hir::EnumDef {
            name: enum_def.name,
            generics: enum_def.generics.clone(),
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

    /// Checks a trait definition and lowers it to HIR.
    pub fn check_trait(
        &mut self,
        trait_def: &ast::TraitDef<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        let hir_methods = trait_def
            .methods
            .iter()
            .map(|method| hir::TraitMethod {
                name: method.name,
                params: method
                    .params
                    .iter()
                    .map(|(name, ty)| (*name, self.lower_type(ty)))
                    .collect(),
                ret_type: method
                    .ret_type
                    .as_ref()
                    .map_or(Ty::Unit, |t| self.lower_type(t)),
                is_public: method.is_public,
            })
            .collect();

        let hir_trait = hir::TraitDef {
            name: trait_def.name,
            methods: hir_methods,
            is_public: trait_def.is_public,
        };
        Ok(hir::Item::Trait(hir_trait))
    }

    /// Checks an impl block and lowers it to HIR.
    pub fn check_impl(
        &mut self,
        impl_block: &ast::ImplBlock<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        // Check each method in the impl block and collect them
        let mut hir_methods = Vec::new();
        for method in &impl_block.methods {
            let hir_method = self.check_function(method)?;
            if let hir::Item::Fn(func) = hir_method {
                hir_methods.push(func);
            }
        }
        
        let hir_impl = hir::ImplBlock {
            trait_name: impl_block.trait_name,
            target_type: self.lower_type(&impl_block.target_type),
            methods: hir_methods,
        };
        Ok(hir::Item::Impl(hir_impl))
    }

    /// Checks an effect definition and lowers it to HIR.
    pub fn check_effect(
        &mut self,
        effect_def: &ast::EffectDef<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        let hir_operations = effect_def
            .operations
            .iter()
            .map(|op| hir::EffectOp {
                name: op.name,
                params: op.params.iter().map(|t| self.lower_type(t)).collect(),
                ret_type: self.lower_type(&op.ret_type),
                is_public: op.is_public,
            })
            .collect();

        let hir_effect = hir::EffectDef {
            name: effect_def.name,
            operations: hir_operations,
            is_public: effect_def.is_public,
        };
        Ok(hir::Item::Effect(hir_effect))
    }

    /// Checks a handler definition and lowers it to HIR.
    pub fn check_handler(
        &mut self,
        handler_def: &ast::HandlerDef<'src>,
    ) -> Result<hir::Item<'src>, TypeError<'src>> {
        // Check each function in the handler and collect them
        let mut hir_functions = Vec::new();
        for func in &handler_def.functions {
            let hir_func = self.check_function(func)?;
            if let hir::Item::Fn(func) = hir_func {
                hir_functions.push(func);
            }
        }

        let hir_handler = hir::HandlerDef {
            name: handler_def.name,
            effects: handler_def.effects.clone(),
            functions: hir_functions,
            is_public: handler_def.is_public,
        };
        Ok(hir::Item::Handler(hir_handler))
    }
} 