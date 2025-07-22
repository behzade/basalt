//! typechecker/context.rs
//!
//! Defines the `TypeContext` and `Scope` structs, which are responsible for
//! managing symbol information during type checking. The context tracks all
//_ visible identifiers like variables, functions, and type definitions.

use crate::{ast, hir};
use std::collections::HashMap;

/// Represents a single lexical scope within the program, such as the body of a
/// function or a block expression.
#[derive(Debug, Clone, Default)]
pub struct Scope<'src> {
    /// A map of variable names to their resolved types (`hir::Ty`) in the current scope.
    variables: HashMap<&'src str, hir::Ty<'src>>,
    // In the future, this could also hold local type definitions or other
    // scope-specific information.
}

/// The `TypeContext` holds all the information needed to resolve types.
/// It maintains a stack of scopes to correctly handle lexical scoping and
/// also stores global definitions like functions and structs.
#[derive(Debug, Clone)]
pub struct TypeContext<'src> {
    /// A stack of scopes. The last scope in the vector is the current, innermost scope.
    scopes: Vec<Scope<'src>>,

    // --- Global Definitions ---
    /// Functions available in the global scope.
    functions: HashMap<&'src str, ast::Function<'src>>,
    /// Struct definitions available in the global scope.
    structs: HashMap<&'src str, ast::StructDef<'src>>,
    /// Enum definitions available in the global scope.
    enums: HashMap<&'src str, ast::EnumDef<'src>>,
    // TODO: Add tables for traits, impls, etc. as they are implemented.
}

impl<'src> TypeContext<'src> {
    /// Creates a new `TypeContext` with a single, empty global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::default()], // Start with the global scope.
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
        }
    }

    // --- Scope Management ---

    /// Pushes a new, empty scope onto the stack, entering a deeper lexical context.
    /// This is used when entering a function body or a block expression.
    pub fn enter_scope(&mut self) {
        self.scopes.push(Scope::default());
    }

    /// Pops the current scope from the stack, leaving the current lexical context.
    /// This is used after checking a function body or a block expression.
    pub fn leave_scope(&mut self) {
        // We should never be able to pop the global scope.
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    // --- Symbol Management ---

    /// Adds a variable to the *current* (innermost) scope.
    pub fn add_variable(&mut self, name: &'src str, ty: hir::Ty<'src>) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.variables.insert(name, ty);
        }
    }

    /// Searches for a variable's type, starting from the innermost scope and
    /// moving outwards.
    pub fn get_variable(&self, name: &'src str) -> Option<&hir::Ty<'src>> {
        for scope in self.scopes.iter().rev() {
            if let Some(ty) = scope.variables.get(name) {
                return Some(ty);
            }
        }
        None
    }

    /// Adds a function definition to the global context.
    pub fn add_function(&mut self, func: ast::Function<'src>) {
        self.functions.insert(func.name, func);
    }

    /// Retrieves a function definition from the global context.
    pub fn get_function(&self, name: &'src str) -> Option<&ast::Function<'src>> {
        self.functions.get(name)
    }

    /// Adds a struct definition to the global context.
    pub fn add_struct(&mut self, struct_def: ast::StructDef<'src>) {
        self.structs.insert(struct_def.name, struct_def);
    }

    /// Retrieves a struct definition from the global context.
    pub fn get_struct(&self, name: &'src str) -> Option<&ast::StructDef<'src>> {
        self.structs.get(name)
    }

    /// Adds an enum definition to the global context.
    pub fn add_enum(&mut self, enum_def: ast::EnumDef<'src>) {
        if let Some(name) = enum_def.name {
            self.enums.insert(name, enum_def);
        }
    }

    /// Retrieves an enum definition from the global context.
    pub fn get_enum(&self, name: &'src str) -> Option<&ast::EnumDef<'src>> {
        self.enums.get(name)
    }
}

// Default implementation for convenience.
impl<'src> Default for TypeContext<'src> {
    fn default() -> Self {
        Self::new()
    }
}
