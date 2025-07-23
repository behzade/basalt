//! src/mir/mod.rs
//!
//! This module handles the lowering of the Hierarchical Intermediate Representation (HIR)
//! into the Mid-level Intermediate Representation (MIR).

mod builder;
pub mod data;

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
            hir::ExprKind::If { cond, then_block, else_block } => {
                // Lower the condition into a temporary local.
                let cond_temp = builder.new_local(cond.ty.clone(), false);
                self.lower_expr(cond, builder, locals, Place { local: cond_temp });

                // Create blocks for the if structure.
                let then_block_id = builder.new_basic_block();
                let else_block_id = builder.new_basic_block();
                let merge_block_id = builder.new_basic_block();

                // Terminate current block with conditional jump.
                builder.set_terminator(Terminator::SwitchInt {
                    discr: Operand::Copy(Place { local: cond_temp }),
                    targets: vec![(1, then_block_id)], // true -> then block
                    otherwise: else_block_id,           // false -> else block
                });

                // Lower the then block.
                builder.switch_to_block(then_block_id);
                self.lower_expr(then_block, builder, locals, destination.clone());
                builder.set_terminator(Terminator::Goto { target: merge_block_id });

                // Lower the else block if it exists.
                if let Some(else_expr) = else_block {
                    builder.switch_to_block(else_block_id);
                    self.lower_expr(else_expr, builder, locals, destination);
                    builder.set_terminator(Terminator::Goto { target: merge_block_id });
                } else {
                    // No else block - assign unit to destination and goto merge
                    builder.switch_to_block(else_block_id);
                    let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                    builder.push_statement(Statement::Assign(destination.clone(), rvalue));
                    builder.set_terminator(Terminator::Goto { target: merge_block_id });
                }

                // Continue from merge block.
                builder.switch_to_block(merge_block_id);
            }
            hir::ExprKind::Block { stmts, last_expr } => {
                // Lower all statements in the block.
                for stmt in stmts {
                    self.lower_stmt(stmt, builder, locals);
                }

                // Check if any statement was a return
                let has_return = stmts.iter().any(|stmt| {
                    matches!(stmt, hir::Stmt::Return(_))
                });

                // Lower the last expression if it exists and no return statement was encountered.
                if let Some(expr) = last_expr {
                    if !has_return {
                        self.lower_expr(expr, builder, locals, destination);
                    }
                } else if !has_return {
                    // No last expression and no return statement - assign unit.
                    let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                    builder.push_statement(Statement::Assign(destination, rvalue));
                }
                // If there's a return statement, don't assign anything to destination
                // The return statement has already handled the return value
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
            hir::ExprKind::Perform { path, args } => {
                // Lower all arguments into temporary locals.
                let mut arg_operands = Vec::new();
                for arg in args {
                    let arg_temp = builder.new_local(arg.ty.clone(), false);
                    self.lower_expr(arg, builder, locals, Place { local: arg_temp });
                    arg_operands.push(Operand::Copy(Place { local: arg_temp }));
                }

                // Extract effect and operation names from path
                let effect_name = path.first().expect("Effect path cannot be empty");
                let operation_name = path.get(1).expect("Effect operation path must have operation name");

                // Create continuation block for after the effect is handled
                let continuation_bb = builder.new_basic_block();
                
                // Create block for when no handler is found
                let no_handler_bb = builder.new_basic_block();

                // Generate dynamic perform operation
                builder.set_terminator(Terminator::Perform {
                    effect: effect_name,
                    operation: operation_name,
                    args: arg_operands,
                    destination: destination.clone(),
                    continuation: continuation_bb,
                    no_handler: no_handler_bb,
                });

                // Set up no-handler block (for now, assign unit and continue)
                builder.switch_to_block(no_handler_bb);
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                builder.push_statement(Statement::Assign(destination.clone(), rvalue));
                builder.set_terminator(Terminator::Goto { target: continuation_bb });

                // Switch to continuation block
                builder.switch_to_block(continuation_bb);
            }
            hir::ExprKind::Handle { body, handler } => {
                match handler {
                    hir::HandlerBody::Path(handler_path) => {
                        // Extract handler name from path
                        let handler_name = handler_path.first().expect("Handler path cannot be empty");
                        
                        // For now, we'll assume the handler handles all effects
                        // In a real implementation, we'd check the handler's effect signature
                        let effect_name = "IO"; // Placeholder - should come from handler definition
                        
                        // Create blocks for the handle structure
                        let body_block = builder.new_basic_block();
                        let after_handle_block = builder.new_basic_block();
                        
                        // Push handler and jump to body
                        builder.set_terminator(Terminator::PushHandler {
                            effect: effect_name,
                            handler: handler_name,
                            target: body_block,
                        });
                        
                        // Lower the body in the body block
                        builder.switch_to_block(body_block);
                        self.lower_expr(body, builder, locals, destination);
                        
                        // Pop handler and continue
                        builder.set_terminator(Terminator::PopHandler {
                            target: after_handle_block,
                        });
                        
                        // Switch to after-handle block
                        builder.switch_to_block(after_handle_block);
                    }
                    hir::HandlerBody::Inline(handler_functions) => {
                        // For inline handlers, we'd need to create local handler functions
                        // and push them onto the effect stack
                        // This is more complex and would require additional MIR constructs
                        
                        // For now, just lower the body without any handler
                        self.lower_expr(body, builder, locals, destination);
                    }
                }
            }
            hir::ExprKind::Array(elements) => {
                // Lower all array elements into operands
                let mut element_operands = Vec::new();
                for element in elements {
                    let element_temp = builder.new_local(element.ty.clone(), false);
                    self.lower_expr(element, builder, locals, Place { local: element_temp });
                    element_operands.push(Operand::Copy(Place { local: element_temp }));
                }
                
                // Create array Rvalue
                let rvalue = Rvalue::Array(element_operands);
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::Map(entries) => {
                // Lower all map entries into operands
                let mut entry_operands = Vec::new();
                for (key, value) in entries {
                    let key_temp = builder.new_local(key.ty.clone(), false);
                    let value_temp = builder.new_local(value.ty.clone(), false);
                    
                    self.lower_expr(key, builder, locals, Place { local: key_temp });
                    self.lower_expr(value, builder, locals, Place { local: value_temp });
                    
                    entry_operands.push((
                        Operand::Copy(Place { local: key_temp }),
                        Operand::Copy(Place { local: value_temp })
                    ));
                }
                
                // Create map Rvalue
                let rvalue = Rvalue::Map(entry_operands);
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::StructInit { path, fields } => {
                // Lower all field values into operands
                let mut field_operands = HashMap::new();
                for (field_name, field_expr) in fields {
                    let field_temp = builder.new_local(field_expr.ty.clone(), false);
                    self.lower_expr(field_expr, builder, locals, Place { local: field_temp });
                    field_operands.insert(*field_name, Operand::Copy(Place { local: field_temp }));
                }
                
                // Create struct initialization Rvalue
                let struct_name = path.first().expect("Struct path cannot be empty");
                let rvalue = Rvalue::StructInit {
                    path: struct_name,
                    fields: field_operands,
                };
                builder.push_statement(Statement::Assign(destination, rvalue));
            }
            hir::ExprKind::Match { scrutinee, arms } => {
                // Lower the scrutinee into a temporary local.
                let scrutinee_temp = builder.new_local(scrutinee.ty.clone(), false);
                self.lower_expr(scrutinee, builder, locals, Place { local: scrutinee_temp });

                // Create blocks for each arm and a merge block.
                let mut arm_blocks = Vec::new();
                for (pattern, _) in arms {
                    let arm_block = builder.new_basic_block();
                    // Convert hir::Pattern to data::Pattern
                    let mir_pattern = Pattern {
                        kind: match &pattern.kind {
                            hir::PatternKind::Literal(lit) => PatternKind::Literal(lit.clone()),
                            hir::PatternKind::Binding { name, is_mut } => {
                                // Create a local for the bound variable
                                let binding_local = builder.new_local(pattern.ty.clone(), false);
                                locals.insert(name, binding_local);
                                PatternKind::Binding {
                                    name,
                                    is_mut: *is_mut,
                                }
                            },
                            hir::PatternKind::AdtVariant { path, fields } => {
                                // Handle field bindings
                                for field in fields {
                                    if let hir::PatternKind::Binding { name, is_mut } = &field.kind {
                                        let field_local = builder.new_local(field.ty.clone(), false);
                                        locals.insert(name, field_local);
                                    }
                                }
                                PatternKind::AdtVariant {
                                    path: path.first().expect("Pattern path cannot be empty"),
                                    fields: fields.iter().map(|f| Pattern {
                                        kind: match &f.kind {
                                            hir::PatternKind::Literal(lit) => PatternKind::Literal(lit.clone()),
                                            hir::PatternKind::Binding { name, is_mut } => PatternKind::Binding {
                                                name,
                                                is_mut: *is_mut,
                                            },
                                            hir::PatternKind::AdtVariant { path, fields } => PatternKind::AdtVariant {
                                                path: path.first().expect("Pattern path cannot be empty"),
                                                fields: Vec::new(), // Simplified for now
                                            },
                                            hir::PatternKind::Wildcard => PatternKind::Wildcard,
                                        },
                                        ty: f.ty.clone(),
                                    }).collect(),
                                }
                            },
                            hir::PatternKind::Wildcard => PatternKind::Wildcard,
                        },
                        ty: pattern.ty.clone(),
                    };
                    arm_blocks.push((mir_pattern, arm_block));
                }
                let default_block = builder.new_basic_block();
                let merge_block = builder.new_basic_block();

                // Lower each arm.
                for ((pattern, expr), arm_block) in arms.iter().zip(arm_blocks.iter()) {
                    builder.switch_to_block(arm_block.1);
                    self.lower_expr(expr, builder, locals, destination.clone());
                    builder.set_terminator(Terminator::Goto { target: merge_block });
                }
                
                // Set up the default block to assign unit and goto merge
                builder.switch_to_block(default_block);
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                builder.push_statement(Statement::Assign(destination, rvalue));
                builder.set_terminator(Terminator::Goto { target: merge_block });
                
                // Set the pattern match terminator in the current block (which should be the original block)
                // We need to manually set it in the correct block since we've been switching around
                let original_block = builder.basic_blocks.len() - 2 - arms.len();
                builder.basic_blocks[original_block].terminator = Terminator::PatternMatch {
                    scrutinee: Operand::Copy(Place { local: scrutinee_temp }),
                    arms: arm_blocks,
                    otherwise: default_block,
                };
                
                // Continue from merge block
                builder.switch_to_block(merge_block);
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
                // Create a temporary local for the expression result.
                let temp_local = builder.new_local(expr.ty.clone(), false);
                self.lower_expr(expr, builder, locals, Place { local: temp_local });
            }
        }
    }
} 