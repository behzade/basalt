use crate::mir;
use crate::ast;
use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, GlobalSection,
    ImportSection, Instruction, Module, TypeSection, ValType,
};

/// Builds a .wasm module from a MIR program.
pub struct WasmBuilder<'a> {
    module: Module,
    type_section: TypeSection,
    import_section: ImportSection,
    function_section: FunctionSection,
    global_section: GlobalSection,
    export_section: ExportSection,
    code_section: CodeSection,
    mir_program: Option<&'a mir::MirProgram<'a>>,
    // Maps a function name to its index in the Wasm module's function index space.
    function_indices: HashMap<&'a str, u32>,
    // Track the next available type index
    next_type_index: u32,
}

impl<'a> WasmBuilder<'a> {
    pub fn new() -> Self {
        Self {
            module: Module::new(),
            type_section: TypeSection::new(),
            import_section: ImportSection::new(),
            function_section: FunctionSection::new(),
            global_section: GlobalSection::new(),
            export_section: ExportSection::new(),
            code_section: CodeSection::new(),
            mir_program: None,
            function_indices: HashMap::new(),
            next_type_index: 0,
        }
    }

    /// The main entry point for building the module.
    pub fn build(
        &mut self,
        program: &'a mir::MirProgram<'a>,
    ) -> Result<Vec<u8>, String> {
        self.mir_program = Some(program);

        // 1. Process Imports (WASI)
        self.build_imports();

        // 2. Define function types and function bodies
        self.build_functions();

        // 3. Define exports
        self.build_exports();

        // 4. Assemble the final module
        self.module.section(&self.type_section);
        self.module.section(&self.import_section);
        self.module.section(&self.function_section);
        self.module.section(&self.global_section);
        self.module.section(&self.export_section);
        self.module.section(&self.code_section);

        Ok(self.module.clone().finish())
    }

    fn build_imports(&mut self) {
        // Import fd_write for printing to stdout.
        // It has the signature: (i32, i32, i32, i32) -> i32
        // Which corresponds to: (fd, iovs_ptr, iovs_len, nwritten_ptr) -> errno
        let params = vec![ValType::I32, ValType::I32, ValType::I32, ValType::I32];
        let results = vec![ValType::I32];
        self.type_section.ty().function(params, results);
        // Type index 0 for the fd_write function
        let type_index = self.next_type_index;
        self.next_type_index += 1;

        self.import_section.import(
            "wasi_snapshot_preview1", // WASI module name
            "fd_write",
            wasm_encoder::EntityType::Function(type_index),
        );
        self.function_indices.insert("fd_write", 0); // It's the first function, index 0.
    }

    fn build_functions(&mut self) {
        let program = self.mir_program.unwrap();

        // Pass 1: Define all function types and get their type indices.
        let mut func_to_type_index = HashMap::new();
        
        for (name, func) in &program.functions {
            let params: Vec<ValType> = func.params.iter()
                .map(|p| self.mir_ty_to_valtype(&func.locals[p].ty))
                .collect();

            let results: Vec<ValType> = if self.is_unit_type(&func.return_type) {
                vec![]
            } else {
                vec![self.mir_ty_to_valtype(&func.return_type)]
            };

            // Add the function type to the type section
            self.type_section.ty().function(params, results);
            func_to_type_index.insert(*name, self.next_type_index);
            self.next_type_index += 1;
        }

        // Pass 2: Build the function and code sections.
        for (name, func) in &program.functions {
            let type_index = func_to_type_index[name];
            let func_index = self.import_section.len() + self.function_section.len();
            self.function_indices.insert(name, func_index);
            self.function_section.function(type_index);

            // Now, generate the code for the function body
            let wasm_func_body = self.build_function_body(func);
            self.code_section.function(&wasm_func_body);
        }
    }

    fn build_function_body(&self, func: &'a mir::MirFunction<'a>) -> Function {
        // Collect locals in order by LocalId to ensure consistent ordering
        let mut locals_vec: Vec<_> = func.locals
            .iter()
            .filter(|(_, l)| !l.is_param) // Only declare non-parameter locals
            .collect();
        locals_vec.sort_by_key(|(id, _)| id.0); // Sort by LocalId
        
        println!("DEBUG: Found {} non-parameter locals", locals_vec.len());
        for (id, local) in &locals_vec {
            println!("DEBUG: Local {}: {:?} -> {:?}", id.0, local.ty, self.mir_ty_to_valtype(&local.ty));
        }
        
        let local_types: Vec<_> = locals_vec
            .iter()
            .map(|(_, l)| self.mir_ty_to_valtype(&l.ty))
            .collect();
        
        println!("DEBUG: Creating WebAssembly function with {} local types: {:?}", local_types.len(), local_types);
        
        let mut wasm_func = Function::new_with_locals_types(local_types);

        // Build the function body using structured instructions
        self.build_instructions(&func.body, &mut wasm_func, func);

        // Every function must end with a terminating instruction.
        // We add an explicit 'end' for the whole function body.
        wasm_func.instruction(&Instruction::End);
        wasm_func
    }

    /// Recursively builds WebAssembly instructions from structured MIR instructions
    fn build_instructions(
        &self,
        instructions: &'a [mir::MirInstruction<'a>],
        f: &mut Function,
        func: &'a mir::MirFunction<'a>,
    ) {
        for instruction in instructions {
            self.build_instruction(instruction, f, func);
        }
    }

    /// Builds a single MIR instruction into WebAssembly instructions
    fn build_instruction(
        &self,
        instruction: &'a mir::MirInstruction<'a>,
        f: &mut Function,
        func: &'a mir::MirFunction<'a>,
    ) {
        match instruction {
            mir::MirInstruction::Assign(place, rvalue) => {
                // 1. Evaluate the Rvalue and push its result onto the stack.
                self.build_rvalue(rvalue, f, func);
                // 2. Store the result from the stack into the local.
                f.instruction(&Instruction::LocalSet(place.local.0 as u32));
            }
            mir::MirInstruction::Block { body } => {
                // Emit block instruction
                f.instruction(&Instruction::Block(wasm_encoder::BlockType::Empty));
                // Recursively build the body
                self.build_instructions(body, f, func);
                // Emit end instruction
                f.instruction(&Instruction::End);
            }
            mir::MirInstruction::Loop { body } => {
                // Emit loop instruction
                f.instruction(&Instruction::Loop(wasm_encoder::BlockType::Empty));
                // Recursively build the body
                self.build_instructions(body, f, func);
                // Emit end instruction
                f.instruction(&Instruction::End);
            }
            mir::MirInstruction::If { condition, then_block, else_block } => {
                // Build the condition
                self.build_operand(condition, f, func);
                
                // Emit if instruction
                f.instruction(&Instruction::If(wasm_encoder::BlockType::Empty));
                
                // Build the then block
                self.build_instructions(then_block, f, func);
                
                if !else_block.is_empty() {
                    // Emit else instruction
                    f.instruction(&Instruction::Else);
                    // Build the else block
                    self.build_instructions(else_block, f, func);
                }
                
                // Emit end instruction
                f.instruction(&Instruction::End);
            }
            mir::MirInstruction::Break(label) => {
                f.instruction(&Instruction::Br(*label));
            }
            mir::MirInstruction::ConditionalBreak(condition, label) => {
                // Build the condition
                self.build_operand(condition, f, func);
                // Emit br_if instruction
                f.instruction(&Instruction::BrIf(*label));
            }
            mir::MirInstruction::Call { func: func_name, args, destination } => {
                // Push all arguments onto the stack
                for arg in args {
                    self.build_operand(arg, f, func);
                }
                
                // Call the function
                if let Some(func_index) = self.function_indices.get(func_name) {
                    f.instruction(&Instruction::Call(*func_index));
                } else {
                    // If function not found, this is an error
                    f.instruction(&Instruction::Unreachable);
                }
                
                // Store the result in the destination if it's not unit
                if !self.is_unit_type(&func.locals[&destination.local].ty) {
                    f.instruction(&Instruction::LocalSet(destination.local.0 as u32));
                }
            }
            mir::MirInstruction::Perform { effect: _, operation: _, args: _, destination: _ } => {
                // For now, just emit unreachable as a placeholder
                // In a real implementation, this would handle dynamic effect dispatch
                f.instruction(&Instruction::Unreachable);
            }
            mir::MirInstruction::PushHandler { effect: _, handler: _, body } => {
                // For now, just execute the body without handler management
                // In a real implementation, this would push the handler onto a stack
                self.build_instructions(body, f, func);
            }
            mir::MirInstruction::PopHandler => {
                // For now, do nothing
                // In a real implementation, this would pop the handler from the stack
            }
            mir::MirInstruction::Resume { value } => {
                // Build the resume value
                self.build_operand(value, f, func);
                // For now, just continue execution
            }
            mir::MirInstruction::PatternMatch { scrutinee, arms, otherwise } => {
                // Build the scrutinee
                self.build_operand(scrutinee, f, func);
                
                // For now, just execute the first matching arm or the default
                // In a real implementation, this would implement proper pattern matching
                if let Some((_pattern, arm_body)) = arms.first() {
                    self.build_instructions(arm_body, f, func);
                } else {
                    self.build_instructions(otherwise, f, func);
                }
            }
            mir::MirInstruction::Return => {
                // The return value should be in local[0] (the return local).
                // We need to put it on the stack before returning.
                f.instruction(&Instruction::LocalGet(0));
                f.instruction(&Instruction::Return);
            }
            mir::MirInstruction::Unreachable => {
                f.instruction(&Instruction::Unreachable);
            }
        }
    }



    fn build_rvalue(&self, rvalue: &'a mir::Rvalue, f: &mut Function, func: &'a mir::MirFunction<'a>) {
        match rvalue {
            mir::Rvalue::Use(op) => self.build_operand(op, f, func),
            mir::Rvalue::BinaryOp(op, lhs, rhs) => {
                // Determine the operand types to choose the correct operation
                let lhs_type = self.get_operand_type(lhs, func);
                let rhs_type = self.get_operand_type(rhs, func);
                
                // Use the wider type for the operation (i64 > i32)
                let operation_type = if matches!(lhs_type, ValType::I64) || matches!(rhs_type, ValType::I64) {
                    ValType::I64
                } else {
                    ValType::I32
                };
                
                self.build_operand(lhs, f, func); // Push LHS
                self.build_operand(rhs, f, func); // Push RHS
                
                let instruction = match (op, operation_type) {
                    (ast::BinaryOp::Add, ValType::I32) => Instruction::I32Add,
                    (ast::BinaryOp::Add, ValType::I64) => Instruction::I64Add,
                    (ast::BinaryOp::Sub, ValType::I32) => Instruction::I32Sub,
                    (ast::BinaryOp::Sub, ValType::I64) => Instruction::I64Sub,
                    (ast::BinaryOp::Mul, ValType::I32) => Instruction::I32Mul,
                    (ast::BinaryOp::Mul, ValType::I64) => Instruction::I64Mul,
                    (ast::BinaryOp::Div, ValType::I32) => Instruction::I32DivS, // Signed division
                    (ast::BinaryOp::Div, ValType::I64) => Instruction::I64DivS, // Signed division
                    (ast::BinaryOp::Eq, ValType::I32) => Instruction::I32Eq,
                    (ast::BinaryOp::Eq, ValType::I64) => Instruction::I64Eq,
                    (ast::BinaryOp::Ne, ValType::I32) => Instruction::I32Ne,
                    (ast::BinaryOp::Ne, ValType::I64) => Instruction::I64Ne,
                    (ast::BinaryOp::Lt, ValType::I32) => Instruction::I32LtS,
                    (ast::BinaryOp::Lt, ValType::I64) => Instruction::I64LtS,
                    (ast::BinaryOp::Gt, ValType::I32) => Instruction::I32GtS,
                    (ast::BinaryOp::Gt, ValType::I64) => Instruction::I64GtS,
                    _ => Instruction::Nop,
                };
                f.instruction(&instruction);
            },
            // ... Other Rvalue types
            _ => {}
        }
    }

    /// Helper function to determine the WebAssembly type of an operand
    fn get_operand_type(&self, op: &'a mir::Operand, func: &'a mir::MirFunction<'a>) -> ValType {
        match op {
            mir::Operand::Constant(lit) => match lit {
                ast::Literal::I32(_) => ValType::I32,
                ast::Literal::I64(_) => ValType::I64,
                ast::Literal::Bool(_) => ValType::I32,
                ast::Literal::F64(_) => ValType::F64,
                ast::Literal::Str(_) => ValType::I32, // Placeholder
                ast::Literal::Unit => ValType::I32,
            },
            mir::Operand::Copy(place) => {
                // Get the type from the local variable
                let local = &func.locals[&place.local];
                self.mir_ty_to_valtype(&local.ty)
            }
        }
    }

    fn build_operand(&self, op: &'a mir::Operand, f: &mut Function, _func: &'a mir::MirFunction<'a>) {
        match op {
            mir::Operand::Constant(lit) => {
                let instruction = match lit {
                    ast::Literal::I32(v) => Instruction::I32Const(*v),
                    ast::Literal::I64(v) => {
                        // Always generate i64 constant for i64 literals
                        Instruction::I64Const(*v)
                    },
                    ast::Literal::Bool(v) => Instruction::I32Const(if *v { 1 } else { 0 }),
                    ast::Literal::F64(v) => Instruction::F64Const(*v),
                    ast::Literal::Str(_s) => {
                        // For strings, we'll need to handle them differently
                        // For now, just use a placeholder
                        Instruction::I32Const(0)
                    }
                    ast::Literal::Unit => Instruction::I32Const(0),
                };
                f.instruction(&instruction);
            }
            mir::Operand::Copy(place) => {
                // Push the value of the local onto the stack.
                f.instruction(&Instruction::LocalGet(place.local.0 as u32));
            }
        }
    }



    fn build_exports(&mut self) {
        if let Some(main_index) = self.function_indices.get("main") {
            self.export_section.export(
                "_start", // WASI expects a `_start` function for the entry point
                ExportKind::Func,
                *main_index,
            );
        }
    }

    /// Converts a MIR type to a Wasm ValType.
    fn mir_ty_to_valtype(&self, ty: &crate::hir::Ty) -> ValType {
        match ty {
            crate::hir::Ty::I32 | crate::hir::Ty::Bool => ValType::I32,
            crate::hir::Ty::I64 => ValType::I64,
            crate::hir::Ty::F64 => ValType::F64,
            // For GC types, we use `i32` as a placeholder until full GC support is widespread.
            // This is a temporary solution - in a real implementation, you'd need proper GC support.
            crate::hir::Ty::Array(_) | crate::hir::Ty::Map {..} | crate::hir::Ty::Adt {..} => ValType::I32,
            // A string could be a reference to a GC-managed object.
            crate::hir::Ty::Str => ValType::I32,
            _ => ValType::I32, // Default/Unit
        }
    }

    fn is_unit_type(&self, ty: &crate::hir::Ty) -> bool {
        matches!(ty, crate::hir::Ty::Unit)
    }
} 