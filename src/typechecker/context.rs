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

/// Represents a symbol signature from a module
#[derive(Debug, Clone)]
pub struct SymbolSignature {
    pub name: String,
    pub kind: SymbolKind,
    pub type_info: ast::OwnedType,
}

/// The kind of symbol
#[derive(Debug, Clone)]
pub enum SymbolKind {
    Function(ast::OwnedFunction),
    Struct(ast::OwnedStructDef),
    Enum(ast::OwnedEnumDef),
    Trait(ast::OwnedTraitDef),
    Effect(ast::OwnedEffectDef),
    Handler(ast::OwnedHandlerDef),
    ExternFunction(ast::OwnedType), // return type
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
    /// Extern functions available in the global scope.
    extern_functions: HashMap<&'src str, ast::Item<'src>>,
    /// Struct definitions available in the global scope.
    structs: HashMap<&'src str, ast::StructDef<'src>>,
    /// Enum definitions available in the global scope.
    enums: HashMap<&'src str, ast::EnumDef<'src>>,
    /// Trait definitions available in the global scope.
    traits: HashMap<&'src str, ast::TraitDef<'src>>,
    /// Trait methods available in the global scope (from impl blocks).
    trait_methods: HashMap<&'src str, ast::TraitMethod<'src>>,
    /// Effect definitions available in the global scope.
    effects: HashMap<&'src str, ast::EffectDef<'src>>,

    // --- Import System ---
    /// Cached module symbols (namespace::module -> symbols)
    module_symbols: HashMap<String, HashMap<String, SymbolSignature>>,
}

impl<'src> TypeContext<'src> {
    /// Creates a new `TypeContext` with a single, empty global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::default()], // Start with the global scope.
            functions: HashMap::new(),
            extern_functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            trait_methods: HashMap::new(),
            effects: HashMap::new(),
            module_symbols: HashMap::new(),
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

    /// Adds an extern function to the global context.
    pub fn add_extern_function(&mut self, name: &'src str, item: ast::Item<'src>) {
        self.extern_functions.insert(name, item);
    }

    /// Retrieves an extern function from the global context.
    pub fn get_extern_function(&self, name: &'src str) -> Option<&ast::Item<'src>> {
        self.extern_functions.get(name)
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

    /// Adds a trait definition to the global context.
    pub fn add_trait(&mut self, trait_def: ast::TraitDef<'src>) {
        self.traits.insert(trait_def.name, trait_def);
    }

    /// Retrieves a trait definition from the global context.
    pub fn get_trait(&self, name: &'src str) -> Option<&ast::TraitDef<'src>> {
        self.traits.get(name)
    }

    /// Adds a trait method to the global context.
    pub fn add_trait_method(&mut self, name: &'src str, method: ast::TraitMethod<'src>) {
        self.trait_methods.insert(name, method);
    }

    /// Retrieves a trait method from the global context.
    pub fn get_trait_method(&self, name: &'src str) -> Option<&ast::TraitMethod<'src>> {
        self.trait_methods.get(name)
    }

    /// Adds an effect definition to the global context.
    pub fn add_effect(&mut self, effect_def: ast::EffectDef<'src>) {
        self.effects.insert(effect_def.name, effect_def);
    }

    /// Retrieves an effect definition from the global context.
    pub fn get_effect(&self, name: &'src str) -> Option<&ast::EffectDef<'src>> {
        self.effects.get(name)
    }

    /// Finds an enum that contains a specific variant.
    pub fn find_enum_by_variant(
        &self,
        variant_name: &'src str,
    ) -> Option<(&'src str, &ast::EnumDef<'src>)> {
        for (name, enum_def) in &self.enums {
            if enum_def.variants.iter().any(|(v, _)| v == &variant_name) {
                return Some((name, enum_def));
            }
        }
        None
    }

    // --- Import System ---

    /// Add module symbols to cache
    pub fn add_module_symbols(
        &mut self,
        module_path: String,
        symbols: HashMap<String, SymbolSignature>,
    ) {
        self.module_symbols.insert(module_path, symbols);
    }

    /// Get module symbols from cache
    pub fn get_module_symbols(
        &self,
        module_path: &str,
    ) -> Option<&HashMap<String, SymbolSignature>> {
        self.module_symbols.get(module_path)
    }
}

// Default implementation for convenience.
impl<'src> Default for TypeContext<'src> {
    fn default() -> Self {
        Self::new()
    }
}
