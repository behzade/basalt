//! Native code generation from Cranelift IR.
//!
//! This module converts Cranelift IR functions into native object files.

use cranelift::codegen::ir::{self, Function};
use cranelift_codegen::isa::CallConv;
use cranelift_codegen::Context;
use cranelift_codegen::settings;
use cranelift_native;
use cranelift_object::{ObjectBuilder, ObjectModule};
use cranelift_module::{Module, Linkage};
use cranelift::prelude::{Signature, AbiParam, Configurable};
use std::collections::HashMap;

/// Native code generator that converts Cranelift IR to object files.
pub struct NativeCodegen;

impl NativeCodegen {
    /// Convert Cranelift IR functions to native object file bytes.
    pub fn generate_object(functions: HashMap<String, Function>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // Set up Cranelift settings for native compilation
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();
        flag_builder.set("opt_level", "speed").unwrap();
        let isa_builder = cranelift_native::builder().unwrap_or_else(|_| {
            panic!("host machine is not a supported target");
        });
        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).unwrap();
        
        // Create an object builder
        let builder = ObjectBuilder::new(isa, "output.o", cranelift_module::default_libcall_names())?;
        let mut module = ObjectModule::new(builder);
        
        // Add functions to the module
        for (name, func) in functions {
            // Convert function signature
            let signature = Self::convert_signature(&func.signature);
            // Use Export linkage for main function, Local for others
            let linkage = if name == "main" { Linkage::Export } else { Linkage::Local };
            let func_id = module.declare_function(&name, linkage, &signature)?;
            
            // Create a context for compilation
            let mut context = Context::new();
            context.func = func;
            
            // Define the function in the module (this will compile it)
            module.define_function(func_id, &mut context).map_err(|e| {
                format!("Compilation error for function '{}': {:?}", name, e)
            })?;
        }
        
        // Finalize the module and get the object product
        let product = module.finish();
        let object_bytes = product.emit()?;
        
        Ok(object_bytes)
    }
    
    /// Convert Cranelift function signature to module signature.
    fn convert_signature(signature: &ir::Signature) -> Signature {
        let mut module_sig = Signature::new(CallConv::SystemV);
        
        // Convert parameters
        for param in &signature.params {
            module_sig.params.push(AbiParam::new(param.value_type));
        }
        
        // Convert return values
        for ret in &signature.returns {
            module_sig.returns.push(AbiParam::new(ret.value_type));
        }
        
        module_sig
    }
} 
