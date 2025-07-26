//! src/codegen/mod.rs
//!
//! A backend for compiling MIR directly to a .wasm file.
//! This backend does NOT use Cranelift or any other IR.

mod builder;

use crate::mir;
pub use builder::WasmBuilder;

pub fn compile_program_to_wasm(program: &mir::MirProgram) -> Result<Vec<u8>, String> {
    let mut builder = WasmBuilder::new();
    builder.build(program)
} 