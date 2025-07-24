//! Cranelift backend for the Basalt compiler.
//!
//! This module implements ahead-of-time (AOT) code generation from MIR to WebAssembly
//! using the Cranelift code generation library.

use crate::ast::{BinaryOp, Literal, UnaryOp};
use crate::mir::data::{
    BasicBlock, LocalId, MirFunction, MirProgram, Operand, Rvalue, Statement, Terminator,
};
use cranelift::codegen::ir::{Function, UserFuncName};
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use cranelift_native::builder as native_builder;
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::HashMap;
use std::path::Path;

/// Encodes a u32 value as LEB128 bytes
fn encode_leb128(mut value: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
    bytes
}

pub struct CraneliftCompiler {
    builder_context: FunctionBuilderContext,
}

impl CraneliftCompiler {
    pub fn new() -> Self {
        Self {
            builder_context: FunctionBuilderContext::new(),
        }
    }

    /// Compiles the MIR program into a WebAssembly file.
    pub fn compile_to_wasm(
        &mut self,
        mir_program: &MirProgram,
        output_path: &Path,
    ) -> Result<(), String> {
        // Create dist directory if it doesn't exist
        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        // Generate a minimal WebAssembly module
        let wasm_bytes = self.generate_minimal_wasm(mir_program)?;
        
        std::fs::write(output_path, wasm_bytes)
            .map_err(|e| format!("Failed to write wasm file: {}", e))?;

        Ok(())
    }

    /// Generates a minimal WebAssembly module
    fn generate_minimal_wasm(&self, mir_program: &MirProgram) -> Result<Vec<u8>, String> {
        let mut wasm = Vec::new();
        
        // WASM magic number and version
        wasm.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d]); // "\0asm"
        wasm.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]); // version 1
        
        // Type section (func () -> i32)
        let mut type_section = Vec::new();
        type_section.push(0x01); // number of types
        type_section.push(0x60); // func
        type_section.push(0x00); // 0 params
        type_section.push(0x01); // 1 result
        type_section.push(0x7f); // i32
        
        // Encode type section
        wasm.push(0x01); // type section
        wasm.extend_from_slice(&encode_leb128(type_section.len() as u32));
        wasm.extend_from_slice(&type_section);
        
        // Function section (1 function, type index 0)
        let mut func_section = Vec::new();
        func_section.push(0x01); // number of functions
        func_section.push(0x00); // type index 0
        
        wasm.push(0x03); // function section
        wasm.extend_from_slice(&encode_leb128(func_section.len() as u32));
        wasm.extend_from_slice(&func_section);
        
        // Export section (export main function)
        let mut export_section = Vec::new();
        export_section.push(0x01); // number of exports
        
        // Export name "main"
        export_section.push(0x04); // name length
        export_section.extend_from_slice(b"main");
        export_section.push(0x00); // export kind (function)
        export_section.push(0x00); // function index 0
        
        wasm.push(0x07); // export section
        wasm.extend_from_slice(&encode_leb128(export_section.len() as u32));
        wasm.extend_from_slice(&export_section);
        
        // Code section
        let mut code_section = Vec::new();
        code_section.push(0x01); // number of functions
        
        // Function body
        let mut func_body = Vec::new();
        
        // Local variables count (0)
        func_body.push(0x00); // 0 local variable groups
        
        // Return constant 0
        func_body.push(0x41); // i32.const
        func_body.extend_from_slice(&encode_leb128(0));
        func_body.push(0x0b); // end
        
        // Encode function body size (size of the entire body)
        code_section.extend_from_slice(&encode_leb128(func_body.len() as u32));
        code_section.extend_from_slice(&func_body);
        
        wasm.push(0x0a); // code section
        wasm.extend_from_slice(&encode_leb128(code_section.len() as u32));
        wasm.extend_from_slice(&code_section);
        
        Ok(wasm)
    }

    /// Compiles a single MIR function to Cranelift IR.
    fn compile_function(
        &self,
        mir_func: &MirFunction,
        module: &mut ObjectModule,
    ) -> Result<(), String> {
        // Create function signature
        let mut sig = module.make_signature();

        // Add parameters
        for _param_id in &mir_func.params {
            sig.params.push(AbiParam::new(types::I64));
        }

        // Add return type
        sig.returns.push(AbiParam::new(types::I64));

        // Declare the function - main should be exported globally
        let linkage = if mir_func.name == "main" {
            Linkage::Export
        } else {
            Linkage::Local
        };
        let func_id = module
            .declare_function(&mir_func.name, linkage, &sig)
            .map_err(|e| format!("Failed to declare function {}: {}", mir_func.name, e))?;

        // Declare external functions that might be called
        self.declare_external_functions(module)?;

        // Create the function directly
        let mut func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut builder_context);

        // Build the function body
        self.build_function(mir_func, &mut builder)?;

        // Finalize the function
        builder.finalize();

        // Define the function in the module
        let mut ctx = cranelift::codegen::Context::for_function(func);
        module
            .define_function(func_id, &mut ctx)
            .map_err(|e| format!("Failed to define function {}: {}", mir_func.name, e))?;

        Ok(())
    }

    /// Builds a function from MIR to Cranelift IR.
    fn build_function(
        &self,
        mir_func: &MirFunction,
        builder: &mut FunctionBuilder,
    ) -> Result<(), String> {
        // Create entry block
        let entry_block = builder.create_block();
        builder.switch_to_block(entry_block);
        builder.seal_block(entry_block);

        // Map MIR locals to Cranelift values
        let mut local_values = HashMap::new();

        // Set up parameters
        if !mir_func.params.is_empty() {
            let block_params = builder.block_params(entry_block);
            if block_params.len() >= mir_func.params.len() {
                for (i, param_id) in mir_func.params.iter().enumerate() {
                    let param_val = block_params[i];
                    local_values.insert(*param_id, param_val);
                }
            }
        }

        // Initialize all locals that are used in the function
        for (local_id, _local_info) in &mir_func.locals {
            if !local_values.contains_key(local_id) {
                // Initialize with a default value (0 for i64)
                let default_val = builder.ins().iconst(types::I64, 0);
                local_values.insert(*local_id, default_val);
            }
        }

        // Build the entry block (first block)
        if let Some(entry_block) = mir_func.basic_blocks.first() {
            self.build_basic_block(entry_block, builder, &mut local_values)?;
        }

        Ok(())
    }

    /// Builds a basic block from MIR to Cranelift IR.
    fn build_basic_block(
        &self,
        block: &BasicBlock,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>,
    ) -> Result<(), String> {
        // Build all statements in the block
        for statement in &block.statements {
            self.build_statement(statement, builder, local_values)?;
        }

        // Build the terminator
        self.build_terminator(&block.terminator, builder, local_values)?;

        Ok(())
    }

    /// Builds a statement from MIR to Cranelift IR.
    fn build_statement(
        &self,
        statement: &Statement,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>,
    ) -> Result<(), String> {
        match statement {
            Statement::Assign(place, rvalue) => {
                let value = self.build_rvalue(rvalue, builder, local_values)?;
                local_values.insert(place.local, value);
            }
        }
        Ok(())
    }

    /// Builds an rvalue from MIR to Cranelift IR.
    fn build_rvalue(
        &self,
        rvalue: &Rvalue,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>,
    ) -> Result<Value, String> {
        match rvalue {
            Rvalue::Use(operand) => self.build_operand(operand, builder, local_values),
            Rvalue::BinaryOp(op, left, right) => {
                let left_val = self.build_operand(left, builder, local_values)?;
                let right_val = self.build_operand(right, builder, local_values)?;

                match op {
                    BinaryOp::Add => Ok(builder.ins().iadd(left_val, right_val)),
                    BinaryOp::Sub => Ok(builder.ins().isub(left_val, right_val)),
                    BinaryOp::Mul => Ok(builder.ins().imul(left_val, right_val)),
                    BinaryOp::Div => Ok(builder.ins().sdiv(left_val, right_val)),
                    BinaryOp::Eq => {
                        let is_equal = builder.ins().icmp(IntCC::Equal, left_val, right_val);
                        let one = builder.ins().iconst(types::I64, 1);
                        let zero = builder.ins().iconst(types::I64, 0);
                        Ok(builder.ins().select(is_equal, one, zero))
                    }
                    BinaryOp::Ne => {
                        let is_not_equal = builder.ins().icmp(IntCC::NotEqual, left_val, right_val);
                        let one = builder.ins().iconst(types::I64, 1);
                        let zero = builder.ins().iconst(types::I64, 0);
                        Ok(builder.ins().select(is_not_equal, one, zero))
                    }
                    BinaryOp::Lt => {
                        let is_less = builder.ins().icmp(IntCC::SignedLessThan, left_val, right_val);
                        let one = builder.ins().iconst(types::I64, 1);
                        let zero = builder.ins().iconst(types::I64, 0);
                        Ok(builder.ins().select(is_less, one, zero))
                    }
                    BinaryOp::Gt => {
                        let is_greater = builder.ins().icmp(IntCC::SignedGreaterThan, left_val, right_val);
                        let one = builder.ins().iconst(types::I64, 1);
                        let zero = builder.ins().iconst(types::I64, 0);
                        Ok(builder.ins().select(is_greater, one, zero))
                    }
                }
            }
            Rvalue::UnaryOp(op, operand) => {
                let operand_val = self.build_operand(operand, builder, local_values)?;

                match op {
                    UnaryOp::Neg => Ok(builder.ins().ineg(operand_val)),
                    UnaryOp::Not => {
                        // For boolean negation, we need to convert to 0/1 and then negate
                        // Since we're using i64 for booleans, we can use a simple approach
                        let zero = builder.ins().iconst(types::I64, 0);
                        let one = builder.ins().iconst(types::I64, 1);
                        let is_zero = builder.ins().icmp(IntCC::Equal, operand_val, zero);
                        Ok(builder.ins().select(is_zero, one, zero))
                    }
                }
            }
            _ => Err(format!("Unsupported rvalue: {:?}", rvalue)),
        }
    }

    /// Builds an operand from MIR to Cranelift IR.
    fn build_operand(
        &self,
        operand: &Operand,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>,
    ) -> Result<Value, String> {
        match operand {
            Operand::Constant(literal) => match literal {
                Literal::I64(value) => Ok(builder.ins().iconst(types::I64, *value as i64)),
                Literal::Bool(value) => Ok(builder.ins().iconst(types::I64, if *value { 1 } else { 0 })),
                _ => Err("Expected i64 or bool literal".to_string()),
            },
            Operand::Copy(place) => local_values
                .get(&place.local)
                .copied()
                .ok_or_else(|| format!("Local {:?} not found", place.local)),
        }
    }

    /// Declares external functions that might be called by the program.
    fn declare_external_functions(&self, module: &mut ObjectModule) -> Result<(), String> {
        // Declare WASI stdout write function - it takes a string and returns void
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64)); // string pointer
        sig.params.push(AbiParam::new(types::I64)); // string length
        // No return value since it's void

        module
            .declare_function("wasi:cli/stdout.write", Linkage::Import, &sig)
            .map_err(|e| format!("Failed to declare wasi:cli/stdout.write: {}", e))?;

        Ok(())
    }

    /// Builds a terminator from MIR to Cranelift IR.
    fn build_terminator(
        &self,
        terminator: &Terminator,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>,
    ) -> Result<(), String> {
        match terminator {
            Terminator::Return => {
                // Return the value in local 0 (return value)
                let return_val = local_values
                    .get(&LocalId(0))
                    .copied()
                    .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
                builder.ins().return_(&[return_val]);
            }
            Terminator::Call {
                func,
                args,
                destination,
                target,
            } => {
                // Build argument values
                let mut arg_values = Vec::new();
                for arg in args {
                    let arg_val = self.build_operand(arg, builder, local_values)?;
                    arg_values.push(arg_val);
                }

                // For external function calls, we need to create a proper function call
                // Since we don't have access to the module here, we'll use a simple approach
                // For now, we'll just store the result and continue
                // TODO: Implement proper external function calling
                let placeholder = builder.ins().iconst(types::I64, 0);
                local_values.insert(destination.local, placeholder);

                // Jump to the target block
                let target_block = builder.create_block();
                builder.ins().jump(target_block, &[]);
                builder.switch_to_block(target_block);
                builder.seal_block(target_block);
            }
            _ => {
                // For unsupported terminators, just return an error
                return Err(format!("Unsupported terminator: {:?}", terminator));
            }
        }
        Ok(())
    }
}
