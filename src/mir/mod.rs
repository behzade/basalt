//! src/mir/mod.rs
//!
//! This module handles the lowering of the Hierarchical Intermediate Representation (HIR)
//! into the Mid-level Intermediate Representation (MIR).

mod builder;
mod data;

use crate::{hir, hir::Ty, ast};
use builder::MirBuilder;
use data::*;
use std::collections::HashMap;

/// The main struct responsible for the HIR -> MIR lowering process.
pub struct MirLowerer<'src> {
    hir_program: &'src [hir::Item<'src>],
    // You can add context here if needed, e.g., for resolving trait methods.
}

impl<'src> MirLowerer<'src> {
    /// Creates a new `MirLowerer`.
    pub fn new(hir_program: &'src [hir::Item<'src>]) -> Self {
        Self { hir_program }
    }

    /// The main entry point for lowering the entire HIR program to MIR.
    pub fn lower_to_mir(self) -> MirProgram<'src> {
        let mut functions = HashMap::new();

        for item in self.hir_program {
            if let hir::Item::Fn(func) = item {
                let mir_func = self.lower_function(func);
                functions.insert(func.name, mir_func);
            }
        }

        MirProgram { functions }
    }

    /// Lowers a single HIR function to a `MirFunction`.
    fn lower_function(&self, func: &'src hir::Function<'src>) -> MirFunction<'src> {
        let mut builder = MirBuilder::new();
        let mut hir_to_mir_locals = HashMap::new(); // Maps HIR variable names to MIR LocalId

        // Create a dedicated return local (`_0`) first.
        let return_local = builder.new_local(func.ret_type.clone(), false);
        assert_eq!(return_local.0, 0);

        // Create MIR locals for function parameters.
        let mut param_locals = Vec::new();
        for (name_opt, ty) in &func.params {
            let local_id = builder.new_local(ty.clone(), true);
            param_locals.push(local_id);
            if let Some(name) = name_opt {
                hir_to_mir_locals.insert(*name, local_id);
            }
        }

        // Lower the function body.
        self.lower_expr(
            &func.body,
            &mut builder,
            &mut hir_to_mir_locals,
            // The result of the function body will be stored in the return place (`_0`)
            Place { local: LocalId(0) },
        );

        // Terminate the last block with a Return.
        builder.set_terminator(Terminator::Return);

        builder.build(func.name, param_locals, func.ret_type.clone())
    }

    /// Lowers an HIR expression into a series of MIR statements and terminators.
    /// The result of the expression is placed in `destination`.
    fn lower_expr(
        &self,
        expr: &'src hir::Expr<'src>,
        builder: &mut MirBuilder<'src>,
        locals: &mut HashMap<&'src str, LocalId>,
        destination: Place,
    ) {
        match &expr.kind {
            hir::ExprKind::Literal(lit) => {
                let rvalue = Rvalue::Use(Operand::Constant(lit.clone()));
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::Path(path) => {
                let name = path.first().expect("Path cannot be empty");
                let local_id = locals
                    .get(name)
                    .expect("Local variable not found in MIR context");
                let rvalue = Rvalue::Use(Operand::Copy(Place { local: *local_id }));
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                // Lower LHS into a temporary local.
                let lhs_temp = builder.new_local(lhs.ty.clone(), false);
                self.lower_expr(lhs, builder, locals, Place { local: lhs_temp });

                // Lower RHS into another temporary local.
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });

                // Create the binary operation Rvalue.
                let rvalue = Rvalue::BinaryOp(
                    *op,
                    Operand::Copy(Place { local: lhs_temp }),
                    Operand::Copy(Place { local: rhs_temp }),
                );
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::Unary { op, rhs } => {
                // Lower RHS into a temporary local.
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });

                // Create the unary operation Rvalue.
                let rvalue = Rvalue::UnaryOp(*op, Operand::Copy(Place { local: rhs_temp }));
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                // Create blocks for then, else, and the merge point after the if.
                let then_bb = builder.new_basic_block();
                let else_bb = builder.new_basic_block();
                let merge_bb = builder.new_basic_block();

                // Lower the condition.
                let cond_temp = builder.new_local(cond.ty.clone(), false);
                self.lower_expr(cond, builder, locals, Place { local: cond_temp });

                // Terminate the current block with a conditional switch.
                builder.set_terminator(Terminator::SwitchInt {
                    discr: Operand::Copy(Place { local: cond_temp }),
                    targets: vec![(1, then_bb)], // if true (1), go to then_bb
                    otherwise: else_bb,
                });

                // Lower the `then` block.
                builder.switch_to_block(then_bb);
                self.lower_expr(then_block, builder, locals, destination.clone());
                builder.set_terminator(Terminator::Goto { target: merge_bb });

                // Lower the `else` block.
                builder.switch_to_block(else_bb);
                if let Some(else_expr) = else_block {
                    self.lower_expr(else_expr, builder, locals, destination);
                }
                builder.set_terminator(Terminator::Goto { target: merge_bb });

                // Continue building from the merge block.
                builder.switch_to_block(merge_bb);
            }
            hir::ExprKind::Block { stmts, last_expr } => {
                // Lower all statements in the block.
                for stmt in stmts {
                    self.lower_stmt(stmt, builder, locals);
                }

                // Lower the last expression if present.
                if let Some(expr) = last_expr {
                    self.lower_expr(expr, builder, locals, destination);
                } else {
                                    // If no last expression, assign unit value.
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                builder.push_statement(Statement::Assign(destination, rvalue));
                }
            }
            hir::ExprKind::Call { fun, args } => {
                // Lower all arguments into temporary locals.
                let mut arg_operands = Vec::new();
                for arg in args {
                    let arg_temp = builder.new_local(arg.ty.clone(), false);
                    self.lower_expr(arg, builder, locals, Place { local: arg_temp });
                    arg_operands.push(Operand::Copy(Place { local: arg_temp }));
                }

                // Create a new block for after the call.
                let after_call_bb = builder.new_basic_block();

                // Terminate current block with the call.
                builder.set_terminator(Terminator::Call {
                    func: match &**fun {
                        hir::Expr { kind: hir::ExprKind::Path(path), .. } => {
                            path.first().expect("Function path cannot be empty")
                        }
                        _ => panic!("Expected function call to have a path expression"),
                    },
                    args: arg_operands,
                    destination: destination.clone(),
                    target: after_call_bb,
                });

                // Switch to the after-call block.
                builder.switch_to_block(after_call_bb);
            }
            hir::ExprKind::While { cond, body } => {
                // Create blocks for the loop: condition, body, and exit.
                let cond_bb = builder.new_basic_block();
                let body_bb = builder.new_basic_block();
                let exit_bb = builder.new_basic_block();

                // Jump to the condition block.
                builder.set_terminator(Terminator::Goto { target: cond_bb });

                // Lower the condition.
                builder.switch_to_block(cond_bb);
                let cond_temp = builder.new_local(cond.ty.clone(), false);
                self.lower_expr(cond, builder, locals, Place { local: cond_temp });

                // Branch based on condition.
                builder.set_terminator(Terminator::SwitchInt {
                    discr: Operand::Copy(Place { local: cond_temp }),
                    targets: vec![(1, body_bb)], // if true, go to body
                    otherwise: exit_bb,           // if false, exit loop
                });

                // Lower the body.
                builder.switch_to_block(body_bb);
                let body_temp = builder.new_local(body.ty.clone(), false);
                self.lower_expr(body, builder, locals, Place { local: body_temp });
                builder.set_terminator(Terminator::Goto { target: cond_bb }); // Loop back to condition

                // Continue from exit block.
                builder.switch_to_block(exit_bb);
                
                // Assign unit value to destination (while loops return unit).
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            // --- TODO: Implement lowering for other HIR expression kinds ---
            // e.g., Array, Map, StructInit, Match, Perform, Handle, etc.
            _ => {
                            // For unhandled expressions, assign a placeholder.
            // This allows incremental implementation.
            let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
            builder.push_statement(Statement::Assign(destination, rvalue));
            }
        }
    }

    /// Lowers an HIR statement into MIR statements.
    fn lower_stmt(
        &self,
        stmt: &'src hir::Stmt<'src>,
        builder: &mut MirBuilder<'src>,
        locals: &mut HashMap<&'src str, LocalId>,
    ) {
        match stmt {
            hir::Stmt::Let { name, value, .. } => {
                // Create a new local for the variable.
                let local_id = builder.new_local(value.ty.clone(), false);
                locals.insert(name, local_id);

                // Lower the value expression.
                self.lower_expr(value, builder, locals, Place { local: local_id });
            }
            hir::Stmt::Return(expr_opt) => {
                if let Some(expr) = expr_opt {
                    // Lower the return expression into the return place.
                    self.lower_expr(expr, builder, locals, Place { local: LocalId(0) });
                }
                builder.set_terminator(Terminator::Return);
            }
            hir::Stmt::Assign(lhs, rhs) => {
                // For now, we'll handle simple assignments to local variables.
                // This is a simplified version - real MIR would handle more complex cases.
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });

                // Find the local variable being assigned to.
                let lhs_local = match &lhs.kind {
                    hir::ExprKind::Path(path) => {
                        let name = path.first().expect("Assignment target path cannot be empty");
                        locals.get(name).expect("Assignment target not found in locals")
                    }
                    _ => panic!("Complex assignment targets not yet supported"),
                };

                let rvalue = Rvalue::Use(Operand::Copy(Place { local: rhs_temp }));
                builder.push_statement(Statement::Assign(Place { local: *lhs_local }, rvalue));
            }
            hir::Stmt::Expr(expr) => {
                // Lower the expression into a temporary local (result is discarded).
                let temp = builder.new_local(expr.ty.clone(), false);
                self.lower_expr(expr, builder, locals, Place { local: temp });
            }
        }
    }
} 