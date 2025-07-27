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
    fn to_static_str(s: &str) -> &'static str {
        Box::leak(Box::new(s.to_string()))
    }

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
        // First pass: collect all definitions
        for item in items {
            match item {
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
                    println!("DEBUG: Processing impl block for target: {:?}", impl_block.target_type);
                    
                    // Extract the target type name
                    let (target_name, generics) = if let ast::Type { path, generics } = &impl_block.target_type {
                        if path.len() == 1 {
                            (path[0], generics.clone())
                        } else {
                            self.errors.push(TypeError::UnknownStruct(""));
                            continue;
                        }
                    } else {
                        self.errors.push(TypeError::UnknownStruct(""));
                        continue;
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
                                        println!("DEBUG: Fixing self parameter type for method {}", method_with_self.name);
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
                        self.errors.push(TypeError::UnknownStruct(target_name));
                    }
                },
                ast::Item::Fn(func) => {
                    // Only add regular functions (not methods from impl blocks)
                    // Methods are added during impl block processing
                    if !func.params.iter().any(|(name, _)| name.as_deref() == Some("self") || name.as_deref() == Some("mut self")) {
                        self.context.add_function(func.clone());
                    }
                }
                ast::Item::ExternBlock { module_name, functions } => {
                    for function in functions {
                        self.context.add_extern_function(function.name, ast::Item::ExternBlock {
                            module_name,
                            functions: vec![function.clone()],
                        });
                    }
                }
                ast::Item::Effect(effect_def) => {
                    self.context.add_effect(effect_def.clone());
                }
                ast::Item::Handler(_handler_def) => {
                    // For now, just add the handler name to the context
                    // In a full implementation, we'd process the handler functions
                }
                _ => {
                    // Other items don't need to be collected in the first pass
                }
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
            hir::ExprKind::EnumVariant { enum_name, variant_name } => hir::ExprKind::EnumVariant {
                enum_name,
                variant_name,
            },
            hir::ExprKind::ModulePath { module, symbol } => hir::ExprKind::ModulePath {
                module,
                symbol,
            },
            hir::ExprKind::FieldAccess { receiver, field } => hir::ExprKind::FieldAccess {
                receiver: Box::new(self.resolve_expr_inference(*receiver)),
                field: field,
            },
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
                println!("DEBUG: Processing impl block for target: {:?}", impl_block.target_type);
                
                // Extract the target type name
                let (target_name, generics) = if let ast::Type { path, generics } = &impl_block.target_type {
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
                                    println!("DEBUG: Fixing self parameter type for method {}", method_with_self.name);
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
            },
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

    /// Convert a static item to a borrowed item for processing
    fn convert_static_item_to_borrowed(&self, item: &ast::Item<'static>) -> ast::Item<'src> {
        // This is a temporary conversion - in practice, we'd need to handle lifetimes properly
        // For now, we'll use a simple approach by converting to owned and back
        match item {
            ast::Item::Stmt(stmt) => ast::Item::Stmt(self.convert_static_stmt_to_borrowed(stmt)),
            ast::Item::Import { path, alias } => ast::Item::Import {
                path: path.iter().map(|s| *s).collect(),
                alias: alias.as_ref().map(|s| *s),
            },
            ast::Item::ExternBlock { module_name, functions } => ast::Item::ExternBlock {
                module_name,
                functions: functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
            },
            ast::Item::Fn(func) => ast::Item::Fn(self.convert_static_function_to_borrowed(func)),
            ast::Item::Struct(struct_def) => ast::Item::Struct(self.convert_static_struct_to_borrowed(struct_def)),
            ast::Item::Enum(enum_def) => ast::Item::Enum(self.convert_static_enum_to_borrowed(enum_def)),
            ast::Item::Trait(trait_def) => ast::Item::Trait(self.convert_static_trait_to_borrowed(trait_def)),
            ast::Item::Impl(impl_block) => ast::Item::Impl(self.convert_static_impl_to_borrowed(impl_block)),
            ast::Item::Effect(effect_def) => ast::Item::Effect(self.convert_static_effect_to_borrowed(effect_def)),
            ast::Item::Handler(handler_def) => ast::Item::Handler(self.convert_static_handler_to_borrowed(handler_def)),
        }
    }

    /// Convert a static HIR item to a static HIR item (identity conversion for now)
    fn convert_hir_item_to_static(&self, item: &hir::Item<'src>) -> hir::Item<'static> {
        // This is a placeholder - in a real implementation, we'd need proper lifetime conversion
        // For now, we'll use a simple approach
        match item {
            hir::Item::Stmt(stmt) => hir::Item::Stmt(self.convert_hir_stmt_to_static(stmt)),
            hir::Item::Import { path, alias } => hir::Item::Import {
                path: path.iter().map(|s| Self::to_static_str(s)).collect(),
                alias: alias.as_ref().map(|s| Self::to_static_str(s)),
            },
            hir::Item::ExternFn { name, params, ret_type } => hir::Item::ExternFn {
                name: Self::to_static_str(name),
                params: params.iter().map(|(name, ty)| (
                    name.as_ref().map(|s| Self::to_static_str(s)),
                    self.convert_hir_type_to_static(ty)
                )).collect(),
                ret_type: self.convert_hir_type_to_static(ret_type),
            },
            hir::Item::ExternBlock { module_name, functions } => hir::Item::ExternBlock {
                module_name: Self::to_static_str(module_name),
                functions: functions.iter().map(|f| self.convert_ast_function_to_static(f)).collect(),
            },
            hir::Item::Fn(func) => hir::Item::Fn(self.convert_hir_function_to_static(func)),
            hir::Item::Struct(struct_def) => hir::Item::Struct(self.convert_hir_struct_to_static(struct_def)),
            hir::Item::Enum(enum_def) => hir::Item::Enum(self.convert_hir_enum_to_static(enum_def)),
            hir::Item::Trait(trait_def) => hir::Item::Trait(self.convert_hir_trait_to_static(trait_def)),
            hir::Item::Impl(impl_block) => hir::Item::Impl(self.convert_hir_impl_to_static(impl_block)),
            hir::Item::Effect(effect_def) => hir::Item::Effect(self.convert_hir_effect_to_static(effect_def)),
            hir::Item::Handler(handler_def) => hir::Item::Handler(self.convert_hir_handler_to_static(handler_def)),
        }
    }

    /// Convert a static error to a static error (identity conversion for now)
    fn convert_error_to_static(&self, error: &TypeError<'src>) -> TypeError<'static> {
        // This is a placeholder - in a real implementation, we'd need proper lifetime conversion
        match error {
            TypeError::MismatchedTypes { expected, found } => TypeError::MismatchedTypes {
                expected: self.convert_hir_type_to_static(expected),
                found: self.convert_hir_type_to_static(found),
            },
            TypeError::UnknownVariable(name) => TypeError::UnknownVariable(name.to_string().leak()),
            TypeError::UnknownFunction(name) => TypeError::UnknownFunction(name.to_string().leak()),
            TypeError::UnknownStruct(name) => TypeError::UnknownStruct(name.to_string().leak()),
            TypeError::UnknownEnum(name) => TypeError::UnknownEnum(name.to_string().leak()),
            TypeError::UnknownEnumVariant { enum_name, variant_name } => TypeError::UnknownEnumVariant {
                enum_name: enum_name.to_string().leak(),
                variant_name: variant_name.to_string().leak(),
            },
            TypeError::WrongArgumentCount { expected, found } => TypeError::WrongArgumentCount {
                expected: *expected,
                found: *found,
            },
            TypeError::WrongNumberOfArguments { expected, found } => TypeError::WrongNumberOfArguments {
                expected: *expected,
                found: *found,
            },
            TypeError::WrongArgumentType { expected, found } => TypeError::WrongArgumentType {
                expected: self.convert_hir_type_to_static(expected),
                found: self.convert_hir_type_to_static(found),
            },
            TypeError::UnknownStructField { struct_name, field_name } => TypeError::UnknownStructField {
                struct_name: struct_name.to_string().leak(),
                field_name: field_name.to_string().leak(),
            },
            TypeError::MissingStructField { struct_name, field_name } => TypeError::MissingStructField {
                struct_name: struct_name.to_string().leak(),
                field_name: field_name.to_string().leak(),
            },
            TypeError::InvalidOperator { op, ty } => TypeError::InvalidOperator {
                op: op.clone(),
                ty: self.convert_hir_type_to_static(ty),
            },
            TypeError::InvalidPattern { pattern } => TypeError::InvalidPattern {
                pattern: pattern.clone(),
            },
            TypeError::UnificationError(ty1, ty2) => TypeError::UnificationError(
                self.convert_hir_type_to_static(ty1),
                self.convert_hir_type_to_static(ty2),
            ),
            TypeError::UnknownModule { namespace, module } => TypeError::UnknownModule {
                namespace: namespace.to_string().leak(),
                module: module.to_string().leak(),
            },
            TypeError::UnknownModuleSymbol { namespace, module, symbol } => TypeError::UnknownModuleSymbol {
                namespace: namespace.to_string().leak(),
                module: module.to_string().leak(),
                symbol: symbol.to_string().leak(),
            },
            TypeError::MissingImport { symbol, suggested_import } => TypeError::MissingImport {
                symbol: symbol.to_string().leak(),
                suggested_import: suggested_import.clone(),
            },
            TypeError::LiteralOverflow { value, target_type } => TypeError::LiteralOverflow {
                value: *value,
                target_type: target_type.clone(),
            },
        }
    }

    // Helper conversion methods for static to borrowed
    fn convert_static_stmt_to_borrowed(&self, stmt: &ast::Stmt<'static>) -> ast::Stmt<'src> {
        match stmt {
            ast::Stmt::Let { is_mut, name, ty, value } => ast::Stmt::Let {
                is_mut: *is_mut,
                name,
                ty: ty.as_ref().map(|t| self.convert_static_type_to_borrowed(t)),
                value: self.convert_static_expr_to_borrowed(value),
            },
            ast::Stmt::Return(expr) => ast::Stmt::Return(
                expr.as_ref().map(|e| self.convert_static_expr_to_borrowed(e))
            ),
            ast::Stmt::Assign(lhs, rhs) => ast::Stmt::Assign(
                self.convert_static_expr_to_borrowed(lhs),
                self.convert_static_expr_to_borrowed(rhs),
            ),
            ast::Stmt::Expr(expr) => ast::Stmt::Expr(
                self.convert_static_expr_to_borrowed(expr)
            ),
            ast::Stmt::Error => ast::Stmt::Error,
        }
    }

    fn convert_static_expr_to_borrowed(&self, expr: &ast::Expr<'static>) -> ast::Expr<'src> {
        match expr {
            ast::Expr::Literal(lit) => ast::Expr::Literal(self.convert_static_literal_to_borrowed(lit)),
            ast::Expr::Array(items) => ast::Expr::Array(
                items.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect()
            ),
            ast::Expr::Map(items) => ast::Expr::Map(
                items.iter().map(|(k, v)| (
                    self.convert_static_expr_to_borrowed(k),
                    self.convert_static_expr_to_borrowed(v)
                )).collect()
            ),
            ast::Expr::Path(path) => ast::Expr::Path(path.clone()),
            ast::Expr::FieldAccess { receiver, field } => ast::Expr::FieldAccess {
                receiver: Box::new(self.convert_static_expr_to_borrowed(receiver)),
                field,
            },
            ast::Expr::Unary { op, rhs } => ast::Expr::Unary {
                op: op.clone(),
                rhs: Box::new(self.convert_static_expr_to_borrowed(rhs)),
            },
            ast::Expr::Binary { op, lhs, rhs } => ast::Expr::Binary {
                op: op.clone(),
                lhs: Box::new(self.convert_static_expr_to_borrowed(lhs)),
                rhs: Box::new(self.convert_static_expr_to_borrowed(rhs)),
            },
            ast::Expr::Call { fun, args } => ast::Expr::Call {
                fun: Box::new(self.convert_static_expr_to_borrowed(fun)),
                args: args.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect(),
            },
            ast::Expr::StructInit { path, generics, fields } => ast::Expr::StructInit {
                path: path.clone(),
                generics: generics.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
                fields: fields.iter().map(|(name, expr)| (
                    *name,
                    self.convert_static_expr_to_borrowed(expr)
                )).collect(),
            },
            ast::Expr::Block { stmts, last_expr } => ast::Expr::Block {
                stmts: stmts.iter().map(|s| self.convert_static_stmt_to_borrowed(s)).collect(),
                last_expr: last_expr.as_ref().map(|e| Box::new(self.convert_static_expr_to_borrowed(e))),
            },
            ast::Expr::If { cond, then_block, else_block } => ast::Expr::If {
                cond: Box::new(self.convert_static_expr_to_borrowed(cond)),
                then_block: Box::new(self.convert_static_expr_to_borrowed(then_block)),
                else_block: else_block.as_ref().map(|e| Box::new(self.convert_static_expr_to_borrowed(e))),
            },
            ast::Expr::Match { scrutinee, arms } => ast::Expr::Match {
                scrutinee: Box::new(self.convert_static_expr_to_borrowed(scrutinee)),
                arms: arms.iter().map(|(pat, expr)| (
                    self.convert_static_pattern_to_borrowed(pat),
                    self.convert_static_expr_to_borrowed(expr)
                )).collect(),
            },
            ast::Expr::While { cond, body } => ast::Expr::While {
                cond: Box::new(self.convert_static_expr_to_borrowed(cond)),
                body: Box::new(self.convert_static_expr_to_borrowed(body)),
            },
            ast::Expr::Perform { path, args } => ast::Expr::Perform {
                path: path.clone(),
                args: args.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect(),
            },
            ast::Expr::Handle { body, handler } => ast::Expr::Handle {
                body: Box::new(self.convert_static_expr_to_borrowed(body)),
                handler: self.convert_static_handler_body_to_borrowed(handler),
            },
            ast::Expr::Error => ast::Expr::Error,
        }
    }

    fn convert_static_function_to_borrowed(&self, func: &ast::Function<'static>) -> ast::Function<'src> {
        ast::Function {
            name: func.name,
            generics: func.generics.clone(),
            params: func.params.iter().map(|(name, ty)| (
                name.map(|s| s),
                self.convert_static_type_to_borrowed(ty)
            )).collect(),
            ret_type: func.ret_type.as_ref().map(|t| self.convert_static_type_to_borrowed(t)),
            effects: func.effects.clone(),
            body: self.convert_static_expr_to_borrowed(&func.body),
            is_public: func.is_public,
        }
    }

    fn convert_static_struct_to_borrowed(&self, struct_def: &ast::StructDef<'static>) -> ast::StructDef<'src> {
        ast::StructDef {
            name: struct_def.name,
            generics: struct_def.generics.clone(),
                          fields: struct_def.fields.iter().map(|(name, ty)| (
                *name,
                self.convert_static_type_to_borrowed(ty)
            )).collect(),
            is_public: struct_def.is_public,
        }
    }

    fn convert_static_enum_to_borrowed(&self, enum_def: &ast::EnumDef<'static>) -> ast::EnumDef<'src> {
        ast::EnumDef {
            name: enum_def.name.map(|s| s),
            generics: enum_def.generics.clone(),
                          variants: enum_def.variants.iter().map(|(name, types)| (
                *name,
                types.as_ref().map(|ts| ts.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect())
            )).collect(),
            is_public: enum_def.is_public,
        }
    }

    fn convert_static_trait_to_borrowed(&self, trait_def: &ast::TraitDef<'static>) -> ast::TraitDef<'src> {
        ast::TraitDef {
            name: trait_def.name,
            methods: trait_def.methods.iter().map(|m| ast::TraitMethod {
                name: m.name,
                params: m.params.iter().map(|(name, ty)| (
                    name.map(|s| s),
                    self.convert_static_type_to_borrowed(ty)
                )).collect(),
                ret_type: m.ret_type.as_ref().map(|t| self.convert_static_type_to_borrowed(t)),
                is_public: m.is_public,
            }).collect(),
            is_public: trait_def.is_public,
        }
    }

    fn convert_static_impl_to_borrowed(&self, impl_block: &ast::ImplBlock<'static>) -> ast::ImplBlock<'src> {
        ast::ImplBlock {
            trait_name: impl_block.trait_name,
            target_type: self.convert_static_type_to_borrowed(&impl_block.target_type),
            methods: impl_block.methods.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
        }
    }

    fn convert_static_effect_to_borrowed(&self, effect_def: &ast::EffectDef<'static>) -> ast::EffectDef<'src> {
        ast::EffectDef {
            name: effect_def.name,
            operations: effect_def.operations.iter().map(|op| ast::EffectOp {
                name: op.name,
                params: op.params.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
                ret_type: self.convert_static_type_to_borrowed(&op.ret_type),
                is_public: op.is_public,
            }).collect(),
            is_public: effect_def.is_public,
        }
    }

    fn convert_static_handler_to_borrowed(&self, handler_def: &ast::HandlerDef<'static>) -> ast::HandlerDef<'src> {
        ast::HandlerDef {
            name: handler_def.name,
            effects: handler_def.effects.clone(),
            functions: handler_def.functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
            is_public: handler_def.is_public,
        }
    }

    fn convert_static_type_to_borrowed(&self, ty: &ast::Type<'static>) -> ast::Type<'src> {
        ast::Type {
            path: ty.path.clone(),
            generics: ty.generics.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
        }
    }

    fn convert_static_pattern_to_borrowed(&self, pat: &ast::Pattern<'static>) -> ast::Pattern<'src> {
        match pat {
            ast::Pattern::Literal(lit) => ast::Pattern::Literal(self.convert_static_literal_to_borrowed(lit)),
            ast::Pattern::Identifier(name) => ast::Pattern::Identifier(name),
            ast::Pattern::Path { path, args } => ast::Pattern::Path {
                path: path.clone(),
                args: args.iter().map(|p| self.convert_static_pattern_to_borrowed(p)).collect(),
            },
            ast::Pattern::Wildcard => ast::Pattern::Wildcard,
        }
    }

    fn convert_static_literal_to_borrowed(&self, lit: &ast::Literal<'static>) -> ast::Literal<'src> {
        match lit {
            ast::Literal::Bool(b) => ast::Literal::Bool(*b),
            ast::Literal::I32(i) => ast::Literal::I32(*i),
            ast::Literal::I64(i) => ast::Literal::I64(*i),
            ast::Literal::F64(f) => ast::Literal::F64(*f),
            ast::Literal::Str(s) => ast::Literal::Str(s),
            ast::Literal::Unit => ast::Literal::Unit,
        }
    }

    fn convert_static_handler_body_to_borrowed(&self, body: &ast::HandlerBody<'static>) -> ast::HandlerBody<'src> {
        match body {
            ast::HandlerBody::Path(path) => ast::HandlerBody::Path(path.clone()),
            ast::HandlerBody::Inline(functions) => ast::HandlerBody::Inline(
                functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect()
            ),
        }
    }

    // Helper conversion methods for HIR to static
    fn convert_hir_function_to_static(&self, func: &hir::Function<'src>) -> hir::Function<'static> {
        // Placeholder implementation - would need proper conversion
        hir::Function {
            name: Self::to_static_str(func.name),
            params: func.params.iter().map(|&(ref name, ref ty)| (
                name.map(|s| Self::to_static_str(s)),
                self.convert_hir_type_to_static(ty)
            )).collect(),
            ret_type: self.convert_hir_type_to_static(&func.ret_type),
            body: self.convert_hir_expr_to_static(&func.body),
            is_public: func.is_public,
        }
    }

    fn convert_hir_struct_to_static(&self, struct_def: &hir::StructDef<'src>) -> hir::StructDef<'static> {
        hir::StructDef {
            name: Self::to_static_str(struct_def.name),
            generics: struct_def.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            fields: struct_def.fields.iter().map(|(name, ty)| (
                Self::to_static_str(name),
                self.convert_hir_type_to_static(ty)
            )).collect(),
            is_public: struct_def.is_public,
        }
    }

    fn convert_hir_enum_to_static(&self, enum_def: &hir::EnumDef<'src>) -> hir::EnumDef<'static> {
        hir::EnumDef {
            name: enum_def.name.as_deref().map(|s| Self::to_static_str(s)),
            generics: enum_def.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            variants: enum_def.variants.iter().map(|(name, types)| (
                Self::to_static_str(name),
                types.as_ref().map(|ts| ts.iter().map(|t| self.convert_hir_type_to_static(t)).collect())
            )).collect(),
            is_public: enum_def.is_public,
        }
    }

    fn convert_hir_trait_to_static(&self, trait_def: &hir::TraitDef<'src>) -> hir::TraitDef<'static> {
        hir::TraitDef {
            name: Self::to_static_str(trait_def.name),
            methods: trait_def.methods.iter().map(|m| hir::TraitMethod {
                name: Self::to_static_str(m.name),
                params: m.params.iter().map(|(name, ty)| (
                    name.map(|s| Self::to_static_str(s)),
                    self.convert_hir_type_to_static(ty)
                )).collect(),
                ret_type: self.convert_hir_type_to_static(&m.ret_type),
                is_public: m.is_public,
            }).collect(),
            is_public: trait_def.is_public,
        }
    }

    fn convert_hir_impl_to_static(&self, impl_block: &hir::ImplBlock<'src>) -> hir::ImplBlock<'static> {
        hir::ImplBlock {
            trait_name: impl_block.trait_name.to_string().leak(),
            target_type: self.convert_hir_type_to_static(&impl_block.target_type),
            methods: impl_block.methods.iter().map(|f| self.convert_hir_function_to_static(f)).collect(),
        }
    }

    fn convert_hir_effect_to_static(&self, effect_def: &hir::EffectDef<'src>) -> hir::EffectDef<'static> {
        hir::EffectDef {
            name: effect_def.name.to_string().leak(),
            operations: effect_def.operations.iter().map(|op| hir::EffectOp {
                name: op.name.to_string().leak(),
                params: op.params.iter().map(|t| self.convert_hir_type_to_static(t)).collect(),
                ret_type: self.convert_hir_type_to_static(&op.ret_type),
                is_public: op.is_public,
            }).collect(),
            is_public: effect_def.is_public,
        }
    }

    fn convert_hir_handler_to_static(&self, handler_def: &hir::HandlerDef<'src>) -> hir::HandlerDef<'static> {
        hir::HandlerDef {
            name: handler_def.name.to_string().leak(),
            effects: handler_def.effects.iter().map(|s| Self::to_static_str(s)).collect(),
            functions: handler_def.functions.iter().map(|f| self.convert_hir_function_to_static(f)).collect(),
            is_public: handler_def.is_public,
        }
    }

    fn convert_hir_type_to_static(&self, ty: &hir::Ty<'src>) -> hir::Ty<'static> {
        match ty {
            hir::Ty::Infer(id) => hir::Ty::Infer(*id),
            hir::Ty::Unit => hir::Ty::Unit,
            hir::Ty::Bool => hir::Ty::Bool,
            hir::Ty::I8 => hir::Ty::I8,
            hir::Ty::I16 => hir::Ty::I16,
            hir::Ty::I32 => hir::Ty::I32,
            hir::Ty::I64 => hir::Ty::I64,
            hir::Ty::U8 => hir::Ty::U8,
            hir::Ty::U16 => hir::Ty::U16,
            hir::Ty::U32 => hir::Ty::U32,
            hir::Ty::U64 => hir::Ty::U64,
            hir::Ty::F32 => hir::Ty::F32,
            hir::Ty::F64 => hir::Ty::F64,
            hir::Ty::Str => hir::Ty::Str,
            hir::Ty::Adt { name, generics } => hir::Ty::Adt {
                name: name.iter().map(|s| Self::to_static_str(s)).collect(),
                generics: generics.iter().map(|t| self.convert_hir_type_to_static(t)).collect(),
            },
            hir::Ty::Array(inner) => hir::Ty::Array(Box::new(self.convert_hir_type_to_static(inner))),
            hir::Ty::Map { key, value } => hir::Ty::Map {
                key: Box::new(self.convert_hir_type_to_static(key)),
                value: Box::new(self.convert_hir_type_to_static(value)),
            },
            hir::Ty::Function { param_types, ret_type } => hir::Ty::Function {
                param_types: param_types.iter().map(|t| self.convert_hir_type_to_static(t)).collect(),
                ret_type: Box::new(self.convert_hir_type_to_static(ret_type)),
            },
            hir::Ty::Error => hir::Ty::Error,
        }
    }

    fn convert_ast_function_to_static(&self, func: &ast::Function<'src>) -> ast::Function<'static> {
        ast::Function {
            name: Self::to_static_str(func.name),
            generics: func.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            params: func.params.iter().map(|(name, ty)| (
                name.map(|s| Self::to_static_str(s)),
                self.convert_type_to_static(ty)
            )).collect(),
            ret_type: func.ret_type.as_ref().map(|t| self.convert_type_to_static(t)),
            effects: func.effects.iter().map(|s| Self::to_static_str(s)).collect(),
            body: self.convert_expr_to_static(&func.body),
            is_public: func.is_public,
        }
    }

    fn convert_type_to_static(&self, ty: &ast::Type<'src>) -> ast::Type<'static> {
        ast::Type {
            path: ty.path.iter().map(|s| Self::to_static_str(s)).collect(),
            generics: ty.generics.iter().map(|t| self.convert_type_to_static(t)).collect(),
        }
    }

    fn convert_expr_to_static(&self, expr: &ast::Expr<'src>) -> ast::Expr<'static> {
        // Placeholder implementation - would need proper conversion
        // For now, create a simple unit expression
        ast::Expr::Literal(ast::Literal::Unit)
    }

    fn convert_hir_expr_to_static(&self, expr: &hir::Expr<'src>) -> hir::Expr<'static> {
        // Placeholder implementation - would need proper conversion
        // For now, create a simple unit expression
        hir::Expr {
            kind: hir::ExprKind::Literal(ast::Literal::Unit),
            ty: hir::Ty::Unit,
        }
    }

    fn convert_hir_stmt_to_static(&self, stmt: &hir::Stmt<'src>) -> hir::Stmt<'static> {
        match stmt {
            hir::Stmt::Let { name, is_mut, value_ty, value } => hir::Stmt::Let {
                name: name.to_string().leak(),
                is_mut: *is_mut,
                value_ty: self.convert_hir_type_to_static(value_ty),
                value: self.convert_hir_expr_to_static(value),
            },
            hir::Stmt::Return(expr) => hir::Stmt::Return(
                expr.as_ref().map(|e| self.convert_hir_expr_to_static(e))
            ),
            hir::Stmt::Assign(lhs, rhs) => hir::Stmt::Assign(
                self.convert_hir_expr_to_static(lhs),
                self.convert_hir_expr_to_static(rhs)
            ),
            hir::Stmt::Expr(expr) => hir::Stmt::Expr(self.convert_hir_expr_to_static(expr)),
        }
    }
}

impl<'src> Default for TypeChecker<'src> {
    fn default() -> Self {
        Self::new()
    }
}
