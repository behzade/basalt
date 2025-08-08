use crate::ast::BinaryOp;
use crate::ast_owned::*; // Your Owned AST definitions
use crate::hir; // Your HIR definitions
use crate::token::SimpleSpan;
use crate::type_unifier::TypeUnifier;
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

    /// Cached map from enum (union) name to its variants for quick lookup.
    /// Populated from type aliases that define unions.
    union_variants: HashMap<hir::OwnedPath, Vec<(String, Option<Vec<hir::Ty>>)>>,
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
        // Ensure there is a global scope
        self.enter_scope();

        // Register builtin functions needed by tests
        self.register_builtin_functions();

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
            // For this pass, we skip lowering of other item kinds (type aliases, enums, traits,
            // effects, handlers, impls, and top-level statements). They are either registered
            // during pass 1 or intentionally ignored without error.
            _ => Err(()),
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
                // Gracefully report instead of panicking
                self.errors.push(TypeError {
                    message: "Function body did not lower to a block expression".to_string(),
                    context: context.clone(),
                });
                return Err(());
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
                match lit.clone() {
                    OwnedLiteral::Unit => Ok(hir::Expr {
                        ty: hir::Ty::Special(hir::SpecialTy::Unit),
                        kind: hir::ExprKind::Block(hir::HirBlock {
                            stmts: vec![],
                            last_expr: None,
                            ty: hir::Ty::Special(hir::SpecialTy::Unit),
                        }),
                    }),
                    _ => {
                let (ty, val_str) = self.lower_literal(lit);
                Ok(hir::Expr {
                    kind: hir::ExprKind::Literal(ty.clone(), val_str),
                    ty: hir::Ty::Primitive(ty),
                })
                    }
                }
            }
            OwnedExpr::Path(path) => {
                // Look up the path in the current scope to find what it refers to.
                let name = path.last().expect("Path cannot be empty");
                match self.lookup_symbol(name) {
                    Some(Symbol::Variable { ty, .. }) => Ok(hir::Expr {
                        kind: hir::ExprKind::Path(path),
                        ty: ty.clone(),
                    }),
                    Some(Symbol::Function { signature }) => Ok(hir::Expr {
                        kind: hir::ExprKind::Path(path),
                        ty: hir::Ty::Function {
                            param_types: signature.params.iter().map(|(_, t)| t.clone()).collect(),
                            ret_type: Box::new(signature.ret_type.clone()),
                            effects: signature.effects.clone(),
                        },
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
                            message: format!("`{}` is not a value", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                }
            }
            OwnedExpr::Unary { op, rhs } => {
                let hir_rhs = self.lower_expr(*rhs, context.clone())?;
                let (result_ty, hir_op) = match op {
                    crate::ast::UnaryOp::Neg => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::I32)
                        | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                        | hir::Ty::Primitive(hir::PrimitiveTy::F64) => {
                            (hir_rhs.ty.clone(), hir::UnaryOp::Negate)
                        }
                        other => {
                            self.errors.push(TypeError {
                                message: format!("Unary '-' not supported for type {:?}", other),
                                context: context.clone(),
                            });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Negate)
                        }
                    },
                    crate::ast::UnaryOp::Not => match hir_rhs.ty.clone() {
                        hir::Ty::Primitive(hir::PrimitiveTy::Bool) => {
                            (hir::Ty::Primitive(hir::PrimitiveTy::Bool), hir::UnaryOp::Not)
                        }
                        other => {
                            self.errors.push(TypeError {
                                message: format!("Unary '!' not supported for type {:?}", other),
                                context: context.clone(),
                            });
                            (hir::Ty::Special(hir::SpecialTy::Unit), hir::UnaryOp::Not)
                        }
                    },
                };
                Ok(hir::Expr {
                    kind: hir::ExprKind::Unary {
                        op: hir_op,
                        rhs: Box::new(hir_rhs),
                    },
                    ty: result_ty,
                })
            }
            OwnedExpr::Binary { op, lhs, rhs } => {
                // Special handling: if one side is a numeric literal, coerce it to the other's type
                // for arithmetic operations to keep i32 variables intact.
                let (hir_lhs, hir_rhs) = match (&lhs.item, &rhs.item) {
                    (OwnedExpr::Literal(l), other) if self.is_numeric_literal(l) => {
                        // Lower other first
                        let other_hir = self.lower_expr(*rhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_lhs = self.lower_expr_with_expected(
                                Spanned { item: OwnedExpr::Literal(l.clone()), span: lhs.span },
                                other_hir.ty.clone(),
                                context.clone(),
                            )?;
                            (coerced_lhs, other_hir)
                        } else {
                            (self.lower_expr(*lhs, context.clone())?, other_hir)
                        }
                    }
                    (other, OwnedExpr::Literal(r)) if self.is_numeric_literal(r) => {
                        let other_hir = self.lower_expr(*lhs, context.clone())?;
                        if TypeUnifier::is_numeric(&other_hir.ty) && self.is_arithmetic_op(op) {
                            let coerced_rhs = self.lower_expr_with_expected(
                                Spanned { item: OwnedExpr::Literal(r.clone()), span: rhs.span },
                                other_hir.ty.clone(),
                                context.clone(),
                            )?;
                            (other_hir, coerced_rhs)
                        } else {
                            (other_hir, self.lower_expr(*rhs, context.clone())?)
                        }
                    }
                    _ => (
                        self.lower_expr(*lhs, context.clone())?,
                        self.lower_expr(*rhs, context.clone())?,
                    ),
                };

                // Here you'd implement type checking rules for binary operators.
                // E.g., arithmetic ops need numbers, logical ops need booleans.
                if hir_lhs.ty != hir_rhs.ty {
                    let op_kind = self.lower_binary_op(op);
                    let both_numeric = TypeUnifier::is_numeric(&hir_lhs.ty) && TypeUnifier::is_numeric(&hir_rhs.ty);
                    let is_comparison = matches!(
                        op_kind,
                        hir::BinaryOp::Eq
                            | hir::BinaryOp::Ne
                            | hir::BinaryOp::Lt
                            | hir::BinaryOp::Lte
                            | hir::BinaryOp::Gt
                            | hir::BinaryOp::Gte
                    );
                    let is_arithmetic = matches!(
                        op_kind,
                        hir::BinaryOp::Add
                            | hir::BinaryOp::Sub
                            | hir::BinaryOp::Mul
                            | hir::BinaryOp::Div
                            | hir::BinaryOp::Mod
                            | hir::BinaryOp::BitShiftLeft
                            | hir::BinaryOp::BitShiftRight
                            | hir::BinaryOp::Xor
                    );
                    if !(both_numeric && (is_comparison || is_arithmetic)) {
                    self.errors.push(TypeError {
                        message: format!(
                            "Binary operation between mismatched types: expected {} but found {}",
                            format!("{:?}", hir_lhs.ty).fg(Color::Green),
                            format!("{:?}", hir_rhs.ty).fg(Color::Red)
                        ),
                        context: context.clone(),
                    });
                    }
                }

                // Determine the result type of the expression.
                let result_ty = match self.lower_binary_op(op) {
                    hir::BinaryOp::Add
                    | hir::BinaryOp::Sub
                    | hir::BinaryOp::Mul
                    | hir::BinaryOp::Div
                    | hir::BinaryOp::Mod
                    | hir::BinaryOp::BitShiftLeft
                    | hir::BinaryOp::BitShiftRight
                    | hir::BinaryOp::Xor => TypeUnifier::unify_numeric(&hir_lhs.ty, &hir_rhs.ty).unwrap_or(hir_lhs.ty.clone()),
                    hir::BinaryOp::Assign => hir_lhs.ty.clone(),
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
            OwnedExpr::FieldAccess { receiver, field } => {
                let recv_hir = self.lower_expr(*receiver, context.clone())?;
                // Try to find struct fields if receiver has struct type
                let field_ty = match recv_hir.ty.clone() {
                    hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) => {
                        self.lookup_struct_field_type(&name, &field).cloned()
                    }
                    _ => None,
                };
                match field_ty {
                    Some(ty) => Ok(hir::Expr {
                        kind: hir::ExprKind::FieldAccess {
                            receiver: Box::new(recv_hir),
                            field,
                        },
                        ty,
                    }),
                    None => {
                        self.errors.push(TypeError {
                            message: "Unknown field access on non-record type or missing field".to_string(),
                            context: context.clone(),
                        });
                        Err(())
                    }
                }
            }
            OwnedExpr::Call { fun, args } => {
                // Prefer handling path callees specially to support functions and union variants
                match fun.item.clone() {
                    OwnedExpr::Path(path) => {
                        let name = path.last().expect("Path cannot be empty").clone();
                        // 1) Function call
                        let signature_opt = match self.lookup_symbol(&name) {
                            Some(Symbol::Function { signature }) => Some(signature.clone()),
                            _ => None,
                        };
                        if let Some(signature) = signature_opt {
                            // Lower args
                            let mut lowered_args = Vec::new();
                            for (idx, arg) in args.into_iter().enumerate() {
                                let arg_expr = self.lower_expr(arg, context.clone())?;
                                if let Some((_, expected)) = signature.params.get(idx) {
                                    if &arg_expr.ty != expected {
                                        self.errors.push(TypeError {
                                            message: format!(
                                                "Argument {} type mismatch: expected {:?}, found {:?}",
                                                idx + 1,
                                                expected,
                                                arg_expr.ty
                                            ),
                                            context: context.clone(),
                                        });
                                    }
                                }
                                lowered_args.push(arg_expr);
                            }
                            if lowered_args.len() != signature.params.len() {
                                self.errors.push(TypeError {
                                    message: format!(
                                        "Function `{}` expects {} args, found {}",
                                        signature.name,
                                        signature.params.len(),
                                        lowered_args.len()
                                    ),
                                    context: context.clone(),
                                });
                            }
                            let fun_expr = hir::Expr {
                                kind: hir::ExprKind::Path(path),
                                ty: hir::Ty::Function {
                                    param_types: signature
                                        .params
                                        .iter()
                                        .map(|(_, t)| t.clone())
                                        .collect(),
                                    ret_type: Box::new(signature.ret_type.clone()),
                                    effects: signature.effects.clone(),
                                },
                            };
                            return Ok(hir::Expr {
                                ty: signature.ret_type.clone(),
                                kind: hir::ExprKind::Call {
                                    fun: Box::new(fun_expr),
                                    args: lowered_args,
                                },
                            });
                        }

                        // 2) Union variant constructor: Variant(payload) -> Union
                        if let Some((union_path, payload_types)) =
                            self.find_union_variant(&name)
                        {
                            let ret_ty = hir::Ty::Adt(hir::AdtTy::Enum {
                                name: union_path.clone(),
                                generics: vec![],
                            });

                            // Expect at most one payload for tests
                            let expected_payload_ty = payload_types
                                .as_ref()
                                .and_then(|v| v.get(0))
                                .cloned();

                            let mut lowered_args = Vec::new();
                            if let Some(exp_ty) = expected_payload_ty.clone() {
                                if let Some(first) = args.get(0) {
                                    // Special-case record literals in argument position
                                    let lowered = self.lower_expr_with_expected(
                                        first.clone(),
                                        exp_ty.clone(),
                                        context.clone(),
                                    )?;
                                    lowered_args.push(lowered);
                                } else {
                                    self.errors.push(TypeError {
                                        message: format!(
                                            "Variant `{}` expects 1 argument",
                                            name
                                        ),
                                        context: context.clone(),
                                    });
                                }
                            }

                            let fun_expr = hir::Expr {
                                kind: hir::ExprKind::Path(vec![name]),
                                ty: hir::Ty::Function {
                                    param_types: expected_payload_ty
                                        .map(|t| vec![t])
                                        .unwrap_or_default(),
                                    ret_type: Box::new(ret_ty.clone()),
                                    effects: vec![],
                                },
                            };

                            return Ok(hir::Expr {
                                ty: ret_ty,
                                kind: hir::ExprKind::Call {
                                    fun: Box::new(fun_expr),
                                    args: lowered_args,
                                },
                            });
                        }

                        // Unknown callee
                        self.errors.push(TypeError {
                            message: format!("Unknown function or constructor `{}`", name),
                            context: context.clone(),
                        });
                        Err(())
                    }
                    _ => {
                        // Fallback: lower callee normally, then check if it is a function type
                        let fun_hir = self.lower_expr(*fun, context.clone())?;
                        let params = match &fun_hir.ty {
                            hir::Ty::Function { param_types, .. } => param_types.clone(),
                            other => {
                                self.errors.push(TypeError {
                                    message: format!(
                                        "Attempted to call a non-function value of type {:?}",
                                        other
                                    ),
                                    context: context.clone(),
                                });
                                vec![]
                            }
                        };
                        let mut lowered_args = Vec::new();
                        for (idx, arg) in args.into_iter().enumerate() {
                            let arg_expr = self.lower_expr(arg, context.clone())?;
                            if let Some(expected) = params.get(idx) {
                                if &arg_expr.ty != expected {
                                    self.errors.push(TypeError {
                                        message: format!(
                                            "Argument {} type mismatch: expected {:?}, found {:?}",
                                            idx + 1,
                                            expected,
                                            arg_expr.ty
                                        ),
                                        context: context.clone(),
                                    });
                                }
                            }
                            lowered_args.push(arg_expr);
                        }
                        let ret_ty = match &fun_hir.ty {
                            hir::Ty::Function { ret_type, .. } => (*ret_type.clone()).clone(),
                            _ => hir::Ty::Special(hir::SpecialTy::Unit),
                        };
                        Ok(hir::Expr {
                            ty: ret_ty,
                            kind: hir::ExprKind::Call {
                                fun: Box::new(fun_hir),
                                args: lowered_args,
                            },
                        })
                    }
                }
            }
            OwnedExpr::Block { stmts, last_expr } => {
                self.enter_scope();
                let mut hir_stmts: Vec<hir::Stmt> = Vec::new();
                for s in stmts.into_iter() {
                    if let Ok(stmt) = self.lower_stmt(s, context.clone()) {
                        hir_stmts.push(stmt);
                    }
                }

                // If no explicit last_expr, but last stmt is Expr, treat it as the block value
                let mut inferred_last_expr: Option<Box<hir::Expr>> = None;
                if last_expr.is_none() {
                    if let Some(hir::Stmt::Expr(e)) = hir_stmts.last() {
                        inferred_last_expr = Some(Box::new(e.clone()));
                        hir_stmts.pop();
                    }
                }

                let (hir_last_expr, block_ty) = match (last_expr, inferred_last_expr) {
                    (Some(expr), _) => {
                        let hir_expr = self.lower_expr(*expr, context.clone())?;
                        let ty = hir_expr.ty.clone();
                        (Some(Box::new(hir_expr)), ty)
                    }
                    (None, Some(inf)) => {
                        let ty = inf.ty.clone();
                        (Some(inf), ty)
                    }
                    (None, None) => (None, hir::Ty::Special(hir::SpecialTy::Unit)),
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
            OwnedExpr::If {
                cond,
                then_block,
                else_block,
            } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                self.errors.push(TypeError {
                        message: "If condition must be a bool".to_string(),
                        context: context.clone(),
                    });
                }
                let then_hir = self.lower_expr(*then_block, context.clone())?;
                let else_hir_opt = if let Some(e) = else_block {
                    Some(Box::new(self.lower_expr(*e, context.clone())?))
                } else {
                    None
                };
                let then_block = match then_hir.kind.clone() {
                    hir::ExprKind::Block(b) => b,
                    _ => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(then_hir.clone())),
                        ty: then_hir.ty.clone(),
                    },
                };
                let result_ty = if let Some(ref else_hir) = else_hir_opt {
                    if then_hir.ty != else_hir.ty {
                        self.errors.push(TypeError {
                            message: format!(
                                "If branches must have same type: then={:?}, else={:?}",
                                then_hir.ty, else_hir.ty
                            ),
                            context: context.clone(),
                        });
                    }
                    else_hir.ty.clone()
                } else {
                    hir::Ty::Special(hir::SpecialTy::Unit)
                };
                Ok(hir::Expr {
                    ty: result_ty,
                    kind: hir::ExprKind::If {
                        cond: Box::new(cond_hir),
                        then_block,
                        else_block: else_hir_opt,
                    },
                })
            }
            OwnedExpr::While { cond, body } => {
                let cond_hir = self.lower_expr(*cond, context.clone())?;
                if cond_hir.ty != hir::Ty::Primitive(hir::PrimitiveTy::Bool) {
                    self.errors.push(TypeError {
                        message: "While condition must be a bool".to_string(),
                        context: context.clone(),
                    });
                }
                let body_hir = self.lower_expr(*body, context.clone())?;
                let body_block = match body_hir.kind {
                    hir::ExprKind::Block(b) => b,
                    _ => hir::HirBlock {
                        stmts: vec![],
                        last_expr: Some(Box::new(body_hir.clone())),
                        ty: body_hir.ty.clone(),
                    },
                };
                Ok(hir::Expr {
                    ty: hir::Ty::Special(hir::SpecialTy::Unit),
                    kind: hir::ExprKind::While {
                        cond: Box::new(cond_hir),
                        body: body_block,
                    },
                })
            }
            OwnedExpr::Match { scrutinee, arms } => {
                let scrutinee_hir = self.lower_expr(*scrutinee, context.clone())?;
                // Lower arms with individual scopes
                let mut lowered_arms = Vec::new();
                let mut result_ty: Option<hir::Ty> = None;
                for (pat, arm_expr) in arms {
                    self.enter_scope();
                    let (hir_pat, hir_arm_expr) =
                        self.lower_match_arm(pat, arm_expr, &scrutinee_hir.ty, context.clone())?;
                    if let Some(ref ty) = result_ty {
                        if *ty != hir_arm_expr.ty {
                            self.errors.push(TypeError {
                                message: format!(
                                    "Match arms must have the same type: expected {:?}, found {:?}",
                                    ty, hir_arm_expr.ty
                                ),
                                context: context.clone(),
                            });
                        }
                    } else {
                        result_ty = Some(hir_arm_expr.ty.clone());
                    }
                    lowered_arms.push((hir_pat, hir_arm_expr));
                    self.leave_scope();
                }
                let result_ty = result_ty.unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                Ok(hir::Expr {
                    ty: result_ty,
                    kind: hir::ExprKind::Match {
                        scrutinee: Box::new(scrutinee_hir),
                        arms: lowered_arms,
                    },
                })
            }
            OwnedExpr::Perform { path, args } => {
                // Resolve effect operation return type if possible
                let (ret_ty, _param_tys) = self
                    .resolve_effect_op(&path)
                    .unwrap_or((hir::Ty::Special(hir::SpecialTy::Unit), vec![]));

                // Lower args in a relaxed mode: do not error on unknown paths (e.g., local fn)
                let mut lowered_args = Vec::new();
                for a in args {
                    match self.lower_expr(a.clone(), context.clone()) {
                        Ok(e) => lowered_args.push(e),
                        Err(_) => lowered_args.push(hir::Expr {
                            kind: hir::ExprKind::Error,
                            ty: hir::Ty::Generic("_unknown".to_string()),
                        }),
                    }
                }
                Ok(hir::Expr {
                    ty: ret_ty,
                    kind: hir::ExprKind::Perform { path, args: lowered_args },
                })
            }
            OwnedExpr::Handle { body, handler } => {
                let body_hir = self.lower_expr(*body, context.clone())?;
                let handler_hir = match handler {
                    OwnedHandlerBody::Path(p) => hir::HirHandlerBody::Path(p),
                    OwnedHandlerBody::Inline(funcs) => {
                        let mut lowered = Vec::new();
                        for f in funcs {
                            if let Ok(h) = self.lower_function(
                                f,
                                ItemContext { span: expr.span, path: context.path.clone() },
                            ) {
                                lowered.push(h);
                            }
                        }
                        hir::HirHandlerBody::Inline(lowered)
                    }
                };
                // For now, the handled expression has the same type as the body
                Ok(hir::Expr {
                    ty: body_hir.ty.clone(),
                    kind: hir::ExprKind::Handle {
                        body: match body_hir.kind.clone() {
                            hir::ExprKind::Block(b) => b,
                            _ => hir::HirBlock {
                                stmts: vec![],
                                last_expr: Some(Box::new(body_hir.clone())),
                                ty: body_hir.ty.clone(),
                            },
                        },
                        handler: handler_hir,
                    },
                })
            }
            OwnedExpr::Cast { expr: inner, ty } => {
                let inner_hir = self.lower_expr(*inner, context.clone())?;
                let target_ty = self.resolve_type(&ty, context.clone())?;
                Ok(hir::Expr {
                    ty: target_ty.clone(),
                    kind: hir::ExprKind::Cast {
                        expr: Box::new(inner_hir),
                    },
                })
            }
            OwnedExpr::StructInit { path, fields, .. } => {
                // Treat as nominal struct init; check fields
                let adt_ty = hir::Ty::Adt(hir::AdtTy::Struct {
                    name: path.clone(),
                    generics: vec![],
                });
                let mut lowered_fields = Vec::new();
                for (name, expr) in fields {
                    let e = self.lower_expr(expr, context.clone())?;
                    lowered_fields.push((name, e));
                }
                Ok(hir::Expr {
                    ty: adt_ty.clone(),
                    kind: hir::ExprKind::StructInit {
                        path,
                        fields: lowered_fields,
                    },
                })
            }
            OwnedExpr::Array(_) | OwnedExpr::Map(_) => {
                // For this pass, arrays/maps are not needed except when used as record literal.
                // We cannot infer record literal type without annotation; error gracefully.
                self.errors.push(TypeError {
                    message: "Cannot infer type for map/record literal without annotation".to_string(),
                    context: context.clone(),
                });
                Err(())
            }
            OwnedExpr::Error => Ok(hir::Expr { kind: hir::ExprKind::Error, ty: hir::Ty::Special(hir::SpecialTy::Unit) }),
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
                // If annotated with a record type and value is a record literal (Map),
                // lower as struct init with that type.
                let (hir_value, var_ty) = if let Some(annotated_ty) = ty.clone() {
                    let resolved_ty = self.resolve_type(&annotated_ty, context.clone())?;
                    match (&resolved_ty, &value.item) {
                        (
                            hir::Ty::Adt(hir::AdtTy::Struct { name: struct_path, .. }),
                            OwnedExpr::Map(entries),
                        ) => {
                            // Lower each field
                            let mut lowered_fields = Vec::new();
                            for (k_expr, v_expr) in entries.clone() {
                                // Key is a string literal by construction
                                let key = match k_expr.item.clone() {
                                    OwnedExpr::Literal(OwnedLiteral::Str(s)) => s,
                                    _ => "<key>".to_string(),
                                };
                                // Try to coerce value to expected field type if known
                                let v_expected_ty = self
                                    .lookup_struct_field_type(struct_path, &key)
                                    .cloned();
                                let v = if let Some(exp_ty) = v_expected_ty {
                                    self.lower_expr_with_expected(v_expr, exp_ty, context.clone())?
                                } else {
                                    self.lower_expr(v_expr, context.clone())?
                                };
                                lowered_fields.push((key, v));
                            }
                            let init_expr = hir::Expr {
                                ty: resolved_ty.clone(),
                                kind: hir::ExprKind::StructInit {
                                    path: struct_path.clone(),
                                    fields: lowered_fields,
                                },
                            };
                            (init_expr, resolved_ty.clone())
                        }
                        _ => {
                            let mut lowered = self.lower_expr(value, context.clone())?;
                            // If numeric mismatch, allow coercion to annotated type
                            if lowered.ty != resolved_ty {
                                if TypeUnifier::is_numeric(&lowered.ty) && TypeUnifier::is_numeric(&resolved_ty) {
                                    lowered = hir::Expr {
                                        ty: resolved_ty.clone(),
                                        kind: hir::ExprKind::Cast { expr: Box::new(lowered) },
                                    };
                                } else {
                        self.errors.push(TypeError {
                            message: format!(
                                "Mismatched types for variable '{}': expected {} but found {}",
                                name,
                                format!("{:?}", resolved_ty).fg(Color::Green),
                                            format!("{:?}", lowered.ty).fg(Color::Red)
                            ),
                            context: context.clone(),
                        });
                    }
                            }
                            (lowered, resolved_ty.clone())
                        }
                    }
                } else {
                    let lowered = self.lower_expr(value, context.clone())?;
                    (lowered.clone(), lowered.ty.clone())
                };

                // Add the new variable to the current scope.
                let symbol = Symbol::Variable {
                    ty: var_ty.clone(),
                    is_mut,
                };
                self.add_symbol_to_current_scope(name.clone(), symbol);

                Ok(hir::Stmt::Let { name, value: hir_value, ty: var_ty, is_mut })
            }
            OwnedStmt::Assign(lhs, rhs) => {
                let lhs_hir = self.lower_expr(lhs, context.clone())?;
                let rhs_hir = self.lower_expr(rhs, context.clone())?;
                if lhs_hir.ty != rhs_hir.ty {
                self.errors.push(TypeError {
                        message: format!(
                            "Assignment type mismatch: lhs={:?}, rhs={:?}",
                            lhs_hir.ty, rhs_hir.ty
                        ),
                    context: context.clone(),
                });
                }
                Ok(hir::Stmt::Assign(lhs_hir, rhs_hir))
            }
            OwnedStmt::Return(expr_opt) => {
                let expr_hir_opt = if let Some(e) = expr_opt {
                    Some(self.lower_expr(e, context.clone())?)
                } else {
                    None
                };
                if let Some(expected) = &self.current_fn_return_type {
                    let actual = expr_hir_opt
                        .as_ref()
                        .map(|e| e.ty.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    if &actual != expected {
                        self.errors.push(TypeError {
                            message: format!(
                                "Return type mismatch: expected {:?}, found {:?}",
                                expected, actual
                            ),
                            context: context.clone(),
                        });
                    }
                }
                Ok(hir::Stmt::Return(expr_hir_opt))
            }
            OwnedStmt::Expr(e) => {
                let e_hir = self.lower_expr(e, context.clone())?;
                Ok(hir::Stmt::Expr(e_hir))
            }
            OwnedStmt::Error => Ok(hir::Stmt::Error),
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
                // Try to resolve against registered type definitions
                let path_vec = owned_ty.path.clone();
                if let Some(item) = self.type_definitions.get(&path_vec) {
                    match item {
                        hir::Item::Struct(_) => Ok(hir::Ty::Adt(hir::AdtTy::Struct {
                            name: path_vec,
                            generics: vec![],
                        })),
                        hir::Item::Enum(_) => Ok(hir::Ty::Adt(hir::AdtTy::Enum {
                            name: path_vec,
                            generics: vec![],
                        })),
                        hir::Item::Effect(_) => Ok(hir::Ty::Adt(hir::AdtTy::Effect {
                            name: path_vec,
                            generics: vec![],
                        })),
                        _ => {
                            self.errors.push(TypeError {
                                message: format!("Unsupported type item for {}", type_name),
                                context: context.clone(),
                            });
                            Err(())
                        }
                    }
                } else if owned_ty.path.len() == 1 {
                    // Treat single-segment unknown types as generics, e.g., T
                    Ok(hir::Ty::Generic(owned_ty.path[0].clone()))
                } else {
                self.errors.push(TypeError {
                    message: format!("Unknown type: {}", type_name),
                    context: context.clone(),
                });
                Err(())
                }
            }
        }
    }

    /// Pass 1: Registers an item in the global scope/type map.
    fn register_top_level_item(&mut self, item: &OwnedItemWithSpan) {
        match &item.item {
            OwnedItem::Fn(func) => {
                // Build function signature and register as a symbol in the global scope
                let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                let mut params = Vec::new();
                let mut has_error = false;
                for (name_opt, ty) in &func.params {
                    match self.resolve_type(ty, ctx.clone()) {
                        Ok(t) => params.push((name_opt.clone().unwrap_or("_".to_string()), t)),
                        Err(_) => has_error = true,
                    }
                }
                let ret_ty = match &func.ret_type {
                    Some(rt) => self.resolve_type(rt, ctx.clone()).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)),
                    None => hir::Ty::Special(hir::SpecialTy::Unit),
                };
                if !has_error {
                    let signature = hir::HirFunctionSignature {
                        name: func.name.clone(),
                        params,
                        ret_type: ret_ty,
                        effects: vec![],
                    };
                    self.add_symbol_to_current_scope(
                        func.name.clone(),
                        Symbol::Function { signature },
                    );
                }
            }
            OwnedItem::TypeAlias(ta) => {
                // Register record and union shapes as struct/enum HIR items
                match &ta.aliased {
                    OwnedTypeAliasBody::Record(fields) => {
                        let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                        let mut lowered_fields = Vec::new();
                        for (name, ty) in fields {
                            if let Ok(t) = self.resolve_type(ty, ctx.clone()) {
                                lowered_fields.push((name.clone(), t));
                            }
                        }
                        let def = hir::HirStructDef { name: ta.name.clone(), fields: lowered_fields, is_public: ta.is_public };
                        self.type_definitions
                            .insert(vec![ta.name.clone()], hir::Item::Struct(def));
                    }
                    OwnedTypeAliasBody::Union(variants) => {
                        let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                        let mut lowered_variants: Vec<(String, Option<Vec<hir::Ty>>)> = Vec::new();
                        for (vname, payload) in variants {
                            let lowered_payload = match payload {
                                Some(t) => match self.resolve_type(t, ctx.clone()) {
                                    Ok(h) => Some(vec![h]),
                                    Err(_) => None,
                                },
                                None => None,
                            };
                            lowered_variants.push((vname.clone(), lowered_payload));
                        }
                        let def = hir::HirEnumDef { name: ta.name.clone(), variants: lowered_variants.clone(), is_public: ta.is_public };
                        let path = vec![ta.name.clone()];
                        self.type_definitions
                            .insert(path.clone(), hir::Item::Enum(def));
                        self.union_variants.insert(path, lowered_variants);
                    }
                    OwnedTypeAliasBody::Type(_) => {
                        // For now, simple aliases are ignored for shape/nominal typing in this pass.
                    }
                }
            }
            OwnedItem::Effect(eff) => {
                // Register effect operations for perform typing
                let ctx = ItemContext { span: item.span, path: PathBuf::from("<global>") };
                let mut ops = Vec::new();
                for op in &eff.operations {
                    let mut params = Vec::new();
                    for p in &op.params {
                        if let Ok(t) = self.resolve_type(p, ctx.clone()) {
                            params.push(("_".to_string(), t));
                        }
                    }
                    let ret_ty = self
                        .resolve_type(&op.ret_type, ctx.clone())
                        .unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit));
                    ops.push(hir::HirFunctionSignature { name: op.name.clone(), params, ret_type: ret_ty, effects: vec![] });
                }
                let def = hir::HirEffectDef { name: eff.name.clone(), operations: ops, is_public: eff.is_public };
                self.type_definitions
                    .insert(vec![eff.name.clone()], hir::Item::Effect(def));
            }
            // Ignore others in registration for this pass
            _ => {}
        }
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
            // Default integer literal to i32 for this pass
            OwnedLiteral::I64(i) => (hir::PrimitiveTy::I32, i.to_string()),
            OwnedLiteral::F64(f) => (hir::PrimitiveTy::F64, f.to_string()),
            OwnedLiteral::Str(s) => (hir::PrimitiveTy::Str, s),
            OwnedLiteral::Unit => (hir::PrimitiveTy::Bool, "false".to_string()), // placeholder; unit literal should be handled at expr level
            _ => (hir::PrimitiveTy::I32, "0".to_string()),
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

    // --- Additional helpers ---

    fn register_builtin_functions(&mut self) {
        // len(str) -> i32
        let signature = hir::HirFunctionSignature {
            name: "len".to_string(),
            params: vec![("s".to_string(), hir::Ty::Primitive(hir::PrimitiveTy::Str))],
            ret_type: hir::Ty::Primitive(hir::PrimitiveTy::I32),
            effects: vec![],
        };
        self.add_symbol_to_current_scope("len".to_string(), Symbol::Function { signature });
    }

    fn is_numeric_type(&self, ty: &hir::Ty) -> bool {
        matches!(
            ty,
            hir::Ty::Primitive(hir::PrimitiveTy::I32)
                | hir::Ty::Primitive(hir::PrimitiveTy::I64)
                | hir::Ty::Primitive(hir::PrimitiveTy::F64)
        )
    }

    fn is_numeric_literal(&self, lit: &OwnedLiteral) -> bool {
        matches!(lit, OwnedLiteral::I32(_) | OwnedLiteral::I64(_) | OwnedLiteral::F64(_))
    }

    fn is_arithmetic_op(&self, op: BinaryOp) -> bool {
        matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod
        )
    }

    fn unify_numeric_types(&self, a: hir::Ty, b: hir::Ty) -> Option<hir::Ty> {
        use hir::PrimitiveTy::*;
        use hir::Ty::*;
        match (a, b) {
            (Primitive(I64), Primitive(I32)) | (Primitive(I32), Primitive(I64)) => Some(Primitive(I64)),
            (Primitive(F64), Primitive(I32))
            | (Primitive(I32), Primitive(F64))
            | (Primitive(F64), Primitive(I64))
            | (Primitive(I64), Primitive(F64)) => Some(Primitive(F64)),
            (Primitive(I32), Primitive(I32)) => Some(Primitive(I32)),
            (Primitive(I64), Primitive(I64)) => Some(Primitive(I64)),
            (Primitive(F64), Primitive(F64)) => Some(Primitive(F64)),
            (x, y) if x == y => Some(x),
            _ => None,
        }
    }

    fn lookup_struct_field_type(
        &self,
        path: &hir::OwnedPath,
        field: &str,
    ) -> Option<&hir::Ty> {
        match self.type_definitions.get(path) {
            Some(hir::Item::Struct(def)) => def
                .fields
                .iter()
                .find(|(n, _)| n == field)
                .map(|(_, t)| t),
            _ => None,
        }
    }

    fn find_union_variant(
        &self,
        variant: &str,
    ) -> Option<(hir::OwnedPath, Option<Vec<hir::Ty>>)> {
        for (union_path, variants) in &self.union_variants {
            if let Some((_, payload)) = variants
                .iter()
                .find(|(name, _)| name == variant)
                .cloned()
            {
                return Some((union_path.clone(), payload));
            }
        }
        None
    }

    fn lower_expr_with_expected(
        &mut self,
        expr: SpannedExpr,
        expected: hir::Ty,
        context: ItemContext,
    ) -> Result<hir::Expr, ()> {
        match (expr.item.clone(), expected.clone()) {
            (OwnedExpr::Map(entries), hir::Ty::Adt(hir::AdtTy::Struct { name, .. })) => {
                let mut lowered_fields = Vec::new();
                for (k_expr, v_expr) in entries {
                    let key = match k_expr.item {
                        OwnedExpr::Literal(OwnedLiteral::Str(s)) => s,
                        _ => "<key>".to_string(),
                    };
                    // Find expected field type for better checking
                    let field_expected = self.lookup_struct_field_type(&name, &key).cloned();
                    let v = if let Some(exp) = field_expected {
                        self.lower_expr_with_expected(v_expr, exp, context.clone())?
                    } else {
                        self.lower_expr(v_expr, context.clone())?
                    };
                    lowered_fields.push((key, v));
                }
                Ok(hir::Expr {
                    ty: expected.clone(),
                    kind: hir::ExprKind::StructInit {
                        path: name,
                        fields: lowered_fields,
                    },
                })
            }
            (OwnedExpr::Literal(lit), hir::Ty::Primitive(exp_prim)) => {
                if let Some((pty, s)) = self.coerce_numeric_literal(&lit, exp_prim.clone()) {
                    return Ok(hir::Expr { ty: hir::Ty::Primitive(pty.clone()), kind: hir::ExprKind::Literal(pty, s) });
                }
                // Fallback
                self.lower_expr(Spanned { item: OwnedExpr::Literal(lit), span: expr.span }, context)
            }
            _ => self.lower_expr(expr, context),
        }
    }

    fn coerce_numeric_literal(
        &self,
        lit: &OwnedLiteral,
        expected: hir::PrimitiveTy,
    ) -> Option<(hir::PrimitiveTy, String)> {
        use hir::PrimitiveTy::*;
        match (lit, expected) {
            (OwnedLiteral::I32(v), I32) => Some((I32, v.to_string())),
            (OwnedLiteral::I64(v), I64) => Some((I64, v.to_string())),
            (OwnedLiteral::F64(v), F64) => Some((F64, v.to_string())),
            (OwnedLiteral::I64(v), I32) => Some((I32, v.to_string())),
            (OwnedLiteral::I32(v), I64) => Some((I64, v.to_string())),
            (OwnedLiteral::I32(v), F64) => Some((F64, (*v as f64).to_string())),
            (OwnedLiteral::I64(v), F64) => Some((F64, (*v as f64).to_string())),
            (OwnedLiteral::F64(v), I32) => Some((I32, (*v as i32).to_string())),
            (OwnedLiteral::F64(v), I64) => Some((I64, (*v as i64).to_string())),
            _ => None,
        }
    }

    fn resolve_effect_op(
        &self,
        path: &hir::OwnedPath,
    ) -> Option<(hir::Ty, Vec<hir::Ty>)> {
        if path.len() != 2 {
            return None;
        }
        let effect_name = vec![path[0].clone()];
        let op_name = &path[1];
        match self.type_definitions.get(&effect_name) {
            Some(hir::Item::Effect(def)) => {
                for sig in &def.operations {
                    if &sig.name == op_name {
                        return Some((sig.ret_type.clone(), sig.params.iter().map(|(_, t)| t.clone()).collect()));
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn lower_match_arm(
        &mut self,
        pat: SpannedPattern,
        expr: SpannedExpr,
        scrutinee_ty: &hir::Ty,
        context: ItemContext,
    ) -> Result<(hir::HirPattern, hir::Expr), ()> {
        // Lower pattern first and bind variables into current scope
        let (hir_pat, bound_types): (hir::HirPattern, Vec<(String, hir::Ty)>) = match pat.item {
            OwnedPattern::Wildcard => (
                hir::HirPattern { kind: hir::HirPatternKind::Wildcard, ty: scrutinee_ty.clone() },
                vec![],
            ),
            OwnedPattern::Identifier(name) => {
                // Bind identifier with scrutinee type
                (hir::HirPattern { kind: hir::HirPatternKind::Identifier(name.clone()), ty: scrutinee_ty.clone() }, vec![(name, scrutinee_ty.clone())])
            }
            OwnedPattern::Path { path, args } => {
                let variant_name = path.last().cloned().unwrap_or_default();
                // Expect enum scrutinee
                let (union_path, payload) = match scrutinee_ty {
                    hir::Ty::Adt(hir::AdtTy::Enum { name, .. }) => {
                        // Find variant on this union
                        let mut found: Option<(hir::OwnedPath, Option<Vec<hir::Ty>>)> = None;
                        if let Some(vs) = self.union_variants.get(name) {
                            for (vn, pl) in vs {
                                if vn == &variant_name {
                                    found = Some((name.clone(), pl.clone()));
                                    break;
                                }
                            }
                        }
                        found.unwrap_or((name.clone(), None))
                    }
                    _ => (vec![], None),
                };
                let mut bound = Vec::new();
                let mut subpatterns = Vec::new();
                if let Some(pl) = payload.clone() {
                    // Only support single payload case
                    if let Some(first) = pl.get(0) {
                        if let Some(arg0) = args.get(0) {
                            match &arg0.item {
                                OwnedPattern::Identifier(n) => {
                                    bound.push((n.clone(), first.clone()));
                                    subpatterns.push(hir::HirPattern { kind: hir::HirPatternKind::Identifier(n.clone()), ty: first.clone() });
                                }
                                _ => {
                                    subpatterns.push(hir::HirPattern { kind: hir::HirPatternKind::Wildcard, ty: first.clone() });
                                }
                            }
                        }
                    }
                }
                (
                    hir::HirPattern { kind: hir::HirPatternKind::Path { path: vec![variant_name], args: subpatterns }, ty: scrutinee_ty.clone() },
                    bound,
                )
            }
            OwnedPattern::Literal(lit) => {
                let (pty, s) = self.lower_literal(lit);
                (
                    hir::HirPattern { kind: hir::HirPatternKind::Literal(pty.clone(), s), ty: hir::Ty::Primitive(pty) },
                    vec![],
                )
            }
        };
        // Insert bindings
        for (name, ty) in &bound_types {
            self.add_symbol_to_current_scope(name.clone(), Symbol::Variable { ty: ty.clone(), is_mut: false });
        }
        let arm_expr = self.lower_expr(expr, context)?;
        Ok((hir_pat, arm_expr))
    }
}
