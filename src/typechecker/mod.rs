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
mod unification;

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
            // FIX: Initialize the new field.
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

    pub fn check_file(
        mut self,
        items: &[ast::Item<'src>],
    ) -> Result<Vec<hir::Item<'src>>, Vec<TypeError<'src>>> {
        // First pass: collect all definitions and process imports
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
            // Resolve all inference variables in the final HIR
            let resolved_items = hir_items
                .into_iter()
                .map(|item| self.resolve_item_inference(item))
                .collect();
            Ok(resolved_items)
        } else {
            Err(self.errors)
        }
    }

    /// Resolves all inference variables in an HIR item
    fn resolve_item_inference(&self, item: hir::Item<'src>) -> hir::Item<'src> {
        match item {
            hir::Item::Fn(func) => hir::Item::Fn(self.resolve_function_inference(func)),
            hir::Item::Stmt(stmt) => hir::Item::Stmt(self.resolve_stmt_inference(stmt)),
            hir::Item::Impl(impl_block) => hir::Item::Impl(self.resolve_impl_inference(impl_block)),
            _ => item, // Other items don't contain expressions that need resolution
        }
    }

    /// Resolves all inference variables in a function
    fn resolve_function_inference(&self, func: hir::Function<'src>) -> hir::Function<'src> {
        hir::Function {
            name: func.name,
            params: func.params,
            ret_type: self.resolve_type(&func.ret_type),
            body: self.resolve_expr_inference(func.body),
            is_public: func.is_public,
        }
    }

    /// Resolves all inference variables in a statement
    fn resolve_stmt_inference(&self, stmt: hir::Stmt<'src>) -> hir::Stmt<'src> {
        match stmt {
            hir::Stmt::Let {
                name,
                is_mut,
                value_ty,
                value,
            } => hir::Stmt::Let {
                name,
                is_mut,
                value_ty: self.resolve_type(&value_ty),
                value: self.resolve_expr_inference(value),
            },
            hir::Stmt::Return(expr) => {
                hir::Stmt::Return(expr.map(|e| self.resolve_expr_inference(e)))
            }
            hir::Stmt::Assign(lhs, rhs) => hir::Stmt::Assign(
                self.resolve_expr_inference(lhs),
                self.resolve_expr_inference(rhs),
            ),
            hir::Stmt::Expr(expr) => hir::Stmt::Expr(self.resolve_expr_inference(expr)),
        }
    }

    /// Resolves all inference variables in an implementation block
    fn resolve_impl_inference(&self, impl_block: hir::ImplBlock<'src>) -> hir::ImplBlock<'src> {
        hir::ImplBlock {
            trait_name: impl_block.trait_name,
            target_type: self.resolve_type(&impl_block.target_type),
            methods: impl_block
                .methods
                .into_iter()
                .map(|m| self.resolve_function_inference(m))
                .collect(),
        }
    }

    /// Resolves all inference variables in an expression
    fn resolve_expr_inference(&self, expr: hir::Expr<'src>) -> hir::Expr<'src> {
        let resolved_ty = self.resolve_type(&expr.ty);
        let resolved_kind = match expr.kind {
            hir::ExprKind::Literal(lit) => hir::ExprKind::Literal(lit),
            hir::ExprKind::Array(elements) => hir::ExprKind::Array(
                elements
                    .into_iter()
                    .map(|e| self.resolve_expr_inference(e))
                    .collect(),
            ),
            hir::ExprKind::Map(pairs) => hir::ExprKind::Map(
                pairs
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            self.resolve_expr_inference(k),
                            self.resolve_expr_inference(v),
                        )
                    })
                    .collect(),
            ),
            hir::ExprKind::Path(path) => hir::ExprKind::Path(path),
            hir::ExprKind::Unary { op, rhs } => hir::ExprKind::Unary {
                op,
                rhs: Box::new(self.resolve_expr_inference(*rhs)),
            },
            hir::ExprKind::Binary { op, lhs, rhs } => hir::ExprKind::Binary {
                op,
                lhs: Box::new(self.resolve_expr_inference(*lhs)),
                rhs: Box::new(self.resolve_expr_inference(*rhs)),
            },
            hir::ExprKind::Call { fun, args } => hir::ExprKind::Call {
                fun: Box::new(self.resolve_expr_inference(*fun)),
                args: args
                    .into_iter()
                    .map(|a| self.resolve_expr_inference(a))
                    .collect(),
            },
            hir::ExprKind::StructInit { path, fields } => hir::ExprKind::StructInit {
                path,
                fields: fields
                    .into_iter()
                    .map(|(k, v)| (k, self.resolve_expr_inference(v)))
                    .collect(),
            },
            hir::ExprKind::Block { stmts, last_expr } => hir::ExprKind::Block {
                stmts: stmts
                    .into_iter()
                    .map(|s| self.resolve_stmt_inference(s))
                    .collect(),
                last_expr: last_expr.map(|e| Box::new(self.resolve_expr_inference(*e))),
            },
            hir::ExprKind::If {
                cond,
                then_block,
                else_block,
            } => hir::ExprKind::If {
                cond: Box::new(self.resolve_expr_inference(*cond)),
                then_block: Box::new(self.resolve_expr_inference(*then_block)),
                else_block: else_block.map(|e| Box::new(self.resolve_expr_inference(*e))),
            },
            hir::ExprKind::Match { scrutinee, arms } => hir::ExprKind::Match {
                scrutinee: Box::new(self.resolve_expr_inference(*scrutinee)),
                arms: arms
                    .into_iter()
                    .map(|(pat, expr)| {
                        (
                            self.resolve_pattern_inference(pat),
                            self.resolve_expr_inference(expr),
                        )
                    })
                    .collect(),
            },
            hir::ExprKind::While { cond, body } => hir::ExprKind::While {
                cond: Box::new(self.resolve_expr_inference(*cond)),
                body: Box::new(self.resolve_expr_inference(*body)),
            },
            hir::ExprKind::Perform { path, args } => hir::ExprKind::Perform {
                path,
                args: args
                    .into_iter()
                    .map(|a| self.resolve_expr_inference(a))
                    .collect(),
            },
            hir::ExprKind::Handle { body, handler } => hir::ExprKind::Handle {
                body: Box::new(self.resolve_expr_inference(*body)),
                handler,
            },
        };
        hir::Expr {
            kind: resolved_kind,
            ty: resolved_ty,
        }
    }

    /// Resolves all inference variables in a pattern
    fn resolve_pattern_inference(&self, pattern: hir::Pattern<'src>) -> hir::Pattern<'src> {
        hir::Pattern {
            kind: pattern.kind,
            ty: self.resolve_type(&pattern.ty),
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

    /// Find the span for a given token or expression
    fn find_span_for_token(&self, token: &Token<'src>) -> Option<SimpleSpan> {
        self.token_spans
            .iter()
            .find(|(t, _)| std::mem::discriminant(t) == std::mem::discriminant(token))
            .map(|(_, span)| *span)
    }

    /// Find a reasonable span for error reporting
    fn get_error_span(&self) -> Option<SimpleSpan> {
        // Use the first token span if available
        self.token_spans.first().map(|(_, span)| *span)
    }

    fn collect_definitions(&mut self, item: &ast::Item<'src>) -> Result<(), TypeError<'src>> {
        match item {
            ast::Item::Fn(func) => {
                self.context.add_function(func.clone());
            }
            ast::Item::ExternBlock { module_name, functions } => {
                // Add each function from the extern block to the context
                for function in functions {
                    self.context.add_extern_function(function.name, item.clone());
                }
            }
            ast::Item::Struct(struct_def) => {
                self.context.add_struct(struct_def.clone());
            }
            ast::Item::Enum(enum_def) => {
                self.context.add_enum(enum_def.clone());
            }
            ast::Item::Trait(trait_def) => {
                self.context.add_trait(trait_def.clone());
            }
            ast::Item::Impl(impl_block) => {
                // Add each method from the impl block to the context
                for method in &impl_block.methods {
                    self.context.add_trait_method(
                        method.name,
                        ast::TraitMethod {
                            name: method.name,
                            params: method.params.clone(),
                            ret_type: method.ret_type.clone(),
                            is_public: method.is_public,
                        },
                    );
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
                ast::Item::ExternBlock { module_name, functions } => {
                    // Add each function from the extern block
                    for function in functions {
                        let ret_type = function.ret_type.as_ref().map_or(
                            ast::Type { path: vec!["none"], generics: vec![] },
                            |t| t.clone()
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

    /// Suggest an import for an unknown symbol
    fn suggest_import(&self, symbol: &str) -> Option<String> {
        // Common standard library modules that might contain the symbol
        let common_modules = [
            ("Std::Fmt", vec!["println", "print", "format"]),
            ("Std::String", vec!["from", "new", "len", "is_empty"]),
            ("Std::Collections", vec!["Vec", "Map", "Set"]),
            ("Std::Math", vec!["add", "sub", "mul", "div", "sqrt", "pow"]),
            ("Self::Utils", vec!["helper_function", "utility"]),
        ];

        for (module_path, symbols) in common_modules.iter() {
            if symbols.contains(&symbol) {
                return Some(format!("import {};", module_path));
            }
        }

        // If it looks like a standard library module name, suggest importing it
        if symbol == "Std" {
            return Some("import Std::Fmt;".to_string());
        }

        if symbol == "Self" {
            return Some("import Self::Utils;".to_string());
        }

        None
    }
}

impl<'src> Default for TypeChecker<'src> {
    fn default() -> Self {
        Self::new()
    }
}
