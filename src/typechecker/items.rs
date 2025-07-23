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
    pub fn check_function(
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
} 