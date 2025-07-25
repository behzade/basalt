//! project.rs
//!
//! This module handles loading and parsing all files in a project,
//! including recursively discovering imported modules.

use crate::ast::Item;
use crate::lexer::lexer;
use crate::parser::file_parser;
use chumsky::Parser;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::fs;

/// Represents a loaded project with all its parsed AST items
pub struct Project {
    /// All AST items from all files in the project
    pub items: Vec<Item<'static>>,
    /// Mapping from file paths to their AST items for debugging
    pub file_items: HashMap<PathBuf, Vec<Item<'static>>>,
    /// Source strings for lifetime management
    pub sources: Vec<String>,
}

/// Loader that discovers and parses all files in a project
pub struct ProjectLoader {
    /// Queue of file paths to visit and parse
    work_queue: VecDeque<PathBuf>,
    /// Set of canonical paths already visited to prevent duplicates and cycles
    visited: HashSet<PathBuf>,
    /// Collection of all parsed AST items from all files
    all_items: Vec<Item<'static>>,
    /// Mapping from file paths to their AST items
    file_items: HashMap<PathBuf, Vec<Item<'static>>>,
    /// Source strings for lifetime management
    sources: Vec<String>,
}

impl ProjectLoader {
    fn to_static_str(s: &str) -> &'static str {
        Box::leak(Box::new(s.to_string()))
    }

    /// Helper function to convert a static string to a borrowed string
    fn static_to_borrowed_str(s: &'static str) -> &str {
        s
    }

    /// Helper function to convert a static path to a borrowed path
    fn static_to_borrowed_path<'a>(path: &[&'static str]) -> Vec<&'a str> {
        path.iter().map(|s| *s).collect()
    }

    /// Helper function to convert a static option string to a borrowed option string
    fn static_to_borrowed_option_str(s: Option<&'static str>) -> Option<&str> {
        s
    }

    // Remove the problematic helper function - we'll use Owned* types instead

    /// Load a complete project starting from the given entry point
    pub fn load(entry_path: &str) -> Result<Project, String> {
        let mut loader = Self {
            work_queue: VecDeque::new(),
            visited: HashSet::new(),
            all_items: Vec::new(),
            file_items: HashMap::new(),
            sources: Vec::new(),
        };

        // Add the entry point to the queue
        loader.add_to_queue(Path::new(entry_path))?;

        // Process all files in the queue
        while let Some(path) = loader.work_queue.pop_front() {
            loader.process_file(&path)?;
        }

        Ok(Project {
            items: loader.all_items,
            file_items: loader.file_items,
            sources: loader.sources,
        })
    }

    /// Process a single file: parse it and discover its imports
    fn process_file(&mut self, path: &Path) -> Result<(), String> {
        println!("Processing file: {:?}", path);

        // Read the file contents
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file {:?}: {}", path, e))?;

        // Lex the file
        let (tokens, lex_errors) = lexer().parse(&contents).into_output_errors();
        if !lex_errors.is_empty() {
            return Err(format!(
                "Lexing errors in file {:?}: {:?}",
                path, lex_errors
            ));
        }

        let tokens = tokens.ok_or_else(|| {
            format!("Failed to lex file {:?}", path)
        })?;

        let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();

        // Parse the file
        let (ast, parse_errors) = file_parser().parse(&token_slice).into_output_errors();
        if !parse_errors.is_empty() {
            return Err(format!(
                "Parsing errors in file {:?}: {:?}",
                path, parse_errors
            ));
        }

        let ast = ast.ok_or_else(|| {
            format!("Failed to parse file {:?}", path)
        })?;

        // Convert AST items to owned versions to avoid lifetime issues
        let owned_items: Vec<Item<'static>> = ast
            .iter()
            .map(|item| self.convert_item_to_owned(item))
            .collect();

        // Store the items for this file
        self.file_items.insert(path.to_path_buf(), owned_items.clone());

        // Add all items to the global collection
        self.all_items.extend(owned_items.clone());

        // Discover imports and add them to the queue
        for item in &owned_items {
            if let Item::Import { path: import_path, .. } = item {
                self.discover_import_path(import_path, path)?;
            }
        }

        Ok(())
    }

    /// Convert an AST item to an owned version to avoid lifetime issues
    fn convert_item_to_owned(&self, item: &Item) -> Item<'static> {
        match item {
            Item::Stmt(stmt) => Item::Stmt(self.convert_stmt_to_owned(stmt)),
            Item::Import { path, alias } => Item::Import {
                path: path.iter().map(|s| Self::to_static_str(s)).collect(),
                alias: alias.as_ref().map(|s| Self::to_static_str(s)),
            },
            Item::ExternBlock { module_name, functions } => Item::ExternBlock {
                module_name: Self::to_static_str(module_name),
                functions: functions.iter().map(|f| self.convert_function_to_owned(f)).collect(),
            },
            Item::Fn(func) => Item::Fn(self.convert_function_to_owned(func)),
            Item::Struct(struct_def) => Item::Struct(self.convert_struct_to_owned(struct_def)),
            Item::Enum(enum_def) => Item::Enum(self.convert_enum_to_owned(enum_def)),
            Item::Trait(trait_def) => Item::Trait(self.convert_trait_to_owned(trait_def)),
            Item::Impl(impl_block) => Item::Impl(self.convert_impl_to_owned(impl_block)),
            Item::Effect(effect_def) => Item::Effect(self.convert_effect_to_owned(effect_def)),
            Item::Handler(handler_def) => Item::Handler(self.convert_handler_to_owned(handler_def)),
        }
    }

    /// Convert a statement to an owned version
    fn convert_stmt_to_owned(&self, stmt: &crate::ast::Stmt) -> crate::ast::Stmt<'static> {
        match stmt {
            crate::ast::Stmt::Let { is_mut, name, ty, value } => crate::ast::Stmt::Let {
                is_mut: *is_mut,
                name: Self::to_static_str(name),
                ty: ty.as_ref().map(|t| self.convert_type_to_owned(t)),
                value: self.convert_expr_to_owned(value),
            },
            crate::ast::Stmt::Return(expr) => crate::ast::Stmt::Return(
                expr.as_ref().map(|e| self.convert_expr_to_owned(e))
            ),
            crate::ast::Stmt::Assign(lhs, rhs) => crate::ast::Stmt::Assign(
                self.convert_expr_to_owned(lhs),
                self.convert_expr_to_owned(rhs),
            ),
            crate::ast::Stmt::Expr(expr) => crate::ast::Stmt::Expr(
                self.convert_expr_to_owned(expr)
            ),
            crate::ast::Stmt::Error => crate::ast::Stmt::Error,
        }
    }

    /// Convert an expression to an owned version
    fn convert_expr_to_owned(&self, expr: &crate::ast::Expr) -> crate::ast::Expr<'static> {
        match expr {
            crate::ast::Expr::Literal(lit) => crate::ast::Expr::Literal(self.convert_literal_to_owned(lit)),
            crate::ast::Expr::Array(items) => crate::ast::Expr::Array(
                items.iter().map(|e| self.convert_expr_to_owned(e)).collect()
            ),
            crate::ast::Expr::Map(items) => crate::ast::Expr::Map(
                items.iter().map(|(k, v)| (
                    self.convert_expr_to_owned(k),
                    self.convert_expr_to_owned(v)
                )).collect()
            ),
            crate::ast::Expr::Path(path) => crate::ast::Expr::Path(
                path.iter().map(|s| Self::to_static_str(s)).collect()
            ),
            crate::ast::Expr::FieldAccess { receiver, field } => crate::ast::Expr::FieldAccess {
                receiver: Box::new(self.convert_expr_to_owned(receiver)),
                field: Self::to_static_str(field),
            },
            crate::ast::Expr::Unary { op, rhs } => crate::ast::Expr::Unary {
                op: op.clone(),
                rhs: Box::new(self.convert_expr_to_owned(rhs)),
            },
            crate::ast::Expr::Binary { op, lhs, rhs } => crate::ast::Expr::Binary {
                op: op.clone(),
                lhs: Box::new(self.convert_expr_to_owned(lhs)),
                rhs: Box::new(self.convert_expr_to_owned(rhs)),
            },
            crate::ast::Expr::Call { fun, args } => crate::ast::Expr::Call {
                fun: Box::new(self.convert_expr_to_owned(fun)),
                args: args.iter().map(|e| self.convert_expr_to_owned(e)).collect(),
            },
            crate::ast::Expr::StructInit { path, generics, fields } => crate::ast::Expr::StructInit {
                path: path.iter().map(|s| Self::to_static_str(s)).collect(),
                generics: generics.iter().map(|t| self.convert_type_to_owned(t)).collect(),
                fields: fields.iter().map(|(name, expr)| (
                    Self::to_static_str(name),
                    self.convert_expr_to_owned(expr)
                )).collect(),
            },
            crate::ast::Expr::Block { stmts, last_expr } => crate::ast::Expr::Block {
                stmts: stmts.iter().map(|s| self.convert_stmt_to_owned(s)).collect(),
                last_expr: last_expr.as_ref().map(|e| Box::new(self.convert_expr_to_owned(e))),
            },
            crate::ast::Expr::If { cond, then_block, else_block } => crate::ast::Expr::If {
                cond: Box::new(self.convert_expr_to_owned(cond)),
                then_block: Box::new(self.convert_expr_to_owned(then_block)),
                else_block: else_block.as_ref().map(|e| Box::new(self.convert_expr_to_owned(e))),
            },
            crate::ast::Expr::Match { scrutinee, arms } => crate::ast::Expr::Match {
                scrutinee: Box::new(self.convert_expr_to_owned(scrutinee)),
                arms: arms.iter().map(|(pat, expr)| (
                    self.convert_pattern_to_owned(pat),
                    self.convert_expr_to_owned(expr)
                )).collect(),
            },
            crate::ast::Expr::While { cond, body } => crate::ast::Expr::While {
                cond: Box::new(self.convert_expr_to_owned(cond)),
                body: Box::new(self.convert_expr_to_owned(body)),
            },
            crate::ast::Expr::Perform { path, args } => crate::ast::Expr::Perform {
                path: path.iter().map(|s| {
                    let owned = s.to_string();
                    let leaked = Box::leak(owned.into_boxed_str());
                    &*leaked
                }).collect(),
                args: args.iter().map(|e| self.convert_expr_to_owned(e)).collect(),
            },
            crate::ast::Expr::Handle { body, handler } => crate::ast::Expr::Handle {
                body: Box::new(self.convert_expr_to_owned(body)),
                handler: self.convert_handler_body_to_owned(handler),
            },
            crate::ast::Expr::Error => crate::ast::Expr::Error,
        }
    }

    /// Convert a function to an owned version
    fn convert_function_to_owned(&self, func: &crate::ast::Function) -> crate::ast::Function<'static> {
        crate::ast::Function {
            attributes: func.attributes.iter().map(|attr| crate::ast::Attribute {
                name: {
                    let owned = attr.name.to_string();
                    let leaked = Box::leak(owned.into_boxed_str());
                    &*leaked
                },
                args: attr.args.iter().map(|s| Self::to_static_str(s)).collect(),
            }).collect(),
            name: Self::to_static_str(func.name),
            generics: func.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            params: func.params.iter().map(|(name, ty)| (
                name.map(|s| Self::to_static_str(s)),
                self.convert_type_to_owned(ty)
            )).collect(),
            ret_type: func.ret_type.as_ref().map(|t| self.convert_type_to_owned(t)),
            effects: func.effects.iter().map(|s| Self::to_static_str(s)).collect(),
            body: self.convert_expr_to_owned(&func.body),
            is_public: func.is_public,
        }
    }

    /// Convert a struct definition to an owned version
    fn convert_struct_to_owned(&self, struct_def: &crate::ast::StructDef) -> crate::ast::StructDef<'static> {
        crate::ast::StructDef {
            attributes: struct_def.attributes.iter().map(|attr| crate::ast::Attribute {
                name: Self::to_static_str(attr.name),
                args: attr.args.iter().map(|s| Self::to_static_str(s)).collect(),
            }).collect(),
            name: Self::to_static_str(struct_def.name),
            generics: struct_def.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            fields: struct_def.fields.iter().map(|(name, ty)| (
                Self::to_static_str(name),
                self.convert_type_to_owned(ty)
            )).collect(),
            is_public: struct_def.is_public,
        }
    }

    /// Convert an enum definition to an owned version
    fn convert_enum_to_owned(&self, enum_def: &crate::ast::EnumDef) -> crate::ast::EnumDef<'static> {
        crate::ast::EnumDef {
            name: enum_def.name.map(|s| Self::to_static_str(s)),
            generics: enum_def.generics.iter().map(|s| Self::to_static_str(s)).collect(),
            variants: enum_def.variants.iter().map(|(name, types)| (
                Self::to_static_str(name),
                types.as_ref().map(|ts| ts.iter().map(|t| self.convert_type_to_owned(t)).collect())
            )).collect(),
            is_public: enum_def.is_public,
        }
    }

    /// Convert a trait definition to an owned version
    fn convert_trait_to_owned(&self, trait_def: &crate::ast::TraitDef) -> crate::ast::TraitDef<'static> {
        crate::ast::TraitDef {
            name: Self::to_static_str(trait_def.name),
            methods: trait_def.methods.iter().map(|m| crate::ast::TraitMethod {
                name: Self::to_static_str(m.name),
                params: m.params.iter().map(|(name, ty)| (
                    name.map(|s| Self::to_static_str(s)),
                    self.convert_type_to_owned(ty)
                )).collect(),
                ret_type: m.ret_type.as_ref().map(|t| self.convert_type_to_owned(t)),
                is_public: m.is_public,
            }).collect(),
            is_public: trait_def.is_public,
        }
    }

    /// Convert an impl block to an owned version
    fn convert_impl_to_owned(&self, impl_block: &crate::ast::ImplBlock) -> crate::ast::ImplBlock<'static> {
        crate::ast::ImplBlock {
            trait_name: Self::to_static_str(impl_block.trait_name),
            target_type: self.convert_type_to_owned(&impl_block.target_type),
            methods: impl_block.methods.iter().map(|f| self.convert_function_to_owned(f)).collect(),
        }
    }

    /// Convert an effect definition to an owned version
    fn convert_effect_to_owned(&self, effect_def: &crate::ast::EffectDef) -> crate::ast::EffectDef<'static> {
        crate::ast::EffectDef {
            name: Self::to_static_str(effect_def.name),
            operations: effect_def.operations.iter().map(|op| crate::ast::EffectOp {
                name: Self::to_static_str(op.name),
                params: op.params.iter().map(|t| self.convert_type_to_owned(t)).collect(),
                ret_type: self.convert_type_to_owned(&op.ret_type),
                is_public: op.is_public,
            }).collect(),
            is_public: effect_def.is_public,
        }
    }

    /// Convert a handler definition to an owned version
    fn convert_handler_to_owned(&self, handler_def: &crate::ast::HandlerDef) -> crate::ast::HandlerDef<'static> {
        crate::ast::HandlerDef {
            name: Self::to_static_str(handler_def.name),
            effects: handler_def.effects.iter().map(|s| Self::to_static_str(s)).collect(),
            functions: handler_def.functions.iter().map(|f| self.convert_function_to_owned(f)).collect(),
            is_public: handler_def.is_public,
        }
    }

    /// Convert a type to an owned version
    fn convert_type_to_owned(&self, ty: &crate::ast::Type) -> crate::ast::Type<'static> {
        crate::ast::Type {
            path: ty.path.iter().map(|s| Self::to_static_str(s)).collect(),
            generics: ty.generics.iter().map(|t| self.convert_type_to_owned(t)).collect(),
        }
    }

    /// Convert a pattern to an owned version
    fn convert_pattern_to_owned(&self, pat: &crate::ast::Pattern) -> crate::ast::Pattern<'static> {
        match pat {
            crate::ast::Pattern::Literal(lit) => crate::ast::Pattern::Literal(
                self.convert_literal_to_owned(lit)
            ),
            crate::ast::Pattern::Identifier(name) => crate::ast::Pattern::Identifier(
                Self::to_static_str(name)
            ),
            crate::ast::Pattern::Path { path, args } => crate::ast::Pattern::Path {
                path: path.iter().map(|s| Self::to_static_str(s)).collect(),
                args: args.iter().map(|p| self.convert_pattern_to_owned(p)).collect(),
            },
            crate::ast::Pattern::Wildcard => crate::ast::Pattern::Wildcard,
        }
    }

    /// Convert a literal to an owned version
    fn convert_literal_to_owned(&self, lit: &crate::ast::Literal) -> crate::ast::Literal<'static> {
        match lit {
            crate::ast::Literal::Bool(b) => crate::ast::Literal::Bool(*b),
            crate::ast::Literal::I32(i) => crate::ast::Literal::I32(*i),
            crate::ast::Literal::I64(i) => crate::ast::Literal::I64(*i),
            crate::ast::Literal::F64(f) => crate::ast::Literal::F64(*f),
            crate::ast::Literal::Str(s) => crate::ast::Literal::Str(Self::to_static_str(s)),
            crate::ast::Literal::Unit => crate::ast::Literal::Unit,
        }
    }

    /// Convert a handler body to an owned version
    fn convert_handler_body_to_owned(&self, body: &crate::ast::HandlerBody) -> crate::ast::HandlerBody<'static> {
        match body {
            crate::ast::HandlerBody::Path(path) => crate::ast::HandlerBody::Path(
                path.iter().map(|s| Self::to_static_str(s)).collect()
            ),
            crate::ast::HandlerBody::Inline(functions) => crate::ast::HandlerBody::Inline(
                functions.iter().map(|f| self.convert_function_to_owned(f)).collect()
            ),
        }
    }

    /// Discover the file path for an import and add it to the queue
    fn discover_import_path(&mut self, import_path: &[&str], current_file: &Path) -> Result<(), String> {
        if import_path.len() < 2 {
            return Err(format!("Invalid import path: {:?}", import_path));
        }

        let namespace = import_path[0];
        let module = import_path[1];

        // Determine the module path based on namespace
        let module_path = if namespace == "Self" {
            // For Self imports, look relative to the current file
            current_file
                .parent()
                .ok_or_else(|| "Current file has no parent directory".to_string())?
                .join(module.to_lowercase())
        } else {
            // For standard library imports, look in modules directory
            PathBuf::from("modules")
                .join(namespace.to_lowercase())
                .join(module.to_lowercase())
        };

        // Look for .bst files in the module directory
        if module_path.exists() && module_path.is_dir() {
            if let Ok(entries) = fs::read_dir(&module_path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        if let Some(extension) = entry.path().extension() {
                            if extension == "bst" {
                                self.add_to_queue(&entry.path())?;
                            }
                        }
                    }
                }
            }
        } else {
            println!("Warning: Module path does not exist: {:?}", module_path);
        }

        Ok(())
    }

    /// Add a file path to the processing queue if not already visited
    fn add_to_queue(&mut self, path: &Path) -> Result<(), String> {
        let canonical_path = path.canonicalize()
            .map_err(|e| format!("Failed to canonicalize path {:?}: {}", path, e))?;

        if !self.visited.contains(&canonical_path) {
            self.visited.insert(canonical_path.clone());
            self.work_queue.push_back(canonical_path);
        }

        Ok(())
    }

    /// Convert static items to borrowed items for typechecking
    pub fn convert_to_borrowed_items(&self) -> Vec<Item> {
        // For now, we'll use a simple approach by converting back to borrowed items
        // This is a temporary solution - in a real implementation, we'd need proper lifetime management
        self.all_items
            .iter()
            .map(|item| self.convert_static_item_to_borrowed(item))
            .collect()
    }

    /// Convert a static item to a borrowed item for typechecking
    fn convert_static_item_to_borrowed(&self, item: &Item<'static>) -> Item {
        // For now, we'll use a simple approach by converting back to borrowed items
        // This is a temporary solution - in a real implementation, we'd need proper lifetime management
        match item {
            Item::Stmt(stmt) => Item::Stmt(self.convert_static_stmt_to_borrowed(stmt)),
            Item::Import { path, alias } => Item::Import {
                path: path.iter().map(|s| Self::to_static_str(s)).collect(),
                alias: alias.as_ref().map(|s| Self::to_static_str(s)),
            },
            Item::ExternBlock { module_name, functions } => Item::ExternBlock {
                module_name: module_name,
                functions: functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
            },
            Item::Fn(func) => Item::Fn(self.convert_static_function_to_borrowed(func)),
            Item::Struct(struct_def) => Item::Struct(self.convert_static_struct_to_borrowed(struct_def)),
            Item::Enum(enum_def) => Item::Enum(self.convert_static_enum_to_borrowed(enum_def)),
            Item::Trait(trait_def) => Item::Trait(self.convert_static_trait_to_borrowed(trait_def)),
            Item::Impl(impl_block) => Item::Impl(self.convert_static_impl_to_borrowed(impl_block)),
            Item::Effect(effect_def) => Item::Effect(self.convert_static_effect_to_borrowed(effect_def)),
            Item::Handler(handler_def) => Item::Handler(self.convert_static_handler_to_borrowed(handler_def)),
        }
    }

    fn convert_static_stmt_to_borrowed(&self, stmt: &crate::ast::Stmt<'static>) -> crate::ast::Stmt {
        match stmt {
            crate::ast::Stmt::Let { is_mut, name, ty, value } => crate::ast::Stmt::Let {
                is_mut: *is_mut,
                name,
                ty: ty.as_ref().map(|t| self.convert_static_type_to_borrowed(t)),
                value: self.convert_static_expr_to_borrowed(value),
            },
            crate::ast::Stmt::Return(expr) => crate::ast::Stmt::Return(
                expr.as_ref().map(|e| self.convert_static_expr_to_borrowed(e))
            ),
            crate::ast::Stmt::Assign(lhs, rhs) => crate::ast::Stmt::Assign(
                self.convert_static_expr_to_borrowed(lhs),
                self.convert_static_expr_to_borrowed(rhs),
            ),
            crate::ast::Stmt::Expr(expr) => crate::ast::Stmt::Expr(
                self.convert_static_expr_to_borrowed(expr)
            ),
            crate::ast::Stmt::Error => crate::ast::Stmt::Error,
        }
    }

    fn convert_static_expr_to_borrowed(&self, expr: &crate::ast::Expr<'static>) -> crate::ast::Expr {
        match expr {
            crate::ast::Expr::Literal(lit) => crate::ast::Expr::Literal(self.convert_static_literal_to_borrowed(lit)),
            crate::ast::Expr::Array(items) => crate::ast::Expr::Array(
                items.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect()
            ),
            crate::ast::Expr::Map(items) => crate::ast::Expr::Map(
                items.iter().map(|(k, v)| (
                    self.convert_static_expr_to_borrowed(k),
                    self.convert_static_expr_to_borrowed(v)
                )).collect()
            ),
            crate::ast::Expr::Path(path) => crate::ast::Expr::Path(path.clone()),
            crate::ast::Expr::FieldAccess { receiver, field } => crate::ast::Expr::FieldAccess {
                receiver: Box::new(self.convert_static_expr_to_borrowed(receiver)),
                field,
            },
            crate::ast::Expr::Unary { op, rhs } => crate::ast::Expr::Unary {
                op: op.clone(),
                rhs: Box::new(self.convert_static_expr_to_borrowed(rhs)),
            },
            crate::ast::Expr::Binary { op, lhs, rhs } => crate::ast::Expr::Binary {
                op: op.clone(),
                lhs: Box::new(self.convert_static_expr_to_borrowed(lhs)),
                rhs: Box::new(self.convert_static_expr_to_borrowed(rhs)),
            },
            crate::ast::Expr::Call { fun, args } => crate::ast::Expr::Call {
                fun: Box::new(self.convert_static_expr_to_borrowed(fun)),
                args: args.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect(),
            },
            crate::ast::Expr::StructInit { path, generics, fields } => crate::ast::Expr::StructInit {
                path: path.clone(),
                generics: generics.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
                fields: fields.iter().map(|(name, expr)| (
                    *name,
                    self.convert_static_expr_to_borrowed(expr)
                )).collect(),
            },
            crate::ast::Expr::Block { stmts, last_expr } => crate::ast::Expr::Block {
                stmts: stmts.iter().map(|s| self.convert_static_stmt_to_borrowed(s)).collect(),
                last_expr: last_expr.as_ref().map(|e| Box::new(self.convert_static_expr_to_borrowed(e))),
            },
            crate::ast::Expr::If { cond, then_block, else_block } => crate::ast::Expr::If {
                cond: Box::new(self.convert_static_expr_to_borrowed(cond)),
                then_block: Box::new(self.convert_static_expr_to_borrowed(then_block)),
                else_block: else_block.as_ref().map(|e| Box::new(self.convert_static_expr_to_borrowed(e))),
            },
            crate::ast::Expr::Match { scrutinee, arms } => crate::ast::Expr::Match {
                scrutinee: Box::new(self.convert_static_expr_to_borrowed(scrutinee)),
                arms: arms.iter().map(|(pat, expr)| (
                    self.convert_static_pattern_to_borrowed(pat),
                    self.convert_static_expr_to_borrowed(expr)
                )).collect(),
            },
            crate::ast::Expr::While { cond, body } => crate::ast::Expr::While {
                cond: Box::new(self.convert_static_expr_to_borrowed(cond)),
                body: Box::new(self.convert_static_expr_to_borrowed(body)),
            },
            crate::ast::Expr::Perform { path, args } => crate::ast::Expr::Perform {
                path: path.clone(),
                args: args.iter().map(|e| self.convert_static_expr_to_borrowed(e)).collect(),
            },
            crate::ast::Expr::Handle { body, handler } => crate::ast::Expr::Handle {
                body: Box::new(self.convert_static_expr_to_borrowed(body)),
                handler: self.convert_static_handler_body_to_borrowed(handler),
            },
            crate::ast::Expr::Error => crate::ast::Expr::Error,
        }
    }

    fn convert_static_function_to_borrowed(&self, func: &crate::ast::Function<'static>) -> crate::ast::Function {
        crate::ast::Function {
            attributes: func.attributes.iter().map(|attr| crate::ast::Attribute {
                name: attr.name,
                args: attr.args.clone(),
            }).collect(),
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

    fn convert_static_struct_to_borrowed(&self, struct_def: &crate::ast::StructDef<'static>) -> crate::ast::StructDef {
        crate::ast::StructDef {
            attributes: struct_def.attributes.iter().map(|attr| crate::ast::Attribute {
                name: attr.name,
                args: attr.args.clone(),
            }).collect(),
            name: struct_def.name,
            generics: struct_def.generics.clone(),
            fields: struct_def.fields.iter().map(|(name, ty)| (
                *name,
                self.convert_static_type_to_borrowed(ty)
            )).collect(),
            is_public: struct_def.is_public,
        }
    }

    fn convert_static_enum_to_borrowed(&self, enum_def: &crate::ast::EnumDef<'static>) -> crate::ast::EnumDef {
        crate::ast::EnumDef {
            name: enum_def.name.map(|s| s),
            generics: enum_def.generics.clone(),
            variants: enum_def.variants.iter().map(|(name, types)| (
                *name,
                types.as_ref().map(|ts| ts.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect())
            )).collect(),
            is_public: enum_def.is_public,
        }
    }

    fn convert_static_trait_to_borrowed(&self, trait_def: &crate::ast::TraitDef<'static>) -> crate::ast::TraitDef {
        crate::ast::TraitDef {
            name: trait_def.name,
            methods: trait_def.methods.iter().map(|m| crate::ast::TraitMethod {
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

    fn convert_static_impl_to_borrowed(&self, impl_block: &crate::ast::ImplBlock<'static>) -> crate::ast::ImplBlock {
        crate::ast::ImplBlock {
            trait_name: impl_block.trait_name,
            target_type: self.convert_static_type_to_borrowed(&impl_block.target_type),
            methods: impl_block.methods.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
        }
    }

    fn convert_static_effect_to_borrowed(&self, effect_def: &crate::ast::EffectDef<'static>) -> crate::ast::EffectDef {
        crate::ast::EffectDef {
            name: effect_def.name,
            operations: effect_def.operations.iter().map(|op| crate::ast::EffectOp {
                name: op.name,
                params: op.params.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
                ret_type: self.convert_static_type_to_borrowed(&op.ret_type),
                is_public: op.is_public,
            }).collect(),
            is_public: effect_def.is_public,
        }
    }

    fn convert_static_handler_to_borrowed(&self, handler_def: &crate::ast::HandlerDef<'static>) -> crate::ast::HandlerDef {
        crate::ast::HandlerDef {
            name: handler_def.name,
            effects: handler_def.effects.clone(),
            functions: handler_def.functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect(),
            is_public: handler_def.is_public,
        }
    }

    fn convert_static_type_to_borrowed(&self, ty: &crate::ast::Type<'static>) -> crate::ast::Type {
        crate::ast::Type {
            path: ty.path.clone(),
            generics: ty.generics.iter().map(|t| self.convert_static_type_to_borrowed(t)).collect(),
        }
    }

    fn convert_static_pattern_to_borrowed(&self, pat: &crate::ast::Pattern<'static>) -> crate::ast::Pattern {
        match pat {
            crate::ast::Pattern::Literal(lit) => crate::ast::Pattern::Literal(self.convert_static_literal_to_borrowed(lit)),
            crate::ast::Pattern::Identifier(name) => crate::ast::Pattern::Identifier(name),
            crate::ast::Pattern::Path { path, args } => crate::ast::Pattern::Path {
                path: path.clone(),
                args: args.iter().map(|p| self.convert_static_pattern_to_borrowed(p)).collect(),
            },
            crate::ast::Pattern::Wildcard => crate::ast::Pattern::Wildcard,
        }
    }

    fn convert_static_literal_to_borrowed(&self, lit: &crate::ast::Literal<'static>) -> crate::ast::Literal {
        match lit {
            crate::ast::Literal::Bool(b) => crate::ast::Literal::Bool(*b),
            crate::ast::Literal::I32(i) => crate::ast::Literal::I32(*i),
            crate::ast::Literal::I64(i) => crate::ast::Literal::I64(*i),
            crate::ast::Literal::F64(f) => crate::ast::Literal::F64(*f),
            crate::ast::Literal::Str(s) => crate::ast::Literal::Str(s),
            crate::ast::Literal::Unit => crate::ast::Literal::Unit,
        }
    }

    fn convert_static_handler_body_to_borrowed(&self, body: &crate::ast::HandlerBody<'static>) -> crate::ast::HandlerBody {
        match body {
            crate::ast::HandlerBody::Path(path) => crate::ast::HandlerBody::Path(path.clone()),
            crate::ast::HandlerBody::Inline(functions) => crate::ast::HandlerBody::Inline(
                functions.iter().map(|f| self.convert_static_function_to_borrowed(f)).collect()
            ),
        }
    }
} 