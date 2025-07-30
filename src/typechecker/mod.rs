//! typechecker/mod.rs
//!
//! This module serves as the main entry point for the type-checking process.
//! It transforms the Abstract Syntax Tree (AST) into a typed Hierarchical
//! Intermediate Representation (HIR), validating the program's type safety
//! along the way.

pub use self::context::TypeContext;
pub use self::error::TypeError;

mod context;
mod error;
mod expressions;
mod items;
mod patterns;
mod statements;
mod types;

use crate::ast;
use crate::hir;
use crate::hir::Ty; // Import Ty for the substitutions map
use crate::token::Token;
use chumsky::span::SimpleSpan;
use std::collections::HashMap;

pub struct TypeChecker<'src> {
    context: TypeContext<'src>,
    errors: Vec<TypeError<'src>>,
    next_infer_var: u32,
    /// FIX: Added the missing `substitutions` field. This map stores the
    /// solutions for inference variables found during unification.
    substitutions: HashMap<u32, Ty<'src>>,
    /// Token spans for better error reporting
    token_spans: Vec<(Token<'src>, SimpleSpan)>,
    /// Import mappings: alias -> full path
    import_mappings: HashMap<&'src str, Vec<&'src str>>,
}

impl<'src> TypeChecker<'src> {
    pub fn new() -> Self {
        Self {
            context: TypeContext::new(),
            errors: Vec::new(),
            next_infer_var: 0,
            substitutions: HashMap::new(),
            token_spans: Vec::new(),
            import_mappings: HashMap::new(),
        }
    }

    pub fn with_token_spans(token_spans: Vec<(Token<'src>, SimpleSpan)>) -> Self {
        Self {
            context: TypeContext::new(),
            errors: Vec::new(),
            next_infer_var: 0,
            substitutions: HashMap::new(),
            token_spans,
            import_mappings: HashMap::new(),
        }
    }

    /// Resolves all inference variables in a function
    fn resolve_function_inference(&self, func: hir::Function) -> hir::Function {
        hir::Function {
            name: func.name,
            params: func.params,
            ret_type: self.resolve_type(&func.ret_type),
            body: self.resolve_expr_inference(func.body),
            is_public: func.is_public
        }
    }
    

    fn collect_definitions(&mut self, item: &ast::Item<'src>) -> Result<(), TypeError<'src>> {
        match item {
            ast::Item::Fn(func) => {
                self.context.add_function(func.clone());
            }
            ast::Item::ExternBlock {
                module_name,
                functions,
            } => {
                // Add each function from the extern block to the context
                for function in functions {
                    self.context
                        .add_extern_function(function.name, item.clone());
                }
            }
            ast::Item::Struct(struct_def) => {
                println!("DEBUG: Adding struct {} to context", struct_def.name);
                self.context.add_struct(struct_def.clone());
            }
            ast::Item::Enum(enum_def) => {
                self.context.add_enum(enum_def.clone());
            }
            ast::Item::Trait(trait_def) => {
                self.context.add_trait(trait_def.clone());
            }
            ast::Item::Impl(impl_block) => {
                println!(
                    "DEBUG: Processing impl block for target: {:?}",
                    impl_block.target_type
                );

                // Extract the target type name
                let (target_name, generics) =
                    if let ast::Type { path, generics } = &impl_block.target_type {
                        if path.len() == 1 {
                            (path[0], generics.clone())
                        } else {
                            return Err(TypeError::UnknownStruct(""));
                        }
                    } else {
                        return Err(TypeError::UnknownStruct(""));
                    };

                // Check if the struct exists
                if let Some(_struct_def) = self.context.get_struct(target_name) {
                    println!("DEBUG: Found struct {} for impl block", target_name);

                    // Process each method in the impl block
                    for method in &impl_block.methods {
                        // Create a copy of the method with the correct self parameter type
                        let mut method_with_self = method.clone();

                        // Fix the self parameter type if it exists
                        for (param_name, param_type) in &mut method_with_self.params {
                            if let Some(name) = param_name {
                                if name == &"self" || name == &"mut self" {
                                    println!(
                                        "DEBUG: Fixing self parameter type for method {}",
                                        method_with_self.name
                                    );
                                    // Set the self parameter type to the struct being implemented
                                    *param_type = ast::Type {
                                        path: vec![target_name],
                                        generics: generics.clone(),
                                    };
                                    println!("DEBUG: Set self parameter type to {:?}", *param_type);
                                }
                            }
                        }

                        // Add the corrected method to the context
                        self.context.add_function(method_with_self);
                    }
                } else {
                    println!("DEBUG: Struct {} not found for impl block", target_name);
                    return Err(TypeError::UnknownStruct(target_name));
                }
            }
            ast::Item::Effect(effect_def) => {
                self.context.add_effect(effect_def.clone());
            }
            ast::Item::Import { path, alias } => {
                // Process imports and build import mappings
                let alias_name = alias.unwrap_or_else(|| path.last().unwrap());
                self.import_mappings.insert(alias_name, path.clone());
            }
            _ => {}
        }
        Ok(())
    }

    /// Resolve a path using import mappings
    fn resolve_path(&self, path: &[&'src str]) -> Vec<&'src str> {
        if path.len() >= 2 {
            // Check if the first part is an imported alias
            if let Some(imported_path) = self.import_mappings.get(path[0]) {
                // Replace the alias with the full imported path
                let mut resolved_path = imported_path.clone();
                resolved_path.extend_from_slice(&path[1..]);
                return resolved_path;
            }
        }
        path.to_vec()
    }

    /// Resolve a module symbol (e.g., "Std::Fmt::println")
    fn resolve_module_symbol(&mut self, path: &[&'src str]) -> Option<ast::Type<'src>> {
        if path.len() < 3 {
            return None; // Need at least namespace::module::symbol
        }

        let namespace = path[0];
        let module = path[1];
        let symbol = path[2];

        // Create module path for caching
        let module_path = format!("{}::{}", namespace, module);

        // Load module symbols if not cached
        if self.context.get_module_symbols(&module_path).is_none() {
            if let Some(symbols) = self.load_module_symbols(namespace, module) {
                self.context
                    .add_module_symbols(module_path.clone(), symbols);
            }
        }

        // Check if we have cached symbols for this module
        if let Some(symbols) = self.context.get_module_symbols(&module_path) {
            if let Some(_signature) = symbols.get(symbol) {
                // For now, just return a simple type based on the symbol name
                // This avoids the lifetime issues with converting back to borrowed types
                return Some(ast::Type {
                    path: vec![symbol], // Use the symbol name as the type
                    generics: vec![],
                });
            }
        }

        None
    }

    /// Load public symbols from a module
    fn load_module_symbols(
        &mut self,
        namespace: &str,
        module: &str,
    ) -> Option<HashMap<String, crate::typechecker::context::SymbolSignature>> {
        // Determine the module path based on namespace
        let module_path = if namespace == "Self" {
            format!("./{}/", module.to_lowercase())
        } else {
            format!(
                "./modules/{}/{}/",
                namespace.to_lowercase(),
                module.to_lowercase()
            )
        };

        // Load all .bst files in the module directory
        let mut symbols = HashMap::new();

        if let Ok(entries) = std::fs::read_dir(&module_path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    if let Some(extension) = entry.path().extension() {
                        if extension == "bst" {
                            if let Ok(contents) = std::fs::read_to_string(entry.path()) {
                                // Parse the file and collect public symbols
                                if let Some(file_symbols) = self.parse_module_file(&contents) {
                                    symbols.extend(file_symbols);
                                }
                            }
                        }
                    }
                }
            }
        }

        if symbols.is_empty() {
            None
        } else {
            Some(symbols)
        }
    }

    /// Parse a module file and extract public symbols
    fn parse_module_file(
        &mut self,
        contents: &str,
    ) -> Option<HashMap<String, crate::typechecker::context::SymbolSignature>> {
        use crate::lexer::lexer;
        use crate::parser::file_parser;
        use chumsky::Parser;

        // Lex the file
        let (tokens, lex_errors) = lexer().parse(contents).into_output_errors();
        if !lex_errors.is_empty() {
            return None; // Skip files with lex errors
        }

        let tokens = tokens?;
        let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();

        // Parse the file
        let (ast, parse_errors) = file_parser().parse(&token_slice).into_output_errors();
        if !parse_errors.is_empty() {
            println!("DEBUG: Parse errors: {:?}", parse_errors);
            return None; // Skip files with parse errors
        }

        let ast = ast?;

        // Extract public symbols
        let mut symbols = HashMap::new();
        for item in &ast {
            match item {
                ast::Item::Fn(func) if func.is_public => {
                    let ret_type = func.ret_type.as_ref().map(|t| t.into()).unwrap_or_else(|| {
                        ast::OwnedType {
                            path: vec!["none".to_string()],
                            generics: vec![],
                        }
                    });
                    symbols.insert(
                        func.name.to_string(),
                        crate::typechecker::context::SymbolSignature {
                            name: func.name.to_string(),
                            kind: crate::typechecker::context::SymbolKind::Function(func.into()),
                            type_info: ret_type,
                        },
                    );
                }
                ast::Item::Struct(struct_def) if struct_def.is_public => {
                    let mut generics = Vec::new();
                    for generic in &struct_def.generics {
                        generics.push(ast::OwnedType {
                            path: vec![generic.to_string()],
                            generics: vec![],
                        });
                    }
                    symbols.insert(
                        struct_def.name.to_string(),
                        crate::typechecker::context::SymbolSignature {
                            name: struct_def.name.to_string(),
                            kind: crate::typechecker::context::SymbolKind::Struct(
                                struct_def.into(),
                            ),
                            type_info: ast::OwnedType {
                                path: vec![struct_def.name.to_string()],
                                generics,
                            },
                        },
                    );
                }
                ast::Item::Enum(enum_def) if enum_def.is_public => {
                    if let Some(name) = &enum_def.name {
                        symbols.insert(
                            name.to_string(),
                            crate::typechecker::context::SymbolSignature {
                                name: name.to_string(),
                                kind: crate::typechecker::context::SymbolKind::Enum(
                                    enum_def.into(),
                                ),
                                type_info: ast::OwnedType {
                                    path: vec![name.to_string()],
                                    generics: vec![],
                                },
                            },
                        );
                    }
                }
                ast::Item::Trait(trait_def) if trait_def.is_public => {
                    symbols.insert(
                        trait_def.name.to_string(),
                        crate::typechecker::context::SymbolSignature {
                            name: trait_def.name.to_string(),
                            kind: crate::typechecker::context::SymbolKind::Trait(trait_def.into()),
                            type_info: ast::OwnedType {
                                path: vec![trait_def.name.to_string()],
                                generics: vec![],
                            },
                        },
                    );
                }
                ast::Item::Effect(effect_def) if effect_def.is_public => {
                    symbols.insert(
                        effect_def.name.to_string(),
                        crate::typechecker::context::SymbolSignature {
                            name: effect_def.name.to_string(),
                            kind: crate::typechecker::context::SymbolKind::Effect(
                                effect_def.into(),
                            ),
                            type_info: ast::OwnedType {
                                path: vec![effect_def.name.to_string()],
                                generics: vec![],
                            },
                        },
                    );
                }
                ast::Item::Handler(handler_def) if handler_def.is_public => {
                    symbols.insert(
                        handler_def.name.to_string(),
                        crate::typechecker::context::SymbolSignature {
                            name: handler_def.name.to_string(),
                            kind: crate::typechecker::context::SymbolKind::Handler(
                                handler_def.into(),
                            ),
                            type_info: ast::OwnedType {
                                path: vec![handler_def.name.to_string()],
                                generics: vec![],
                            },
                        },
                    );
                }
                ast::Item::ExternBlock {
                    module_name,
                    functions,
                } => {
                    // Add each function from the extern block
                    for function in functions {
                        let ret_type = function.ret_type.as_ref().map_or(
                            ast::Type {
                                path: vec!["none"],
                                generics: vec![],
                            },
                            |t| t.clone(),
                        );
                        symbols.insert(
                            function.name.to_string(),
                            crate::typechecker::context::SymbolSignature {
                                name: function.name.to_string(),
                                kind: crate::typechecker::context::SymbolKind::ExternFunction(
                                    ast::OwnedType::from(&ret_type),
                                ),
                                type_info: ast::OwnedType::from(&ret_type),
                            },
                        );
                    }
                }
                _ => {} // Skip non-public items
            }
        }

        if symbols.is_empty() {
            None
        } else {
            Some(symbols)
        }
    }
}

impl<'src> Default for TypeChecker<'src> {
    fn default() -> Self {
        Self::new()
    }
}
