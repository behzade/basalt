//! typechecker/types.rs
//!
//! This module contains the logic for type lowering and substitution.

use super::TypeChecker;
use crate::{ast, hir, hir::Ty};
use std::collections::HashMap;

impl<'src> TypeChecker<'src> {
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
    pub fn substitute_generics(&self, ast_ty: &ast::Type<'src>, substitution: &HashMap<&'src str, hir::Ty<'src>>) -> hir::Ty<'src> {
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