//! Cranelift IR code generation from MIR.
//!
//! This module converts the Mid-level Intermediate Representation (MIR) into
//! Cranelift IR for final code generation.

use crate::mir::{self, BasicBlock, LocalId, MirFunction, MirProgram, Operand, Place, Rvalue, Statement, Terminator, MirLocal, HandlerContext, PatternKind};
use crate::ast;
use cranelift::codegen::ir::{self, AbiParam, Function, InstBuilder, Signature, Type};
use cranelift::prelude::IntCC;
use cranelift::codegen::isa::CallConv;
use cranelift::frontend::{FunctionBuilder, FunctionBuilderContext};
use std::collections::HashMap;

/// Converts MIR to Cranelift IR and generates machine code.
pub struct CraneliftCodegen;

impl CraneliftCodegen {
    /// Convert a MIR program to Cranelift IR functions.
    pub fn convert_program(mir_program: &mir::MirProgram) -> HashMap<String, Function> {
        let mut functions = HashMap::new();
        
        for (name, mir_func) in &mir_program.functions {
            let cranelift_func = Self::convert_function(mir_func);
            functions.insert(name.to_string(), cranelift_func);
        }
        
        functions
    }
    
    /// Convert a single MIR function to Cranelift IR.
    fn convert_function(mir_func: &mir::MirFunction) -> Function {
        // Convert function signature first
        let signature = Self::convert_signature(mir_func);
        
        let mut func = Function::new();
        func.signature = signature.clone();
        
        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut builder_context);
        
        // Create all blocks first
        let mut block_mapping = HashMap::new();
        for (i, _) in mir_func.basic_blocks.iter().enumerate() {
            let block = builder.create_block();
            block_mapping.insert(i, block);
        }
        
        // Convert parameters to SSA values (for entry block)
        let entry_block = block_mapping[&0]; // Assume first block is entry
        let mut param_values = Vec::new();
        for param in &signature.params {
            let val = builder.append_block_param(entry_block, param.value_type);
            param_values.push(val);
        }
        
        // Switch to entry block first
        builder.switch_to_block(entry_block);
        
        // Convert MIR locals to Cranelift locals
        let mut local_mapping = HashMap::new();
        for (local_id, mir_local) in &mir_func.locals {
            if !mir_local.is_param {
                let cranelift_type = Self::convert_type(&mir_local.ty);
                let val = builder.ins().iconst(cranelift_type, 0); // Initialize with default value
                local_mapping.insert(*local_id, val);
            } else {
                // Parameters are already handled above
                let param_index = mir_func.params.iter().position(|p| *p == *local_id).unwrap();
                local_mapping.insert(*local_id, param_values[param_index]);
            }
        }
        
        // Convert each basic block
        for (i, mir_block) in mir_func.basic_blocks.iter().enumerate() {
            let cranelift_block = block_mapping[&i];
            
            // Skip switching for the first block since we're already there
            if i > 0 {
                builder.switch_to_block(cranelift_block);
            }
            
            // Convert statements
            for statement in &mir_block.statements {
                Self::convert_statement(statement, &mut builder, &mut local_mapping);
            }
            
            // Convert terminator
            Self::convert_terminator(
                &mir_block.terminator,
                &mut builder,
                &block_mapping,
                &mut local_mapping,
                i,
            );
            
            // Seal the block after it's completely filled
            builder.seal_block(cranelift_block);
        }
        
        builder.finalize();
        func
    }
    
    /// Convert MIR function signature to Cranelift signature.
    fn convert_signature(mir_func: &mir::MirFunction) -> Signature {
        let mut signature = Signature::new(CallConv::SystemV);
        
        // Add parameters
        for param_id in &mir_func.params {
            let local = &mir_func.locals[param_id];
            let param_type = Self::convert_type(&local.ty);
            signature.params.push(AbiParam::new(param_type));
        }
        
        // Add return value
        let return_type = Self::convert_type(&mir_func.return_type);
        signature.returns.push(AbiParam::new(return_type));
        
        signature
    }
    
    /// Convert MIR type to Cranelift type.
    fn convert_type(ty: &crate::hir::Ty) -> Type {
        match ty {
            crate::hir::Ty::Bool => Type::int(8).unwrap(),
            crate::hir::Ty::I64 => Type::int(64).unwrap(),
            crate::hir::Ty::F64 => Type::int(64).unwrap(), // Treat f64 as i64 for now
            crate::hir::Ty::Str => Type::int(64).unwrap(), // String as pointer
            crate::hir::Ty::Unit => Type::int(32).unwrap(), // Unit as i32 for simplicity
            crate::hir::Ty::Adt { .. } => Type::int(64).unwrap(), // ADT as pointer
            crate::hir::Ty::Array(_) => Type::int(64).unwrap(), // Array as pointer
            crate::hir::Ty::Map { .. } => Type::int(64).unwrap(), // Map as pointer
            crate::hir::Ty::Function { .. } => Type::int(64).unwrap(), // Function pointer
            crate::hir::Ty::Infer(_) => Type::int(32).unwrap(), // Infer as i32 for simplicity
            crate::hir::Ty::Error => Type::int(32).unwrap(), // Error as i32 for simplicity
        }
    }
    
    /// Convert MIR statement to Cranelift instructions.
    fn convert_statement(
        statement: &mir::Statement,
        builder: &mut FunctionBuilder,
        local_mapping: &mut HashMap<LocalId, ir::Value>,
    ) {
        match statement {
            mir::Statement::Assign(place, rvalue) => {
                let src_val = Self::convert_rvalue(rvalue, builder, local_mapping);
                // For now, we'll store the value in the local mapping
                // In a real implementation, we'd need proper SSA value handling
                local_mapping.insert(place.local, src_val);
            }
        }
    }
    
    /// Convert MIR rvalue to Cranelift value.
    fn convert_rvalue(
        rvalue: &mir::Rvalue,
        builder: &mut FunctionBuilder,
        local_mapping: &HashMap<LocalId, ir::Value>,
    ) -> ir::Value {
        match rvalue {
            mir::Rvalue::Use(operand) => Self::convert_operand(operand, builder, local_mapping),
            mir::Rvalue::BinaryOp(op, lhs, rhs) => {
                let lhs_val = Self::convert_operand(lhs, builder, local_mapping);
                let rhs_val = Self::convert_operand(rhs, builder, local_mapping);
                
                match op {
                    ast::BinaryOp::Add => builder.ins().iadd(lhs_val, rhs_val),
                    ast::BinaryOp::Sub => builder.ins().isub(lhs_val, rhs_val),
                    ast::BinaryOp::Mul => builder.ins().imul(lhs_val, rhs_val),
                    ast::BinaryOp::Div => builder.ins().udiv(lhs_val, rhs_val),
                    ast::BinaryOp::Eq => builder.ins().icmp(IntCC::Equal, lhs_val, rhs_val),
                    ast::BinaryOp::Ne => builder.ins().icmp(IntCC::NotEqual, lhs_val, rhs_val),
                    ast::BinaryOp::Lt => builder.ins().icmp(IntCC::SignedLessThan, lhs_val, rhs_val),
                    ast::BinaryOp::Gt => builder.ins().icmp(IntCC::SignedGreaterThan, lhs_val, rhs_val),
                }
            }
            mir::Rvalue::UnaryOp(op, operand) => {
                let operand_val = Self::convert_operand(operand, builder, local_mapping);
                
                match op {
                    ast::UnaryOp::Neg => builder.ins().ineg(operand_val),
                    ast::UnaryOp::Not => builder.ins().bnot(operand_val),
                }
            }
            mir::Rvalue::Ref(place) => {
                // For now, return the place value directly since we're using pointers
                Self::get_place_value(place, builder, local_mapping)
            }
            mir::Rvalue::Array(elements) => {
                // For now, create a simple array representation
                // In a real implementation, this would allocate memory
                let first_element = Self::convert_operand(&elements[0], builder, local_mapping);
                first_element
            }
            mir::Rvalue::Map(_) => {
                // For now, return a null pointer
                builder.ins().iconst(Type::int(64).unwrap(), 0)
            }
            mir::Rvalue::StructInit { .. } => {
                // For now, return a null pointer
                builder.ins().iconst(Type::int(64).unwrap(), 0)
            }
        }
    }
    
    /// Convert MIR operand to Cranelift value.
    fn convert_operand(
        operand: &mir::Operand,
        builder: &mut FunctionBuilder,
        local_mapping: &HashMap<LocalId, ir::Value>,
    ) -> ir::Value {
        match operand {
            mir::Operand::Constant(literal) => Self::convert_literal(literal, builder),
            mir::Operand::Copy(place) => {
                // Get the value from the local mapping
                local_mapping[&place.local]
            }
        }
    }
    
    /// Convert MIR literal to Cranelift constant.
    fn convert_literal(literal: &ast::Literal, builder: &mut FunctionBuilder) -> ir::Value {
        match literal {
            ast::Literal::I64(value) => {
                if *value <= i32::MAX as i64 && *value >= i32::MIN as i64 {
                    builder.ins().iconst(Type::int(32).unwrap(), *value as i64)
                } else {
                    builder.ins().iconst(Type::int(64).unwrap(), *value)
                }
            }
            ast::Literal::F64(value) => {
                if value.fract() == 0.0 && *value <= f32::MAX as f64 && *value >= f32::MIN as f64 {
                    builder.ins().f32const(ir::immediates::Ieee32::with_bits(*value as u32))
                } else {
                    builder.ins().f64const(ir::immediates::Ieee64::with_bits(*value as u64))
                }
            }
            ast::Literal::Bool(value) => {
                builder.ins().iconst(Type::int(8).unwrap(), if *value { 1 } else { 0 })
            }
            ast::Literal::Str(_) => {
                // For now, return a null pointer for strings
                builder.ins().iconst(Type::int(64).unwrap(), 0)
            }
            ast::Literal::Unit => {
                builder.ins().iconst(Type::int(32).unwrap(), 0)
            }
        }
    }
    
    /// Get the Cranelift value for a MIR place.
    fn get_place_value(
        place: &mir::Place,
        builder: &mut FunctionBuilder,
        local_mapping: &HashMap<LocalId, ir::Value>,
    ) -> ir::Value {
        local_mapping[&place.local]
    }
    
    /// Convert MIR terminator to Cranelift control flow.
    fn convert_terminator(
        terminator: &mir::Terminator,
        builder: &mut FunctionBuilder,
        block_mapping: &HashMap<usize, ir::Block>,
        local_mapping: &mut HashMap<LocalId, ir::Value>,
        current_block: usize,
    ) {
        match terminator {
            mir::Terminator::Goto { target } => {
                let target_block = block_mapping[target];
                builder.ins().jump(target_block, &[]);
            }
            mir::Terminator::SwitchInt { discr, targets, otherwise } => {
                let discr_val = Self::convert_operand(discr, builder, local_mapping);
                
                // For now, implement a simple conditional jump
                // In a real implementation, you'd create a proper jump table
                if let Some((_, target)) = targets.first() {
                    let target_block = block_mapping[target];
                    let otherwise_block = block_mapping[otherwise];
                    builder.ins().brif(discr_val, target_block, &[], otherwise_block, &[]);
                } else {
                    let otherwise_block = block_mapping[otherwise];
                    builder.ins().jump(otherwise_block, &[]);
                }
            }
            mir::Terminator::Call { func: _, args, destination, target } => {
                // For now, we'll create a simple call mechanism
                // In a real implementation, you'd need to handle function signatures properly
                let mut arg_values = Vec::new();
                for arg in args {
                    let arg_val = Self::convert_operand(arg, builder, local_mapping);
                    arg_values.push(arg_val);
                }
                
                // Create a placeholder for the function call
                // In a real implementation, you'd resolve the function and call it
                let result = builder.ins().iconst(Type::int(32).unwrap(), 0); // Placeholder result
                
                // Store result in destination
                local_mapping.insert(destination.local, result);
                
                // Jump to target block
                let target_block = block_mapping[target];
                builder.ins().jump(target_block, &[]);
            }
            mir::Terminator::PushHandler { effect, handler, target } => {
                // For now, just jump to the target block
                // In a real implementation, you'd push the handler onto a stack
                let target_block = block_mapping[target];
                builder.ins().jump(target_block, &[]);
            }
            mir::Terminator::PopHandler { target } => {
                // For now, just jump to the target block
                // In a real implementation, you'd pop the handler from the stack
                let target_block = block_mapping[target];
                builder.ins().jump(target_block, &[]);
            }
            mir::Terminator::Perform { effect, operation, args, destination, continuation, no_handler } => {
                // For now, just jump to the no_handler block
                // In a real implementation, you'd look up the handler and perform the operation
                let no_handler_block = block_mapping[no_handler];
                builder.ins().jump(no_handler_block, &[]);
            }
            mir::Terminator::Resume { value, target } => {
                // For now, just jump to the target block
                // In a real implementation, you'd resume with the value
                let target_block = block_mapping[target];
                builder.ins().jump(target_block, &[]);
            }
            mir::Terminator::PatternMatch { scrutinee, arms, otherwise } => {
                let scrutinee_val = Self::convert_operand(scrutinee, builder, local_mapping);
                
                // For now, implement a simple switch-based pattern matching
                // In a real implementation, you'd need more sophisticated pattern matching
                if let Some((pattern, target)) = arms.first() {
                    if let mir::PatternKind::Literal(literal) = &pattern.kind {
                        if let ast::Literal::I64(value) = literal {
                            let target_block = block_mapping[target];
                            let otherwise_block = block_mapping[otherwise];
                            let const_val = builder.ins().iconst(Type::int(64).unwrap(), *value);
                            builder.ins().icmp_imm(IntCC::Equal, scrutinee_val, *value as i64);
                            builder.ins().brif(scrutinee_val, target_block, &[], otherwise_block, &[]);
                            return;
                        }
                    }
                }
                
                let otherwise_block = block_mapping[otherwise];
                builder.ins().jump(otherwise_block, &[]);
            }
            mir::Terminator::Return => {
                // For now, return a default value
                // In a real implementation, you'd return the actual return value
                let return_val = builder.ins().iconst(Type::int(32).unwrap(), 0);
                builder.ins().return_(&[return_val]);
            }
            mir::Terminator::Unreachable => {
                builder.ins().trap(ir::TrapCode::UnreachableCodeReached);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir::{MirFunction, MirLocal, BasicBlock, Statement, Terminator, Rvalue, Operand, Place};
    use crate::ast::Literal;
    use crate::hir::Ty;
    
    #[test]
    fn test_convert_simple_function() {
        // Create a simple MIR function: fn add(a: i64, b: i64) -> i64 { a + b }
        let mut locals = HashMap::new();
        locals.insert(LocalId(0), MirLocal { id: LocalId(0), ty: Ty::I64, is_param: false }); // return value
        locals.insert(LocalId(1), MirLocal { id: LocalId(1), ty: Ty::I64, is_param: true });  // param a
        locals.insert(LocalId(2), MirLocal { id: LocalId(2), ty: Ty::I64, is_param: true });  // param b
        
        let mut basic_blocks = Vec::new();
        let mut statements = Vec::new();
        
        // _0 = _1 + _2
        statements.push(Statement::Assign(
            Place { local: LocalId(0) },
            Rvalue::BinaryOp(
                ast::BinaryOp::Add,
                Operand::Copy(Place { local: LocalId(1) }),
                Operand::Copy(Place { local: LocalId(2) }),
            ),
        ));
        
        basic_blocks.push(BasicBlock {
            id: 0,
            statements,
            terminator: Terminator::Return,
        });
        
        let mir_func = MirFunction {
            name: "add",
            params: vec![LocalId(1), LocalId(2)],
            return_type: Ty::I64,
            basic_blocks,
            locals,
            next_local_id: LocalId(3),
            handler_context: mir::HandlerContext::new(),
        };
        
        let cranelift_func = CraneliftCodegen::convert_function(&mir_func);
        
        // Basic validation that the function was created
        assert_eq!(cranelift_func.signature.params.len(), 2);
        assert_eq!(cranelift_func.signature.returns.len(), 1);
    }
} 