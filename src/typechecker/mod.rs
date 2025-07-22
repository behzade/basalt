//! typechecker/mod.rs
//!
//! This module serves as the main entry point for the type-checking process.
//! It transforms the Abstract Syntax Tree (AST) into a typed Hierarchical
//! Intermediate Representation (HIR), validating the program's type safety
//! along the way.

pub use self::context::TypeContext;
pub use self::error::TypeError;

mod check;
mod context;
mod error;
mod unification;

use crate::ast;
use crate::hir;
use crate::hir::Ty; // Import Ty for the substitutions map
use std::collections::HashMap;

pub struct TypeChecker<'src> {
    context: TypeContext<'src>,
    errors: Vec<TypeError<'src>>,
    next_infer_var: u32,
    /// FIX: Added the missing `substitutions` field. This map stores the
    /// solutions for inference variables found during unification.
    substitutions: HashMap<u32, Ty<'src>>,
}

impl<'src> TypeChecker<'src> {
    pub fn new() -> Self {
        Self {
            context: TypeContext::new(),
            errors: Vec::new(),
            next_infer_var: 0,
            // FIX: Initialize the new field.
            substitutions: HashMap::new(),
        }
    }

    pub fn check_file(
        mut self,
        items: &[ast::Item<'src>],
    ) -> Result<Vec<hir::Item<'src>>, Vec<TypeError<'src>>> {
        for item in items {
            if let Err(e) = self.collect_definitions(item) {
                self.errors.push(e);
            }
        }

        if !self.errors.is_empty() {
            return Err(self.errors);
        }

        let mut hir_items = Vec::new();
        for item in items {
            match self.check_item(item) {
                Ok(hir_item) => hir_items.push(hir_item),
                Err(e) => self.errors.push(e),
            }
        }

        if self.errors.is_empty() {
            Ok(hir_items)
        } else {
            Err(self.errors)
        }
    }

    fn next_infer_id(&mut self) -> u32 {
        let id = self.next_infer_var;
        self.next_infer_var += 1;
        id
    }

    fn new_infer_ty(&mut self) -> hir::Ty<'src> {
        hir::Ty::Infer(self.next_infer_id())
    }

    fn collect_definitions(&mut self, item: &ast::Item<'src>) -> Result<(), TypeError<'src>> {
        match item {
            ast::Item::Fn(func) => {
                self.context.add_function(func.clone());
            }
            ast::Item::ExternFn { name, .. } => {
                self.context.add_extern_function(name, item.clone());
            }
            ast::Item::Struct(struct_def) => {
                self.context.add_struct(struct_def.clone());
            }
            ast::Item::Enum(enum_def) => {
                self.context.add_enum(enum_def.clone());
            }
            _ => {}
        }
        Ok(())
    }
}

impl<'src> Default for TypeChecker<'src> {
    fn default() -> Self {
        Self::new()
    }
}
