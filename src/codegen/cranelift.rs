//! Cranelift backend for the Basalt compiler.
//! 
//! This module implements ahead-of-time (AOT) code generation from MIR to object files
//! using the Cranelift code generation library.

use crate::mir::data::{MirProgram, MirFunction, BasicBlock, Statement, Rvalue, Operand, Terminator, LocalId};
use crate::ast::{Literal, BinaryOp};
use cranelift::prelude::*;
use cranelift::codegen::ir::{Function, UserFuncName};
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_native::builder as native_builder;
use cranelift_module::{Linkage, Module};
use std::collections::HashMap;
use std::path::Path;

pub struct CraneliftCompiler {
    builder_context: FunctionBuilderContext,
}

impl CraneliftCompiler {
    pub fn new() -> Self {
        Self {
            builder_context: FunctionBuilderContext::new(),
        }
    }

    /// Compiles the MIR program into an executable in the dist folder.
    pub fn compile_to_executable(&mut self, mir_program: &MirProgram, output_name: &str) -> Result<(), String> {
        // Create dist directory if it doesn't exist
        let dist_dir = std::path::Path::new("dist");
        std::fs::create_dir_all(dist_dir)
            .map_err(|e| format!("Failed to create dist directory: {}", e))?;
        
        // Create object builder and module
        let isa = native_builder()
            .map_err(|e| format!("Failed to create native builder: {}", e))?
            .finish(settings::Flags::new(settings::builder()))
            .map_err(|e| format!("Failed to create ISA: {}", e))?;
        
        let mut module = ObjectModule::new(
            ObjectBuilder::new(isa, "basalt", cranelift_module::default_libcall_names())
                .map_err(|e| format!("Failed to create object builder: {}", e))?
        );
        
        // Compile all functions
        for (_name, mir_func) in &mir_program.functions {
            self.compile_function(mir_func, &mut module)?;
        }
        
        // Write the object file to a temporary location
        let temp_obj = std::env::temp_dir().join("basalt_temp.o");
        let object_data = module.finish();
        std::fs::write(&temp_obj, object_data.emit().unwrap())
            .map_err(|e| format!("Failed to write object file: {}", e))?;
        
        // Build the runtime library
        let runtime_dir = std::path::Path::new("runtime");
        let status = std::process::Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(runtime_dir)
            .status()
            .map_err(|e| format!("Failed to build runtime: {}", e))?;
        
        if !status.success() {
            return Err("Failed to build runtime".to_string());
        }
        
        // Link everything together
        let executable_path = dist_dir.join(output_name);
        let runtime_lib = runtime_dir.join("target/release/libbasalt_runtime.a");
        
        let status = std::process::Command::new("cc")
            .args(&[
                "-o", executable_path.to_str().unwrap(),
                temp_obj.to_str().unwrap(),
                runtime_lib.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| format!("Failed to link: {}", e))?;
        
        if !status.success() {
            return Err("Failed to link executable".to_string());
        }
        
        Ok(())
    }
    
    /// Compiles the MIR program into an object file.
    pub fn compile_to_object(&mut self, mir_program: &MirProgram, output_path: &Path) -> Result<(), String> {
        // Create object builder and module
        let isa = native_builder()
            .map_err(|e| format!("Failed to create native builder: {}", e))?
            .finish(settings::Flags::new(settings::builder()))
            .map_err(|e| format!("Failed to create ISA: {}", e))?;
        
        let mut module = ObjectModule::new(
            ObjectBuilder::new(isa, "basalt", cranelift_module::default_libcall_names())
                .map_err(|e| format!("Failed to create object builder: {}", e))?
        );
        
        // Compile all functions
        for (_name, mir_func) in &mir_program.functions {
            self.compile_function(mir_func, &mut module)?;
        }
        
        // Write the object file
        let object_data = module.finish();
        std::fs::write(output_path, object_data.emit().unwrap())
            .map_err(|e| format!("Failed to write object file: {}", e))?;
        
        Ok(())
    }

    /// Compiles the MIR program and returns the main function's return value.
    /// This is a convenience method for testing that compiles and executes the program.
    pub fn compile_and_run(&mut self, mir_program: &MirProgram) -> Result<i64, String> {
        // Create a temporary object file
        let temp_obj = std::env::temp_dir().join("basalt_temp.o");
        self.compile_to_object(mir_program, &temp_obj)?;
        
        // Build the runtime library
        let runtime_dir = std::path::Path::new("runtime");
        let status = std::process::Command::new("cargo")
            .args(&["build", "--release"])
            .current_dir(runtime_dir)
            .status()
            .map_err(|e| format!("Failed to build runtime: {}", e))?;
        
        if !status.success() {
            return Err("Failed to build runtime".to_string());
        }
        
        // Link everything together
        let executable = std::env::temp_dir().join("basalt_exec");
        let runtime_lib = runtime_dir.join("target/release/libbasalt_runtime.a");
        
        let status = std::process::Command::new("cc")
            .args(&[
                "-o", executable.to_str().unwrap(),
                temp_obj.to_str().unwrap(),
                runtime_lib.to_str().unwrap(),
            ])
            .status()
            .map_err(|e| format!("Failed to link: {}", e))?;
        
        if !status.success() {
            return Err("Failed to link executable".to_string());
        }
        
        // Run the executable and capture the output
        let output = std::process::Command::new(executable)
            .output()
            .map_err(|e| format!("Failed to run executable: {}", e))?;
        
        if !output.status.success() {
            return Err("Executable failed to run".to_string());
        }
        
        // Parse the return value from stdout
        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.trim().parse::<i64>()
            .map_err(|e| format!("Failed to parse output: {}", e))
    }
    
    /// Compiles a single MIR function to Cranelift IR.
    fn compile_function(&self, mir_func: &MirFunction, module: &mut ObjectModule) -> Result<(), String> {
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
        let func_id = module.declare_function(&mir_func.name, linkage, &sig)
            .map_err(|e| format!("Failed to declare function {}: {}", mir_func.name, e))?;
        
        // Create the function directly
        let mut func = Function::with_name_signature(
            UserFuncName::user(0, func_id.as_u32()),
            sig
        );
        
        let mut builder_context = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut func, &mut builder_context);
        
        // Build the function body
        self.build_function(mir_func, &mut builder)?;
        
        // Finalize the function
        builder.finalize();
        
        // Define the function in the module
        let mut ctx = cranelift::codegen::Context::for_function(func);
        module.define_function(func_id, &mut ctx)
            .map_err(|e| format!("Failed to define function {}: {}", mir_func.name, e))?;
        
        Ok(())
    }
    
    /// Builds a function from MIR to Cranelift IR.
    fn build_function(&self, mir_func: &MirFunction, builder: &mut FunctionBuilder) -> Result<(), String> {
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
        local_values: &mut HashMap<LocalId, Value>
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
        local_values: &mut HashMap<LocalId, Value>
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
        local_values: &mut HashMap<LocalId, Value>
    ) -> Result<Value, String> {
        match rvalue {
            Rvalue::Use(operand) => {
                self.build_operand(operand, builder, local_values)
            }
            Rvalue::BinaryOp(op, left, right) => {
                let left_val = self.build_operand(left, builder, local_values)?;
                let right_val = self.build_operand(right, builder, local_values)?;
                
                match op {
                    BinaryOp::Add => Ok(builder.ins().iadd(left_val, right_val)),
                    BinaryOp::Sub => Ok(builder.ins().isub(left_val, right_val)),
                    BinaryOp::Mul => Ok(builder.ins().imul(left_val, right_val)),
                    BinaryOp::Div => Ok(builder.ins().sdiv(left_val, right_val)),
                    _ => Err(format!("Unsupported binary operation: {:?}", op)),
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
        local_values: &mut HashMap<LocalId, Value>
    ) -> Result<Value, String> {
        match operand {
            Operand::Constant(literal) => {
                match literal {
                    Literal::I64(value) => {
                        Ok(builder.ins().iconst(types::I64, *value as i64))
                    }
                    _ => Err("Expected i64 literal".to_string()),
                }
            }
            Operand::Copy(place) => {
                local_values.get(&place.local)
                    .copied()
                    .ok_or_else(|| format!("Local {:?} not found", place.local))
            }
        }
    }
    
    /// Builds a terminator from MIR to Cranelift IR.
    fn build_terminator(
        &self,
        terminator: &Terminator,
        builder: &mut FunctionBuilder,
        local_values: &mut HashMap<LocalId, Value>
    ) -> Result<(), String> {
        match terminator {
            Terminator::Return => {
                // Return the value in local 0 (return value)
                let return_val = local_values.get(&LocalId(0))
                    .copied()
                    .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
                builder.ins().return_(&[return_val]);
            }
            Terminator::Call { func: _func, args, destination, target: _target } => {
                // For now, we'll handle simple function calls
                // In a full implementation, we'd need to handle function pointers
                let mut arg_values = Vec::new();
                for arg in args {
                    let arg_val = self.build_operand(arg, builder, local_values)?;
                    arg_values.push(arg_val);
                }
                
                // Create a call to the function - for now, just use a placeholder
                // In a real implementation, you'd need to resolve the function reference
                // For now, we'll create a dummy function reference
                let _dummy_func_ref = builder.ins().iconst(types::I64, 0); // Placeholder
                
                // Note: This is a simplified approach. In a real implementation,
                // you'd need to properly resolve the function reference and create
                // a proper FuncRef. For now, we'll just store a placeholder result.
                
                // Store the result
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
