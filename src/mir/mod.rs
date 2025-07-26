//! src/mir/mod.rs
//!
//! This module handles the lowering of the Hierarchical Intermediate Representation (HIR)
//! into the Mid-level Intermediate Representation (MIR) using structured control flow.

mod builder;
pub mod data;

use crate::{ast, hir};
use builder::MirBuilder;
use data::*;

// Re-export commonly used types for easier access
pub use data::{
    LocalId, MirFunction, MirProgram, Operand, PatternKind,
    Place, Rvalue, Statement, MirInstruction,
};
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
        let body_instructions = self.lower_expr(
            &func.body,
            &mut builder,
            &mut hir_to_mir_locals,
            // The result of the function body will be stored in the return place (`_0`)
            Place { local: LocalId(0) },
        );

        // Add the body instructions to the builder
        for instruction in body_instructions {
            builder.push_instruction(instruction);
        }

        // Add a return instruction at the end
        builder.push_instruction(MirInstruction::Return);

        builder.build(func.name, param_locals, func.ret_type.clone())
    }

    /// Coerces a literal to the expected type, handling type conversions as needed.
    fn coerce_literal_to_type(&self, lit: &ast::Literal<'src>, expected_ty: &hir::Ty<'src>) -> ast::Literal<'src> {
        match (lit, expected_ty) {
            (ast::Literal::I64(value), hir::Ty::I32) => {
                // Convert to I32 literal when expected type is I32
                // The typechecker should have already verified the value fits
                ast::Literal::I32(*value as i32)
            }
            _ => {
                // For other cases, just return the literal as-is
                lit.clone()
            }
        }
    }

    /// Lowers an HIR expression into a series of MIR instructions.
    /// The result of the expression is placed in `destination`.
    /// Returns a Vec<MirInstruction> representing the structured control flow.
    fn lower_expr(
        &self,
        expr: &'src hir::Expr<'src>,
        builder: &mut MirBuilder<'src>,
        locals: &mut HashMap<&'src str, LocalId>,
        destination: Place,
    ) -> Vec<MirInstruction<'src>> {
        match &expr.kind {
            hir::ExprKind::Literal(lit) => {
                // Get the expected type from the destination
                let expected_ty = &builder.locals[&destination.local].ty;
                let coerced_lit = self.coerce_literal_to_type(lit, expected_ty);
                let rvalue = Rvalue::Use(Operand::Constant(coerced_lit));
                vec![MirInstruction::Assign(destination, rvalue)]
            }
            hir::ExprKind::Path(path) => {
                // This is a local variable or function reference
                let name = path.first().expect("Path cannot be empty");
                let local_id = locals
                    .get(name)
                    .expect("Local variable not found in MIR context");
                let rvalue = Rvalue::Use(Operand::Copy(Place { local: *local_id }));
                vec![MirInstruction::Assign(destination, rvalue)]
            }
            hir::ExprKind::EnumVariant { enum_name: _, variant_name: _ } => {
                // TODO: Implement proper enum variant construction
                // This should create an enum variant with the specified fields
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                vec![MirInstruction::Assign(destination, rvalue)]
            }
            hir::ExprKind::ModulePath { module: _, symbol: _ } => {
                // TODO: Implement proper module symbol resolution
                // This should resolve the symbol from the specified module
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                vec![MirInstruction::Assign(destination, rvalue)]
            }
            hir::ExprKind::FieldAccess { receiver, field } => {
                // Lower the receiver into a temporary local
                let receiver_temp = builder.new_local(receiver.ty.clone(), false);
                let mut instructions = self.lower_expr(receiver, builder, locals, Place { local: receiver_temp });
                
                // Create a projection to access the specific field
                let rvalue = Rvalue::Projection {
                    base: Place { local: receiver_temp },
                    field,
                };
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::Binary { op, lhs, rhs } => {
                // Lower LHS into a temporary local.
                let lhs_temp = builder.new_local(lhs.ty.clone(), false);
                let mut instructions = self.lower_expr(lhs, builder, locals, Place { local: lhs_temp });

                // Lower RHS into another temporary local.
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                let rhs_instructions = self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });
                instructions.extend(rhs_instructions);

                // Create the binary operation Rvalue.
                let rvalue = Rvalue::BinaryOp(
                    *op,
                    Operand::Copy(Place { local: lhs_temp }),
                    Operand::Copy(Place { local: rhs_temp }),
                );
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::Unary { op, rhs } => {
                // Lower RHS into a temporary local.
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                let mut instructions = self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });

                // Create the unary operation Rvalue.
                let rvalue = Rvalue::UnaryOp(*op, Operand::Copy(Place { local: rhs_temp }));
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                // Lower the condition into a temporary local.
                let cond_temp = builder.new_local(cond.ty.clone(), false);
                let mut instructions = self.lower_expr(cond, builder, locals, Place { local: cond_temp });

                // Lower the then block.
                let then_instructions = self.lower_expr(then_block, builder, locals, destination.clone());

                // Lower the else block if it exists.
                let else_instructions = if let Some(else_expr) = else_block {
                    self.lower_expr(else_expr, builder, locals, destination)
                } else {
                    // No else block - assign unit to destination
                    let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                    vec![MirInstruction::Assign(destination, rvalue)]
                };

                // Create the if instruction
                instructions.push(MirInstruction::If {
                    condition: Operand::Copy(Place { local: cond_temp }),
                    then_block: then_instructions,
                    else_block: else_instructions,
                });

                instructions
            }
            hir::ExprKind::Block { stmts, last_expr } => {
                let mut instructions = Vec::new();

                // Lower all statements in the block.
                for stmt in stmts {
                    let stmt_instructions = self.lower_stmt(stmt, builder, locals);
                    instructions.extend(stmt_instructions);
                }

                // Check if any statement was a return
                let has_return = stmts
                    .iter()
                    .any(|stmt| matches!(stmt, hir::Stmt::Return(_)));

                // Lower the last expression if it exists and no return statement was encountered.
                if let Some(expr) = last_expr {
                    if !has_return {
                        let expr_instructions = self.lower_expr(expr, builder, locals, destination);
                        instructions.extend(expr_instructions);
                    }
                } else if !has_return {
                    // No last expression and no return statement - assign unit.
                    let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                    instructions.push(MirInstruction::Assign(destination, rvalue));
                }
                // If there's a return statement, don't assign anything to destination
                // The return statement has already handled the return value

                instructions
            }
            hir::ExprKind::Call { fun, args } => {
                // Lower all arguments into temporary locals.
                let mut instructions = Vec::new();
                let mut arg_operands = Vec::new();
                for arg in args {
                    let arg_temp = builder.new_local(arg.ty.clone(), false);
                    let arg_instructions = self.lower_expr(arg, builder, locals, Place { local: arg_temp });
                    instructions.extend(arg_instructions);
                    arg_operands.push(Operand::Copy(Place { local: arg_temp }));
                }

                // Create the call instruction
                let call_instruction = MirInstruction::Call {
                    func: match &**fun {
                        hir::Expr {
                            kind: hir::ExprKind::Path(path),
                            ..
                        } => path.first().expect("Function path cannot be empty"),
                        hir::Expr {
                            kind: hir::ExprKind::EnumVariant { enum_name: _, variant_name },
                            ..
                        } => variant_name, // Use the variant name as the function name
                        hir::Expr {
                            kind: hir::ExprKind::ModulePath { module: _, symbol },
                            ..
                        } => symbol, // Use the symbol name as the function name
                        _ => panic!("Expected function call to have a path, enum variant, or module path expression"),
                    },
                    args: arg_operands,
                    destination: destination,
                };

                instructions.push(call_instruction);
                instructions
            }
            hir::ExprKind::Perform { path, args } => {
                // Lower all arguments into temporary locals.
                let mut instructions = Vec::new();
                let mut arg_operands = Vec::new();
                for arg in args {
                    let arg_temp = builder.new_local(arg.ty.clone(), false);
                    let arg_instructions = self.lower_expr(arg, builder, locals, Place { local: arg_temp });
                    instructions.extend(arg_instructions);
                    arg_operands.push(Operand::Copy(Place { local: arg_temp }));
                }

                // Extract effect and operation names from path
                let effect_name = path.first().expect("Effect path cannot be empty");
                let operation_name = path
                    .get(1)
                    .expect("Effect operation path must have operation name");

                // Create the perform instruction
                let perform_instruction = MirInstruction::Perform {
                    effect: effect_name,
                    operation: operation_name,
                    args: arg_operands,
                    destination: destination,
                };

                instructions.push(perform_instruction);
                instructions
            }
            hir::ExprKind::Handle { body, handler } => {
                match handler {
                    hir::HandlerBody::Path(handler_path) => {
                        // Extract handler name from path
                        let handler_name =
                            handler_path.first().expect("Handler path cannot be empty");

                        // TODO: Check the handler's effect signature to determine which effects it handles
                        let effect_name = "IO"; // Should come from handler definition

                        // Lower the body
                        let body_instructions = self.lower_expr(body, builder, locals, destination);

                        // Create the push handler instruction
                        vec![MirInstruction::PushHandler {
                            effect: effect_name,
                            handler: handler_name,
                            body: body_instructions,
                        }]
                    }
                    hir::HandlerBody::Inline(_handler_functions) => {
                        // For inline handlers, we'd need to create local handler functions
                        // and push them onto the effect stack
                        // This is more complex and would require additional MIR constructs

                        // For now, just lower the body without any handler
                        self.lower_expr(body, builder, locals, destination)
                    }
                }
            }
            hir::ExprKind::Array(elements) => {
                // Lower all array elements into operands
                let mut instructions = Vec::new();
                let mut element_operands = Vec::new();
                for element in elements {
                    let element_temp = builder.new_local(element.ty.clone(), false);
                    let element_instructions = self.lower_expr(
                        element,
                        builder,
                        locals,
                        Place {
                            local: element_temp,
                        },
                    );
                    instructions.extend(element_instructions);
                    element_operands.push(Operand::Copy(Place {
                        local: element_temp,
                    }));
                }

                // Create array Rvalue
                let rvalue = Rvalue::Array(element_operands);
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::Map(entries) => {
                // Lower all map entries into operands
                let mut instructions = Vec::new();
                let mut entry_operands = Vec::new();
                for (key, value) in entries {
                    let key_temp = builder.new_local(key.ty.clone(), false);
                    let value_temp = builder.new_local(value.ty.clone(), false);

                    let key_instructions = self.lower_expr(key, builder, locals, Place { local: key_temp });
                    let value_instructions = self.lower_expr(value, builder, locals, Place { local: value_temp });

                    instructions.extend(key_instructions);
                    instructions.extend(value_instructions);

                    entry_operands.push((
                        Operand::Copy(Place { local: key_temp }),
                        Operand::Copy(Place { local: value_temp }),
                    ));
                }

                // Create map Rvalue
                let rvalue = Rvalue::Map(entry_operands);
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::StructInit { path, fields } => {
                // Lower all field values into operands
                let mut instructions = Vec::new();
                let mut field_operands = HashMap::new();
                for (field_name, field_expr) in fields {
                    let field_temp = builder.new_local(field_expr.ty.clone(), false);
                    let field_instructions = self.lower_expr(field_expr, builder, locals, Place { local: field_temp });
                    instructions.extend(field_instructions);
                    field_operands.insert(*field_name, Operand::Copy(Place { local: field_temp }));
                }

                // Create struct initialization Rvalue
                let struct_name = path.first().expect("Struct path cannot be empty");
                let rvalue = Rvalue::StructInit {
                    path: struct_name,
                    fields: field_operands,
                };
                instructions.push(MirInstruction::Assign(destination, rvalue));
                instructions
            }
            hir::ExprKind::Match { scrutinee, arms } => {
                // Lower the scrutinee into a temporary local.
                let scrutinee_temp = builder.new_local(scrutinee.ty.clone(), false);
                let mut instructions = self.lower_expr(
                    scrutinee,
                    builder,
                    locals,
                    Place {
                        local: scrutinee_temp,
                    },
                );

                // Lower each arm
                let mut arm_instructions = Vec::new();
                for (pattern, expr) in arms {
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
                            }
                            hir::PatternKind::AdtVariant { path, fields } => {
                                // Handle field bindings
                                for field in fields {
                                    if let hir::PatternKind::Binding { name, is_mut: _ } = &field.kind
                                    {
                                        let field_local =
                                            builder.new_local(field.ty.clone(), false);
                                        locals.insert(name, field_local);
                                    }
                                }
                                PatternKind::AdtVariant {
                                    path: path.first().expect("Pattern path cannot be empty"),
                                    fields: fields
                                        .iter()
                                        .map(|f| Pattern {
                                            kind: match &f.kind {
                                                hir::PatternKind::Literal(lit) => {
                                                    PatternKind::Literal(lit.clone())
                                                }
                                                hir::PatternKind::Binding { name, is_mut } => {
                                                    PatternKind::Binding {
                                                        name,
                                                        is_mut: *is_mut,
                                                    }
                                                }
                                                hir::PatternKind::AdtVariant { path, fields: _ } => {
                                                    PatternKind::AdtVariant {
                                                        path: path
                                                            .first()
                                                            .expect("Pattern path cannot be empty"),
                                                        fields: Vec::new(), // Simplified for now
                                                    }
                                                }
                                                hir::PatternKind::Wildcard => PatternKind::Wildcard,
                                            },
                                            ty: f.ty.clone(),
                                        })
                                        .collect(),
                                }
                            }
                            hir::PatternKind::Wildcard => PatternKind::Wildcard,
                        },
                        ty: pattern.ty.clone(),
                    };

                    let arm_body = self.lower_expr(expr, builder, locals, destination.clone());
                    arm_instructions.push((mir_pattern, arm_body));
                }

                // Create default case (assign unit)
                let default_instructions = {
                    let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                    vec![MirInstruction::Assign(destination, rvalue)]
                };

                // Create the pattern match instruction
                instructions.push(MirInstruction::PatternMatch {
                    scrutinee: Operand::Copy(Place {
                        local: scrutinee_temp,
                    }),
                    arms: arm_instructions,
                    otherwise: default_instructions,
                });

                instructions
            }
            hir::ExprKind::While { cond, body } => {
                // Lower the condition into a temporary local.
                let cond_temp = builder.new_local(cond.ty.clone(), false);
                let cond_instructions = self.lower_expr(cond, builder, locals, Place { local: cond_temp });

                // Lower the body
                let body_temp = builder.new_local(body.ty.clone(), false);
                let body_instructions = self.lower_expr(body, builder, locals, Place { local: body_temp });

                // Create the loop structure
                // The pattern for while (cond) { body } is:
                // loop
                //   ;; ... code for cond ...
                //   ;; check if cond is false and break if so
                //   ;; ... code for body ...
                //   br 0 ;; branch to the start of the loop
                // end

                let mut loop_body = Vec::new();
                
                // Add condition evaluation
                loop_body.extend(cond_instructions);
                
                // Add conditional break - break out of loop if condition is false
                // For boolean conditions, we want to break when condition is false
                // For integer conditions, we want to break when condition is zero
                // The ConditionalBreak instruction breaks when the condition is true
                // So for while loops, we want to break when the condition is false
                if matches!(cond.ty, hir::Ty::Bool) {
                    // For boolean, we want to break when condition is false
                    // So we need to check if condition == false
                    let false_temp = builder.new_local(hir::Ty::Bool, false);
                    loop_body.push(MirInstruction::Assign(
                        Place { local: false_temp },
                        Rvalue::Use(Operand::Constant(ast::Literal::Bool(false)))
                    ));
                    let eq_temp = builder.new_local(hir::Ty::Bool, false);
                    loop_body.push(MirInstruction::Assign(
                        Place { local: eq_temp },
                        Rvalue::BinaryOp(
                            ast::BinaryOp::Eq,
                            Operand::Copy(Place { local: cond_temp }),
                            Operand::Copy(Place { local: false_temp })
                        )
                    ));
                    loop_body.push(MirInstruction::ConditionalBreak(
                        Operand::Copy(Place { local: eq_temp }),
                        0, // break out of the immediate enclosing loop
                    ));
                } else {
                    // For integers, we want to break when condition is zero
                    // So we break when condition == 0
                    loop_body.push(MirInstruction::ConditionalBreak(
                        Operand::Copy(Place { local: cond_temp }),
                        0, // break out of the immediate enclosing loop
                    ));
                }
                
                // Add body instructions
                loop_body.extend(body_instructions);
                
                // Add unconditional break (br 0) - jump back to start of loop
                loop_body.push(MirInstruction::Break(0));

                // Create the loop instruction
                let loop_instruction = MirInstruction::Loop { body: loop_body };

                // Assign unit value to destination (while loops return unit)
                let rvalue = Rvalue::Use(Operand::Constant(ast::Literal::Unit));
                vec![loop_instruction, MirInstruction::Assign(destination, rvalue)]
            }
        }
    }

    /// Lowers an HIR statement into MIR instructions.
    fn lower_stmt(
        &self,
        stmt: &'src hir::Stmt<'src>,
        builder: &mut MirBuilder<'src>,
        locals: &mut HashMap<&'src str, LocalId>,
    ) -> Vec<MirInstruction<'src>> {
        match stmt {
            hir::Stmt::Let { name, value, .. } => {
                // Create a new local for the variable.
                let local_id = builder.new_local(value.ty.clone(), false);
                locals.insert(name, local_id);

                // Lower the value expression.
                self.lower_expr(value, builder, locals, Place { local: local_id })
            }
            hir::Stmt::Return(expr_opt) => {
                if let Some(expr) = expr_opt {
                    // Lower the return expression into the return place.
                    let mut instructions = self.lower_expr(expr, builder, locals, Place { local: LocalId(0) });
                    instructions.push(MirInstruction::Return);
                    instructions
                } else {
                    vec![MirInstruction::Return]
                }
            }
            hir::Stmt::Assign(lhs, rhs) => {
                // TODO: Support complex assignment targets like struct fields
                let rhs_temp = builder.new_local(rhs.ty.clone(), false);
                let mut instructions = self.lower_expr(rhs, builder, locals, Place { local: rhs_temp });

                // Find the local variable being assigned to.
                let lhs_local = match &lhs.kind {
                    hir::ExprKind::Path(path) => {
                        let name = path
                            .first()
                            .expect("Assignment target path cannot be empty");
                        locals
                            .get(name)
                            .expect("Assignment target not found in locals")
                    }
                    _ => panic!("Complex assignment targets not yet supported"),
                };

                let rvalue = Rvalue::Use(Operand::Copy(Place { local: rhs_temp }));
                instructions.push(MirInstruction::Assign(Place { local: *lhs_local }, rvalue));
                instructions
            }
            hir::Stmt::Expr(expr) => {
                // Create a temporary local for the expression result.
                let temp_local = builder.new_local(expr.ty.clone(), false);
                self.lower_expr(expr, builder, locals, Place { local: temp_local })
            }
        }
    }
}

