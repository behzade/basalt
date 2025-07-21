//! This module contains the type checker, which is responsible for validating
//! the types of expressions and statements in the AST. It ensures type safety
//! and provides detailed error messages for type mismatches.

use std::collections::HashMap;
use crate::ast::*;

/// Represents a type error that occurred during type checking
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Option<(usize, usize)>, // (start, end) character positions
}

impl TypeError {
    pub fn new(message: String) -> Self {
        Self {
            message,
            span: None,
        }
    }

    pub fn with_span(mut self, start: usize, end: usize) -> Self {
        self.span = Some((start, end));
        self
    }
}

/// The type checker context that maintains the current scope and type information
#[derive(Debug, Clone)]
pub struct TypeContext<'src> {
    /// Variables in the current scope with their types
    variables: HashMap<&'src str, Type<'src>>,
    /// Functions available in the current scope
    functions: HashMap<&'src str, Function<'src>>,
    /// Struct definitions available in the current scope
    structs: HashMap<&'src str, StructDef<'src>>,
    /// Enum definitions available in the current scope
    enums: HashMap<&'src str, EnumDef<'src>>,
    /// Trait definitions available in the current scope
    traits: HashMap<&'src str, TraitDef<'src>>,
    /// Effect definitions available in the current scope
    effects: HashMap<&'src str, EffectDef<'src>>,
    /// External functions available in the current scope
    extern_functions: HashMap<&'src str, Type<'src>>, // name -> return type
    /// Current function's return type (for return statement validation)
    current_return_type: Option<Type<'src>>,
}

impl<'src> TypeContext<'src> {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            enums: HashMap::new(),
            traits: HashMap::new(),
            effects: HashMap::new(),
            extern_functions: HashMap::new(),
            current_return_type: None,
        }
    }

    /// Add a variable to the current scope
    pub fn add_variable(&mut self, name: &'src str, ty: Type<'src>) {
        self.variables.insert(name, ty);
    }

    /// Get the type of a variable
    pub fn get_variable(&self, name: &'src str) -> Option<&Type<'src>> {
        self.variables.get(name)
    }

    /// Add a function to the current scope
    pub fn add_function(&mut self, func: Function<'src>) {
        self.functions.insert(func.name, func);
    }

    /// Get a function by name
    pub fn get_function(&self, name: &'src str) -> Option<&Function<'src>> {
        self.functions.get(name)
    }

    /// Add a struct definition to the current scope
    pub fn add_struct(&mut self, struct_def: StructDef<'src>) {
        self.structs.insert(struct_def.name, struct_def);
    }

    /// Get a struct definition by name
    pub fn get_struct(&self, name: &'src str) -> Option<&StructDef<'src>> {
        self.structs.get(name)
    }

    /// Add an enum definition to the current scope
    pub fn add_enum(&mut self, enum_def: EnumDef<'src>) {
        if let Some(name) = &enum_def.name {
            self.enums.insert(name, enum_def);
        }
    }

    /// Get an enum definition by name
    pub fn get_enum(&self, name: &'src str) -> Option<&EnumDef<'src>> {
        self.enums.get(name)
    }

    /// Add a trait definition to the current scope
    pub fn add_trait(&mut self, trait_def: TraitDef<'src>) {
        self.traits.insert(trait_def.name, trait_def);
    }

    /// Get a trait definition by name
    pub fn get_trait(&self, name: &'src str) -> Option<&TraitDef<'src>> {
        self.traits.get(name)
    }

    /// Add an effect definition to the current scope
    pub fn add_effect(&mut self, effect_def: EffectDef<'src>) {
        self.effects.insert(effect_def.name, effect_def);
    }

    /// Get an effect definition by name
    pub fn get_effect(&self, name: &'src str) -> Option<&EffectDef<'src>> {
        self.effects.get(name)
    }

    /// Add an external function to the current scope
    pub fn add_extern_function(&mut self, name: &'src str, return_type: Type<'src>) {
        self.extern_functions.insert(name, return_type);
    }

    /// Get an external function by name
    pub fn get_extern_function(&self, name: &'src str) -> Option<&Type<'src>> {
        self.extern_functions.get(name)
    }

    /// Set the current function's return type
    pub fn set_return_type(&mut self, return_type: Option<Type<'src>>) {
        self.current_return_type = return_type;
    }

    /// Get the current function's return type
    pub fn get_return_type(&self) -> Option<&Type<'src>> {
        self.current_return_type.as_ref()
    }
}

/// The main type checker that validates AST nodes
pub struct TypeChecker<'src> {
    context: TypeContext<'src>,
    errors: Vec<TypeError>,
}

impl<'src> TypeChecker<'src> {
    pub fn new() -> Self {
        Self {
            context: TypeContext::new(),
            errors: Vec::new(),
        }
    }

    /// Type check a complete file (list of items)
    pub fn check_file(&mut self, items: &[Item<'src>]) -> Result<(), Vec<TypeError>> {
        // First pass: collect all definitions
        for item in items {
            self.collect_definitions(item);
        }

        // Second pass: type check all items
        for item in items {
            self.check_item(item);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    /// Collect all definitions (functions, structs, etc.) into the context
    fn collect_definitions(&mut self, item: &Item<'src>) {
        match item {
            Item::Fn(func) => {
                self.context.add_function(func.clone());
            }
            Item::Struct(struct_def) => {
                self.context.add_struct(struct_def.clone());
            }
            Item::Enum(enum_def) => {
                self.context.add_enum(enum_def.clone());
            }
            Item::Trait(trait_def) => {
                self.context.add_trait(trait_def.clone());
            }
            Item::Effect(effect_def) => {
                self.context.add_effect(effect_def.clone());
            }
            Item::ExternFn { name, ret_type, .. } => {
                self.context.add_extern_function(name, ret_type.clone());
            }
            _ => {} // Other items don't define new types
        }
    }

    /// Type check a single item
    fn check_item(&mut self, item: &Item<'src>) {
        match item {
            Item::Stmt(stmt) => {
                self.check_stmt(stmt);
            }
            Item::Fn(func) => {
                self.check_function(func);
            }
            Item::Struct(struct_def) => {
                self.check_struct(struct_def);
            }
            Item::Enum(enum_def) => {
                self.check_enum(enum_def);
            }
            Item::Trait(trait_def) => {
                self.check_trait(trait_def);
            }
            Item::Effect(effect_def) => {
                self.check_effect(effect_def);
            }
            Item::Impl(impl_block) => {
                self.check_impl(impl_block);
            }
            Item::Handler(handler_def) => {
                self.check_handler(handler_def);
            }
            Item::Import { .. } => {
                // Imports are handled at a different level
            }
            Item::ExternFn { .. } => {
                // Extern functions are already collected
            }
        }
    }

    /// Type check a statement
    fn check_stmt(&mut self, stmt: &Stmt<'src>) {
        match stmt {
            Stmt::Let { name, ty, value, .. } => {
                let value_type = self.check_expr(value);
                if let Some(declared_type) = ty {
                    if !self.types_compatible(&value_type, declared_type) {
                        self.errors.push(TypeError::new(format!(
                            "Type mismatch in let binding: expected {:?}, got {:?}",
                            declared_type, value_type
                        )));
                    }
                }
                self.context.add_variable(name, value_type);
            }
            Stmt::Return(expr) => {
                if let Some(expr) = expr {
                    let expr_type = self.check_expr(expr);
                    if let Some(expected_return_type) = self.context.get_return_type() {
                        if !self.types_compatible(&expr_type, expected_return_type) {
                            self.errors.push(TypeError::new(format!(
                                "Return type mismatch: expected {:?}, got {:?}",
                                expected_return_type, expr_type
                            )));
                        }
                    }
                } else {
                    // Return without expression - check if we expect Unit
                    if let Some(expected_return_type) = self.context.get_return_type() {
                        if !self.is_unit_type(expected_return_type) {
                            self.errors.push(TypeError::new(format!(
                                "Function expects return type {:?}, but return statement has no value",
                                expected_return_type
                            )));
                        }
                    }
                }
            }
            Stmt::Assign(lhs, rhs) => {
                let lhs_type = self.check_expr(lhs);
                let rhs_type = self.check_expr(rhs);
                if !self.types_compatible(&lhs_type, &rhs_type) {
                    self.errors.push(TypeError::new(format!(
                        "Type mismatch in assignment: cannot assign {:?} to {:?}",
                        rhs_type, lhs_type
                    )));
                }
            }
            Stmt::Expr(expr) => {
                self.check_expr(expr);
            }
            Stmt::Error => {
                // Skip error statements
            }
        }
    }

    /// Type check an expression and return its type
    fn check_expr(&mut self, expr: &Expr<'src>) -> Type<'src> {
        match expr {
            Expr::Literal(lit) => self.type_of_literal(lit),
            Expr::Array(elements) => {
                if elements.is_empty() {
                    // Empty array - we can't infer the type
                    Type {
                        path: vec!["Array"],
                        generics: vec![Type {
                            path: vec!["Unknown"],
                            generics: vec![],
                        }],
                    }
                } else {
                    let element_types: Vec<_> = elements.iter().map(|e| self.check_expr(e)).collect();
                    let first_type = &element_types[0];
                    
                    // Check that all elements have the same type
                    for (i, element_type) in element_types.iter().enumerate().skip(1) {
                        if !self.types_compatible(first_type, element_type) {
                            self.errors.push(TypeError::new(format!(
                                "Array element {} has type {:?}, expected {:?}",
                                i, element_type, first_type
                            )));
                        }
                    }
                    
                    Type {
                        path: vec!["Array"],
                        generics: vec![first_type.clone()],
                    }
                }
            }
            Expr::Map(pairs) => {
                if pairs.is_empty() {
                    // Empty map - we can't infer the types
                    Type {
                        path: vec!["Map"],
                        generics: vec![
                            Type {
                                path: vec!["Unknown"],
                                generics: vec![],
                            },
                            Type {
                                path: vec!["Unknown"],
                                generics: vec![],
                            },
                        ],
                    }
                } else {
                    let key_types: Vec<_> = pairs.iter().map(|(k, _)| self.check_expr(k)).collect();
                    let value_types: Vec<_> = pairs.iter().map(|(_, v)| self.check_expr(v)).collect();
                    
                    let first_key_type = &key_types[0];
                    let first_value_type = &value_types[0];
                    
                    // Check that all keys have the same type
                    for (i, key_type) in key_types.iter().enumerate().skip(1) {
                        if !self.types_compatible(first_key_type, key_type) {
                            self.errors.push(TypeError::new(format!(
                                "Map key {} has type {:?}, expected {:?}",
                                i, key_type, first_key_type
                            )));
                        }
                    }
                    
                    // Check that all values have the same type
                    for (i, value_type) in value_types.iter().enumerate().skip(1) {
                        if !self.types_compatible(first_value_type, value_type) {
                            self.errors.push(TypeError::new(format!(
                                "Map value {} has type {:?}, expected {:?}",
                                i, value_type, first_value_type
                            )));
                        }
                    }
                    
                    Type {
                        path: vec!["Map"],
                        generics: vec![first_key_type.clone(), first_value_type.clone()],
                    }
                }
            }
            Expr::Path(path) => {
                // Try to resolve the path as a variable, function, or type
                if let Some(var_type) = self.context.get_variable(path[0]) {
                    var_type.clone()
                } else if let Some(func) = self.context.get_function(path[0]) {
                    // Return the function's return type, or unit if none
                    func.ret_type.clone().unwrap_or_else(|| Type {
                        path: vec!["Unit"],
                        generics: vec![],
                    })
                } else if let Some(extern_type) = self.context.get_extern_function(path[0]) {
                    extern_type.clone()
                } else {
                    // Try to resolve as enum variant
                    if path.len() == 2 {
                        if let Some(enum_def) = self.context.get_enum(path[0]) {
                            // Check if the second part is a variant
                            for (variant_name, _) in &enum_def.variants {
                                if variant_name == &path[1] {
                                    return Type {
                                        path: path.clone(),
                                        generics: vec![],
                                    };
                                }
                            }
                        }
                    }
                    
                    // Unknown identifier
                    self.errors.push(TypeError::new(format!(
                        "Unknown identifier: {}",
                        path.join("::")
                    )));
                    Type {
                        path: vec!["Unknown"],
                        generics: vec![],
                    }
                }
            }
            Expr::Unary { op, rhs } => {
                let rhs_type = self.check_expr(rhs);
                match op {
                    UnaryOp::Neg => {
                        if !self.is_numeric_type(&rhs_type) {
                            self.errors.push(TypeError::new(format!(
                                "Cannot apply negation to non-numeric type {:?}",
                                rhs_type
                            )));
                        }
                        rhs_type
                    }
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                let lhs_type = self.check_expr(lhs);
                let rhs_type = self.check_expr(rhs);
                
                match op {
                    BinaryOp::Add => {
                        if self.is_string_type(&lhs_type) && self.is_string_type(&rhs_type) {
                            // String concatenation
                            Type {
                                path: vec!["string"],
                                generics: vec![],
                            }
                        } else if self.is_numeric_type(&lhs_type) && self.is_numeric_type(&rhs_type) {
                            // Numeric addition
                            lhs_type
                        } else {
                            self.errors.push(TypeError::new(format!(
                                "Cannot apply Add to incompatible types {:?} and {:?}",
                                lhs_type, rhs_type
                            )));
                            Type {
                                path: vec!["Unknown"],
                                generics: vec![],
                            }
                        }
                    }
                    BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                        if !self.is_numeric_type(&lhs_type) || !self.is_numeric_type(&rhs_type) {
                            self.errors.push(TypeError::new(format!(
                                "Cannot apply {:?} to non-numeric types {:?} and {:?}",
                                op, lhs_type, rhs_type
                            )));
                        }
                        // For now, assume the result type is the same as the left operand
                        lhs_type
                    }
                    BinaryOp::Eq | BinaryOp::Ne => {
                        if !self.types_compatible(&lhs_type, &rhs_type) {
                            self.errors.push(TypeError::new(format!(
                                "Cannot compare incompatible types {:?} and {:?}",
                                lhs_type, rhs_type
                            )));
                        }
                        Type {
                            path: vec!["bool"],
                            generics: vec![],
                        }
                    }
                    BinaryOp::Lt | BinaryOp::Gt => {
                        if !self.is_numeric_type(&lhs_type) || !self.is_numeric_type(&rhs_type) {
                            self.errors.push(TypeError::new(format!(
                                "Cannot apply {:?} to non-numeric types {:?} and {:?}",
                                op, lhs_type, rhs_type
                            )));
                        }
                        Type {
                            path: vec!["bool"],
                            generics: vec![],
                        }
                    }
                }
            }
            Expr::Call { fun, args: _args } => {
                let _fun_type = self.check_expr(fun);
                
                // For now, assume function calls return unit
                // In a more sophisticated implementation, we'd look up the function
                // and check argument types against parameter types
                Type {
                    path: vec!["Unit"],
                    generics: vec![],
                }
            }
            Expr::StructInit { path, generics, fields } => {
                // Check that the struct exists
                if let Some(struct_def) = self.context.get_struct(path[0]) {
                    // Clone the struct definition to avoid borrowing issues
                    let struct_def = struct_def.clone();
                    
                    // Check that all required fields are provided
                    for (field_name, _field_type) in &struct_def.fields {
                        if !fields.iter().any(|(name, _)| name == field_name) {
                            self.errors.push(TypeError::new(format!(
                                "Missing required field '{}' in struct initialization",
                                field_name
                            )));
                        }
                    }
                    
                    // Check that provided fields have correct types
                    for (field_name, field_expr) in fields {
                        if let Some(expected_type) = struct_def.fields.iter().find(|(name, _)| name == field_name).map(|(_, ty)| ty) {
                            let actual_type = self.check_expr(field_expr);
                            if !self.types_compatible(expected_type, &actual_type) {
                                self.errors.push(TypeError::new(format!(
                                    "Field '{}' has type {:?}, expected {:?}",
                                    field_name, actual_type, expected_type
                                )));
                            }
                        } else {
                            self.errors.push(TypeError::new(format!(
                                "Unknown field '{}' in struct '{}'",
                                field_name, path[0]
                            )));
                        }
                    }
                    
                    Type {
                        path: path.clone(),
                        generics: generics.clone(),
                    }
                } else {
                    self.errors.push(TypeError::new(format!(
                        "Unknown struct: {}",
                        path.join("::")
                    )));
                    Type {
                        path: vec!["Unknown"],
                        generics: vec![],
                    }
                }
            }
            Expr::Block { stmts, last_expr } => {
                // Create a new scope for the block
                let mut block_context = self.context.clone();
                std::mem::swap(&mut self.context, &mut block_context);
                
                // Check all statements
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                
                // Check the last expression (if any)
                let result_type = if let Some(expr) = last_expr {
                    self.check_expr(expr)
                } else {
                    Type {
                        path: vec!["Unit"],
                        generics: vec![],
                    }
                };
                
                // Restore the original context
                std::mem::swap(&mut self.context, &mut block_context);
                
                result_type
            }
            Expr::If { cond, then_block, else_block } => {
                let cond_type = self.check_expr(cond);
                if !self.is_bool_type(&cond_type) {
                    self.errors.push(TypeError::new(format!(
                        "Condition in if expression must be boolean, got {:?}",
                        cond_type
                    )));
                }
                
                let then_type = self.check_expr(then_block);
                if let Some(else_expr) = else_block {
                    let else_type = self.check_expr(else_expr);
                    if !self.types_compatible(&then_type, &else_type) {
                        self.errors.push(TypeError::new(format!(
                            "If branches have incompatible types: {:?} and {:?}",
                            then_type, else_type
                        )));
                    }
                    then_type
                } else {
                    then_type
                }
            }
            Expr::Match { scrutinee, arms } => {
                let _scrutinee_type = self.check_expr(scrutinee);
                
                if arms.is_empty() {
                    self.errors.push(TypeError::new("Match expression must have at least one arm".to_string()));
                    Type {
                        path: vec!["Unknown"],
                        generics: vec![],
                    }
                } else {
                    let arm_types: Vec<_> = arms.iter().map(|(_, expr)| self.check_expr(expr)).collect();
                    let first_arm_type = &arm_types[0];
                    
                    // Check that all arms have the same type
                    for (i, arm_type) in arm_types.iter().enumerate().skip(1) {
                        if !self.types_compatible(first_arm_type, arm_type) {
                            self.errors.push(TypeError::new(format!(
                                "Match arm {} has type {:?}, expected {:?}",
                                i, arm_type, first_arm_type
                            )));
                        }
                    }
                    
                    first_arm_type.clone()
                }
            }
            Expr::While { cond, body } => {
                let cond_type = self.check_expr(cond);
                if !self.is_bool_type(&cond_type) {
                    self.errors.push(TypeError::new(format!(
                        "Condition in while loop must be boolean, got {:?}",
                        cond_type
                    )));
                }
                
                self.check_expr(body);
                
                // While loops always return unit
                Type {
                    path: vec!["Unit"],
                    generics: vec![],
                }
            }
            Expr::Perform(_path) => {
                // Perform expressions return the effect's return type
                // For now, assume they return unit
                Type {
                    path: vec!["Unit"],
                    generics: vec![],
                }
            }
            Expr::Handle { body, handler } => {
                // Check the body expression
                let body_type = self.check_expr(body);
                
                // Check the handler (simplified for now)
                match handler {
                    HandlerBody::Path(_) => {
                        // Handler by path - assume it's valid
                    }
                    HandlerBody::Inline(functions) => {
                        for func in functions {
                            self.check_function(func);
                        }
                    }
                }
                
                body_type
            }
            Expr::Error => {
                // Skip error expressions
                Type {
                    path: vec!["Unknown"],
                    generics: vec![],
                }
            }
        }
    }

    /// Get the type of a literal
    fn type_of_literal(&self, lit: &Literal<'src>) -> Type<'src> {
        match lit {
            Literal::Bool(_) => Type {
                path: vec!["bool"],
                generics: vec![],
            },
            Literal::I64(_) => Type {
                path: vec!["i64"],
                generics: vec![],
            },
            Literal::F64(_) => Type {
                path: vec!["f64"],
                generics: vec![],
            },
            Literal::Str(_) => Type {
                path: vec!["string"],
                generics: vec![],
            },
        }
    }

    /// Check if a type is numeric
    fn is_numeric_type(&self, ty: &Type<'src>) -> bool {
        matches!(ty.path.as_slice(), ["i64"] | ["f64"])
    }

    /// Check if a type is boolean
    fn is_bool_type(&self, ty: &Type<'src>) -> bool {
        matches!(ty.path.as_slice(), ["bool"])
    }

    /// Check if a type is string
    fn is_string_type(&self, ty: &Type<'src>) -> bool {
        matches!(ty.path.as_slice(), ["string"])
    }

    /// Check if a type is unit
    fn is_unit_type(&self, ty: &Type<'src>) -> bool {
        matches!(ty.path.as_slice(), ["Unit"] | ["none"])
    }

    /// Check if two types are compatible
    fn types_compatible(&self, t1: &Type<'src>, t2: &Type<'src>) -> bool {
        // For now, do simple equality check
        // In a more sophisticated implementation, this would handle
        // subtyping, type parameters, etc.
        t1 == t2
    }

    /// Type check a function definition
    fn check_function(&mut self, func: &Function<'src>) {
        // Create a new context for the function body
        let mut func_context = self.context.clone();
        
        // Set the return type for this function
        func_context.set_return_type(func.ret_type.clone());
        
        // Add parameters to the context
        for (name, param_type) in &func.params {
            if let Some(name) = name {
                func_context.add_variable(name, param_type.clone());
            }
        }
        
        // Swap contexts and check the body
        std::mem::swap(&mut self.context, &mut func_context);
        let body_type = self.check_function_body(&func.body);
        std::mem::swap(&mut self.context, &mut func_context);
        
        // Check that the body type matches the return type
        if let Some(return_type) = &func.ret_type {
            if !self.types_compatible(&body_type, return_type) {
                self.errors.push(TypeError::new(format!(
                    "Function '{}' body has type {:?}, expected {:?}",
                    func.name, body_type, return_type
                )));
            }
        }
    }

    /// Type check a function body and return its type
    fn check_function_body(&mut self, body: &Expr<'src>) -> Type<'src> {
        match body {
            Expr::Block { stmts, last_expr } => {
                // Check all statements
                for stmt in stmts {
                    self.check_stmt(stmt);
                }
                
                // Check the last expression (if any)
                if let Some(expr) = last_expr {
                    self.check_expr(expr)
                } else {
                    Type {
                        path: vec!["Unit"],
                        generics: vec![],
                    }
                }
            }
            _ => {
                // If the body is not a block, check it as a regular expression
                self.check_expr(body)
            }
        }
    }

    /// Type check a struct definition
    fn check_struct(&mut self, struct_def: &StructDef<'src>) {
        // Check that field names are unique
        let mut field_names = std::collections::HashSet::new();
        for (field_name, _) in &struct_def.fields {
            if !field_names.insert(field_name) {
                self.errors.push(TypeError::new(format!(
                    "Duplicate field name '{}' in struct '{}'",
                    field_name, struct_def.name
                )));
            }
        }
    }

    /// Type check an enum definition
    fn check_enum(&mut self, enum_def: &EnumDef<'src>) {
        // Check that variant names are unique
        let mut variant_names = std::collections::HashSet::new();
        for (variant_name, _) in &enum_def.variants {
            if !variant_names.insert(variant_name) {
                self.errors.push(TypeError::new(format!(
                    "Duplicate variant name '{}' in enum",
                    variant_name
                )));
            }
        }
    }

    /// Type check a trait definition
    fn check_trait(&mut self, trait_def: &TraitDef<'src>) {
        // Check that method names are unique
        let mut method_names = std::collections::HashSet::new();
        for method in &trait_def.methods {
            if !method_names.insert(&method.name) {
                self.errors.push(TypeError::new(format!(
                    "Duplicate method name '{}' in trait '{}'",
                    method.name, trait_def.name
                )));
            }
        }
    }

    /// Type check an effect definition
    fn check_effect(&mut self, effect_def: &EffectDef<'src>) {
        // Check that operation names are unique
        let mut op_names = std::collections::HashSet::new();
        for op in &effect_def.operations {
            if !op_names.insert(&op.name) {
                self.errors.push(TypeError::new(format!(
                    "Duplicate operation name '{}' in effect '{}'",
                    op.name, effect_def.name
                )));
            }
        }
    }

    /// Type check an impl block
    fn check_impl(&mut self, impl_block: &ImplBlock<'src>) {
        // Check that the trait exists
        if self.context.get_trait(impl_block.trait_name).is_none() {
            self.errors.push(TypeError::new(format!(
                "Unknown trait '{}' in impl block",
                impl_block.trait_name
            )));
        }
        
        // Check all methods in the impl block
        for method in &impl_block.methods {
            self.check_function(method);
        }
    }

    /// Type check a handler definition
    fn check_handler(&mut self, handler_def: &HandlerDef<'src>) {
        // Check that all effects exist
        for effect_name in &handler_def.effects {
            if self.context.get_effect(effect_name).is_none() {
                self.errors.push(TypeError::new(format!(
                    "Unknown effect '{}' in handler '{}'",
                    effect_name, handler_def.name
                )));
            }
        }
        
        // Check all functions in the handler
        for func in &handler_def.functions {
            self.check_function(func);
        }
    }
}

/// Convenience function to type check a file
pub fn type_check_file<'src>(items: &[Item<'src>]) -> Result<(), Vec<TypeError>> {
    let mut checker = TypeChecker::new();
    checker.check_file(items)
} 