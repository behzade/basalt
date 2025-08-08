use crate::ast::BinaryOp;
use crate::ast_owned::*; // Your Owned AST definitions
use crate::hir; // Your HIR definitions
use crate::token::SimpleSpan;
use ariadne::{Color, Fmt};
use std::collections::HashMap;
use std::path::PathBuf;

/// A type error with source location information
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub context: ItemContext,
}

// A simplified representation of a symbol in a scope.
// This helps track variables, functions, and types.
#[derive(Debug, Clone)]
enum Symbol {
    Variable {
        ty: hir::Ty,
        is_mut: bool,
    },
    Function {
        signature: hir::HirFunctionSignature,
    },
    Type {
        canonical_path: hir::OwnedPath,
        ty: hir::Ty,
    },
}

#[derive(Default)]
pub struct Typechecker {
    /// A stack of scopes. The last element is the current, innermost scope.
    /// Used for resolving local variables and function parameters.
    scopes: Vec<HashMap<String, Symbol>>,

    /// A global map of all defined types (structs, enums, effects).
    /// The key is the canonical, fully-qualified path to the type.
    type_definitions: HashMap<hir::OwnedPath, hir::Item>,

    /// A place to collect errors found during checking.
    errors: Vec<TypeError>,

    /// Context for the current function being checked, needed for 'return' statements.
    current_fn_return_type: Option<hir::Ty>,
}

#[derive(Debug, Clone)]
pub struct ItemContext {
    pub span: SimpleSpan,
    pub path: PathBuf,
}

impl Typechecker {
    pub fn check_program(
        &mut self,
        files: HashMap<PathBuf, Vec<OwnedItemWithSpan>>,
    ) -> Result<Vec<hir::Item>, Vec<TypeError>> {
        // --- PASS 1: Register all top-level definitions ---
        for file in &files {
            for item in file.1 {
                self.register_top_level_item(item);
            }
        }

        let mut hir_items: Vec<hir::Item> = Vec::new();
        for file in &files {
            hir_items.extend(
                file.1
                    .iter()
                    .filter_map(|item| self.lower_item(item.clone(), file.0.to_path_buf()).ok()),
            );
        }

        if self.errors.is_empty() {
            Ok(hir_items)
        } else {
            Err(self.errors.clone())
        }
    }

    //================================================================================//
    //                             Lowering Logic
    //================================================================================//

    /// Lowers a single `OwnedItem` from AST to HIR. This is the dispatcher.
    fn lower_item(&mut self, item: OwnedItemWithSpan, path: PathBuf) -> Result<hir::Item, ()> {
        match item.item {
            OwnedItem::Fn(func) => self
                .lower_function(
                    func,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(hir::Item::Fn),
            OwnedItem::Struct(s) => self
                .lower_struct(
                    s,
                    ItemContext {
                        span: item.span,
                        path,
                    },
                )
                .map(hir::Item::Struct),
            // TODO: Implement lowering for Enum, Trait, Effect, etc.
            _ => {
                self.errors.push(TypeError {
                    message: "Lowering for this item type is not yet implemented.".to_string(),
                    context: ItemContext {
                        span: item.span,
                        path,
                    },
                });
                Err(())
            }
        }
    }

    /// Lowers an `OwnedFunction` to a `hir::HirFunction`.
    fn lower_function(
        &mut self,
        func: OwnedFunction,
        context: ItemContext,
    ) -> Result<hir::HirFunction, ()> {
        // 1. Resolve types for the function signature.
        let params = func
            .params
            .iter()
            .map(|(name, ty)| {
                let resolved_ty = self.resolve_type(ty, context.clone())?;
                // Assume param name is present for now.
                Ok((name.clone().unwrap_or_else(|| "_".to_string()), resolved_ty))
            })
            .collect::<Result<Vec<_>, ()>>()?;

        let ret_type = match &func.ret_type {
            Some(rt) => self.resolve_type(rt, context.clone())?,
            None => hir::Ty::Special(hir::SpecialTy::Unit), // Default return type is unit
        };

        // TODO: Resolve effect types.
        let effects = Vec::new();

        let signature = hir::HirFunctionSignature {
            name: func.name.clone(),
            params: params.clone(),
            ret_type: ret_type.clone(),
            effects,
        };

        // 2. Set context and scope for checking the body.
        let old_return_type = self.current_fn_return_type.replace(ret_type);
        self.enter_scope();

        // Add function parameters as variables to the new scope.
        for (p_name, p_ty) in &params {
            let symbol = Symbol::Variable {
                ty: p_ty.clone(),
                is_mut: false,
            };
            self.add_symbol_to_current_scope(p_name.clone(), symbol);
        }

        // 3. Lower the function body.
        // The AST has an `OwnedExpr` body, while the HIR expects a `HirBlock`.
        // We lower the expression and ensure it's a block.
        let body_expr = self.lower_expr(func.body, context.clone())?;
        let body_block = match body_expr.kind {
            hir::ExprKind::Block(block) => {
                // The type of the block must match the function's return type.
                if block.ty != signature.ret_type {
                    self.errors.push(TypeError {
                        message: format!(
                            "Mismatched return type for function '{}': expected {} but found {}",
                            func.name,
                            format!("{:?}", signature.ret_type).fg(Color::Green),
                            format!("{:?}", block.ty).fg(Color::Red)
                        ),
                        context: context.clone(),
                    });
                }
                block
            }
            _ => {
                // This indicates a parser error, as function bodies should always be blocks.
                panic!("Compiler error: function body did not lower to a block expression.");
            }
        };

        // 4. Clean up scope and context.
        self.leave_scope();
        self.current_fn_return_type = old_return_type;

        Ok(hir::HirFunction {
            signature,
            body: body_block,
            is_public: func.is_public,
        })
    }

    /// Lowers an `OwnedExpr` to a `hir::Expr`, the core of type checking.
    fn lower_expr(&mut self, expr: SpannedExpr, context: ItemContext) -> Result<hir::Expr, ()> {
        match expr.item {
            OwnedExpr::Literal(lit) => {
                // Literals have a straightforward mapping to primitive types.
                let (ty, val_str) = self.lower_literal(lit);
                Ok(hir::Expr {
                    kind: hir::ExprKind::Literal(ty.clone(), val_str),
                    ty: hir::Ty::Primitive(ty),
                })
            }
            OwnedExpr::Path(path) => {
                // Look up the path in the current scope to find what it refers to.
                let name = path.last().expect("Path cannot be empty");
                match self.lookup_symbol(name) {
                    Some(Symbol::Variable { ty, .. }) => Ok(hir::Expr {
                        kind: hir::ExprKind::Path(path),
                        ty: ty.clone(),
                    }),
                    None => {
                        self.errors.push(TypeError {
                            message: format!("Cannot find value `{}` in this scope", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    _ => {
                        self.errors.push(TypeError {
                            message: format!("`{}` is not a variable", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                }
            }
            OwnedExpr::Binary { op, lhs, rhs } => {
                let hir_lhs = self.lower_expr(*lhs, context.clone())?;
                let hir_rhs = self.lower_expr(*rhs, context.clone())?;

                // Here you'd implement type checking rules for binary operators.
                // E.g., arithmetic ops need numbers, logical ops need booleans.
                if hir_lhs.ty != hir_rhs.ty {
                    self.errors.push(TypeError {
                        message: format!(
                            "Binary operation between mismatched types: expected {} but found {}",
                            format!("{:?}", hir_lhs.ty).fg(Color::Green),
                            format!("{:?}", hir_rhs.ty).fg(Color::Red)
                        ),
                        context: context.clone(),
                    });
                }

                // Determine the result type of the expression.
                let result_ty = match self.lower_binary_op(op) {
                    hir::BinaryOp::Add
                    | hir::BinaryOp::Sub
                    | hir::BinaryOp::Mul
                    | hir::BinaryOp::Div
                    | hir::BinaryOp::Mod
                    | hir::BinaryOp::Assign
                    | hir::BinaryOp::BitShiftLeft
                    | hir::BinaryOp::BitShiftRight
                    | hir::BinaryOp::Xor => hir_lhs.ty.clone(),
                    hir::BinaryOp::Eq
                    | hir::BinaryOp::Ne
                    | hir::BinaryOp::Lt
                    | hir::BinaryOp::Lte
                    | hir::BinaryOp::Gt
                    | hir::BinaryOp::Gte
                    | hir::BinaryOp::And
                    | hir::BinaryOp::Or => hir::Ty::Primitive(hir::PrimitiveTy::Bool),
                };

                Ok(hir::Expr {
                    kind: hir::ExprKind::Binary {
                        op: self.lower_binary_op(op),
                        lhs: Box::new(hir_lhs),
                        rhs: Box::new(hir_rhs),
                    },
                    ty: result_ty,
                })
            }
            OwnedExpr::Block { stmts, last_expr } => {
                self.enter_scope();
                let hir_stmts = stmts
                    .into_iter()
                    .filter_map(|s| self.lower_stmt(s, context.clone()).ok())
                    .collect();

                let (hir_last_expr, block_ty) = match last_expr {
                    Some(expr) => {
                        let hir_expr = self.lower_expr(*expr, context.clone())?;
                        let ty = hir_expr.ty.clone();
                        (Some(Box::new(hir_expr)), ty)
                    }
                    None => (None, hir::Ty::Special(hir::SpecialTy::Unit)),
                };
                self.leave_scope();

                let block = hir::HirBlock {
                    stmts: hir_stmts,
                    last_expr: hir_last_expr,
                    ty: block_ty.clone(),
                };

                Ok(hir::Expr {
                    kind: hir::ExprKind::Block(block),
                    ty: block_ty,
                })
            }
            // TODO: Implement lowering for other expressions (If, Match, Call, etc.)
            _ => {
                self.errors.push(TypeError {
                    message: "Lowering for this expression type not implemented.".to_string(),
                    context: context.clone(),
                });
                Err(())
            }
        }
    }

    /// Lowers an `OwnedStmt` to a `hir::Stmt`.
    fn lower_stmt(&mut self, stmt: SpannedStmt, context: ItemContext) -> Result<hir::Stmt, ()> {
        match stmt.item {
            OwnedStmt::Let {
                is_mut,
                name,
                ty,
                value,
            } => {
                let hir_value = self.lower_expr(value, context.clone())?;

                // Infer the type from the value, or check against the annotation.
                let var_ty = if let Some(annotated_ty) = ty {
                    let resolved_ty = self.resolve_type(&annotated_ty, context.clone())?;
                    if resolved_ty != hir_value.ty {
                        self.errors.push(TypeError {
                            message: format!(
                                "Mismatched types for variable '{}': expected {} but found {}",
                                name,
                                format!("{:?}", resolved_ty).fg(Color::Green),
                                format!("{:?}", hir_value.ty).fg(Color::Red)
                            ),
                            context: context.clone(),
                        });
                    }
                    resolved_ty
                } else {
                    hir_value.ty.clone()
                };

                // Add the new variable to the current scope.
                let symbol = Symbol::Variable {
                    ty: var_ty.clone(),
                    is_mut,
                };
                self.add_symbol_to_current_scope(name.clone(), symbol);

                Ok(hir::Stmt::Let {
                    name,
                    value: hir_value,
                    ty: var_ty,
                    is_mut,
                })
            }
            _ => {
                self.errors.push(TypeError {
                    message: "Lowering for this statement type not implemented.".to_string(),
                    context: context.clone(),
                });
                Err(())
            }
        }
    }

    /// Lowers a `OwnedStructDef` to a `hir::HirStructDef`.
    /// This is simpler than a function as it has no executable body.
    fn lower_struct(
        &mut self,
        s: OwnedStructDef,
        context: ItemContext,
    ) -> Result<hir::HirStructDef, ()> {
        let fields = s
            .fields
            .into_iter()
            .map(|(name, ty)| self.resolve_type(&ty, context.clone()).map(|t| (name, t)))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(hir::HirStructDef {
            name: s.name,
            fields,
            is_public: s.is_public,
        })
    }

    //================================================================================//
    //                             Helper & Utility Functions
    //================================================================================//

    /// **Crucial**: Resolves an AST type representation into a canonical HIR type.
    fn resolve_type(&mut self, owned_ty: &OwnedType, context: ItemContext) -> Result<hir::Ty, ()> {
        // This is a placeholder for a complex process. A real implementation must:
        // 1. Handle primitive types ("i32", "bool", etc.).
        // 2. Look up custom types (structs, enums) in `type_definitions`.
        // 3. Resolve generic type parameters.
        // 4. Handle fully-qualified paths.
        let type_name = owned_ty.path.join("::");
        match type_name.as_str() {
            "i32" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I32)),
            "i64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::I64)),
            "f64" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::F64)),
            "bool" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Bool)),
            "str" => Ok(hir::Ty::Primitive(hir::PrimitiveTy::Str)),
            "()" => Ok(hir::Ty::Special(hir::SpecialTy::Unit)),
            _ => {
                self.errors.push(TypeError {
                    message: format!("Unknown type: {}", type_name),
                    context: context.clone(),
                });
                Err(())
            }
        }
    }

    /// Pass 1: Registers an item in the global scope/type map.
    fn register_top_level_item(&mut self, _item: &OwnedItemWithSpan) {
        // TODO: Implement registration logic.
        // For a struct: parse its definition, create a `hir::Ty`, and store it.
        // For a function: parse its signature and store it in the global scope.
    }

    // --- Scope Management ---
    fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn leave_scope(&mut self) {
        self.scopes.pop();
    }
    fn add_symbol_to_current_scope(&mut self, name: String, symbol: Symbol) {
        self.scopes.last_mut().unwrap().insert(name, symbol);
    }

    fn lookup_symbol(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(symbol) = scope.get(name) {
                return Some(symbol);
            }
        }
        None
    }

    // --- Simple Converters ---
    fn lower_literal(&self, lit: OwnedLiteral) -> (hir::PrimitiveTy, String) {
        match lit {
            OwnedLiteral::Bool(b) => (hir::PrimitiveTy::Bool, b.to_string()),
            OwnedLiteral::I32(i) => (hir::PrimitiveTy::I32, i.to_string()),
            OwnedLiteral::I64(i) => (hir::PrimitiveTy::I64, i.to_string()),
            OwnedLiteral::F64(f) => (hir::PrimitiveTy::F64, f.to_string()),
            OwnedLiteral::Str(s) => (hir::PrimitiveTy::Str, s),
            _ => panic!("Unsupported literal type"),
        }
    }

    fn lower_binary_op(&self, op: BinaryOp) -> hir::BinaryOp {
        match op {
            BinaryOp::Add => hir::BinaryOp::Add,
            BinaryOp::Sub => hir::BinaryOp::Sub,
            BinaryOp::Mul => hir::BinaryOp::Mul,
            BinaryOp::Div => hir::BinaryOp::Div,
            BinaryOp::Mod => hir::BinaryOp::Mod,
            BinaryOp::Assign => hir::BinaryOp::Assign,
            BinaryOp::Eq => hir::BinaryOp::Eq,
            BinaryOp::Ne => hir::BinaryOp::Ne,
            BinaryOp::Lt => hir::BinaryOp::Lt,
            BinaryOp::Lte => hir::BinaryOp::Lte,
            BinaryOp::Gt => hir::BinaryOp::Gt,
            BinaryOp::Gte => hir::BinaryOp::Gte,
            BinaryOp::And => hir::BinaryOp::And,
            BinaryOp::Or => hir::BinaryOp::Or,

            BinaryOp::BinaryXor => hir::BinaryOp::Xor,
            BinaryOp::BinaryAnd => hir::BinaryOp::And,
            BinaryOp::BinaryOr => hir::BinaryOp::Or,

            BinaryOp::BitShiftLeft => hir::BinaryOp::BitShiftLeft,
            BinaryOp::BitShiftRight => hir::BinaryOp::BitShiftRight,
        }
    }
}
