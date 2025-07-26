use crate::mir;
use crate::ast;
use crate::hir;
use std::collections::HashMap;
use wasm_encoder::{
    CodeSection, ExportKind, ExportSection, Function, FunctionSection, GlobalSection,
    ImportSection, Instruction, Module, TypeSection, ValType, DataSection, HeapType, FieldType, StorageType,
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
    data_section: DataSection,
    mir_program: Option<&'a mir::MirProgram<'a>>,
    // Maps a function name to its index in the Wasm module's function index space.
    function_indices: HashMap<&'a str, u32>,
    // Track the next available type index
    next_type_index: u32,
    // Maps HIR types to their Wasm type indices for GC types
    type_map: HashMap<hir::Ty<'a>, u32>,
    // Track string literals and their data segment offsets
    string_literals: HashMap<&'a str, u32>,
    // Buffer for string data
    string_data: Vec<u8>,
    // Track struct definitions for field access
    struct_defs: HashMap<&'a str, Vec<&'a str>>,
    // Track actual struct definitions from HIR program
    hir_struct_defs: HashMap<&'a str, hir::StructDef<'a>>,
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
            data_section: DataSection::new(),
            mir_program: None,
            function_indices: HashMap::new(),
            next_type_index: 0,
            type_map: HashMap::new(),
            string_literals: HashMap::new(),
            string_data: Vec::new(),
            struct_defs: HashMap::new(),
            hir_struct_defs: HashMap::new(),
        }
    }

    /// The main entry point for building the module.
    pub fn build(
        &mut self,
        mir_program: &'a mir::MirProgram<'a>,
        _hir_program: &'a [hir::Item<'a>],
    ) -> Result<Vec<u8>, String> {
        self.mir_program = Some(mir_program);

        // 1. Process Imports (WASI)
        self.build_imports();

        // 2. Collect struct definitions from MIR program
        self.collect_struct_definitions(mir_program);

        // 3. Collect string literals and populate data section
        self.collect_string_literals();

        // 4. Collect and define all aggregate types
        self.collect_and_define_types();

        // 5. Define function types and function bodies
        self.build_functions();

        // 6. Define exports
        self.build_exports();

        // 7. Assemble the final module
        self.module.section(&self.type_section);
        self.module.section(&self.import_section);
        self.module.section(&self.function_section);
        self.module.section(&self.global_section);
        self.module.section(&self.export_section);
        self.module.section(&self.code_section);
        self.module.section(&self.data_section);

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

    /// Collect struct definitions from the MIR program
    fn collect_struct_definitions(&mut self, program: &'a mir::MirProgram<'a>) {
        // Clear any existing struct definitions
        self.hir_struct_defs.clear();
        
        // Iterate over all struct definitions from the MIR program
        for (struct_name, struct_def) in &program.structs {
            self.hir_struct_defs.insert(struct_name, struct_def.clone());
        }
    }

    /// Collect string literals from the MIR program and populate the data section.
    fn collect_string_literals(&mut self) {
        let program = self.mir_program.unwrap();
        let mut current_offset: u32 = 0;
        self.string_data.clear();
        self.string_literals.clear();

        // Collect string literals from all instructions
        for (_name, func) in &program.functions {
            for instruction in &func.body {
                if let mir::MirInstruction::Assign(_, mir::Rvalue::Use(mir::Operand::Constant(ast::Literal::Str(s)))) = instruction {
                    if !self.string_literals.contains_key(s) {
                        self.string_literals.insert(s, current_offset);
                        self.string_data.extend_from_slice(s.as_bytes());
                        current_offset += s.len() as u32;
                    }
                }
            }
        }

        // Add a passive data segment for all string bytes if any
        if !self.string_data.is_empty() {
            self.data_section.passive(self.string_data.clone());
        }
    }

    /// Collect all aggregate types from the MIR program and define them in the type section
    fn collect_and_define_types(&mut self) {
        let program = self.mir_program.unwrap();
        
        // First pass: collect all unique aggregate types
        let mut unique_types = std::collections::HashSet::new();
        
        for (_name, func) in &program.functions {
            // Collect types from function parameters and locals
            for local in func.locals.values() {
                self.collect_aggregate_types(&local.ty, &mut unique_types);
            }
            
            // Collect types from function body
            self.collect_types_from_instructions(&func.body, &mut unique_types);
        }
        
        // Second pass: define types in the type section
        for ty in unique_types {
            self.define_type(&ty);
        }
        
        // Define string type: array of i8 (UTF-8 bytes)
        self.define_string_type();
    }
    
    /// Recursively collect aggregate types from a HIR type
    fn collect_aggregate_types(&self, ty: &hir::Ty<'a>, unique_types: &mut std::collections::HashSet<hir::Ty<'a>>) {
        match ty {
            hir::Ty::Adt { .. } | hir::Ty::Array(_) => {
                unique_types.insert(ty.clone());
                // Also collect nested types
                match ty {
                    hir::Ty::Adt { generics, .. } => {
                        for generic_ty in generics {
                            self.collect_aggregate_types(generic_ty, unique_types);
                        }
                    }
                    hir::Ty::Array(element_type) => {
                        self.collect_aggregate_types(element_type, unique_types);
                    }
                    _ => {}
                }
            }
            hir::Ty::Map { key, value } => {
                self.collect_aggregate_types(key, unique_types);
                self.collect_aggregate_types(value, unique_types);
            }
            _ => {}
        }
    }
    
    /// Collect types from MIR instructions
    fn collect_types_from_instructions(&self, instructions: &[mir::MirInstruction<'a>], unique_types: &mut std::collections::HashSet<hir::Ty<'a>>) {
        for instruction in instructions {
            match instruction {
                mir::MirInstruction::Assign(_, rvalue) => {
                    self.collect_types_from_rvalue(rvalue, unique_types);
                }
                mir::MirInstruction::Block { body } | mir::MirInstruction::Loop { body } => {
                    self.collect_types_from_instructions(body, unique_types);
                }
                mir::MirInstruction::If { then_block, else_block, .. } => {
                    self.collect_types_from_instructions(then_block, unique_types);
                    self.collect_types_from_instructions(else_block, unique_types);
                }
                mir::MirInstruction::PushHandler { body, .. } => {
                    self.collect_types_from_instructions(body, unique_types);
                }
                mir::MirInstruction::PatternMatch { arms, otherwise, .. } => {
                    for (_pattern, arm_body) in arms {
                        self.collect_types_from_instructions(arm_body, unique_types);
                    }
                    self.collect_types_from_instructions(otherwise, unique_types);
                }
                _ => {}
            }
        }
    }
    
    /// Collect types from Rvalue
    fn collect_types_from_rvalue(&self, rvalue: &mir::Rvalue<'a>, unique_types: &mut std::collections::HashSet<hir::Ty<'a>>) {
        match rvalue {
            mir::Rvalue::StructInit { path, .. } => {
                // For now, we'll use a simple string-based approach
                // In a real implementation, you'd resolve the path to get the actual type
                let adt_type = hir::Ty::Adt {
                    name: vec![path], // Path is Vec<&str>
                    generics: vec![],
                };
                unique_types.insert(adt_type);
            }
            mir::Rvalue::Array(_) => {
                // We'll need to determine the element type from the operands
                // For now, we'll use a placeholder
                let array_type = hir::Ty::Array(Box::new(hir::Ty::I32));
                unique_types.insert(array_type);
            }
            _ => {}
        }
    }
    
    /// Define a type in the type section
    fn define_type(&mut self, ty: &hir::Ty<'a>) {
        match ty {
            hir::Ty::Adt { name, generics: _ } => {
                // Look up the actual struct definition
                if let Some(struct_name) = name.first() {
                    if let Some(struct_def) = self.hir_struct_defs.get(struct_name) {
                        // Convert field types to FieldTypes
                        let mut fields = Vec::new();
                        let mut field_names = Vec::new();
                        
                        for (field_name, field_type) in &struct_def.fields {
                            let val_type = self.hir_ty_to_valtype(field_type);
                            fields.push(FieldType {
                                element_type: StorageType::Val(val_type),
                                mutable: false,
                            });
                            field_names.push(*field_name);
                        }
                        
                        // Create the struct type using WasmGC
                        self.type_section.ty().struct_(fields);
                        
                        // Store the type index and field names
                        self.type_map.insert(ty.clone(), self.next_type_index);
                        self.struct_defs.insert(struct_name, field_names);
                        self.next_type_index += 1;
                    } else {
                        // Fallback: create a simple struct with i32 fields
                        let field_names = vec!["field0", "field1"];
                        let fields = vec![
                            FieldType {
                                element_type: StorageType::Val(ValType::I32),
                                mutable: false,
                            },
                            FieldType {
                                element_type: StorageType::Val(ValType::I32),
                                mutable: false,
                            },
                        ];
                        self.type_section.ty().struct_(fields);
                        
                        self.type_map.insert(ty.clone(), self.next_type_index);
                        self.struct_defs.insert(struct_name, field_names);
                        self.next_type_index += 1;
                    }
                }
            }
            hir::Ty::Array(element_type) => {
                let element_valtype = self.hir_ty_to_valtype(element_type);
                
                // Create the array type using WasmGC
                let field_type = FieldType {
                    element_type: StorageType::Val(element_valtype),
                    mutable: false,
                };
                self.type_section.ty().array(&field_type.element_type, field_type.mutable);
                
                self.type_map.insert(ty.clone(), self.next_type_index);
                self.next_type_index += 1;
            }
            _ => {}
        }
    }
    
    /// Define the string type: array of i8 (UTF-8 bytes)
    fn define_string_type(&mut self) {
        // Define array type for UTF-8 bytes (use I32 for now)
        let array_type_index = self.next_type_index;
        self.type_section.ty().array(&StorageType::Val(ValType::I32), false);
        self.type_map.insert(hir::Ty::Array(Box::new(hir::Ty::I32)), array_type_index);
        self.next_type_index += 1;

        // Define struct type: (i32, array i32)
        let struct_type_index = self.next_type_index;
        self.type_section.ty().struct_([
            FieldType {
                element_type: StorageType::Val(ValType::I32),
                mutable: false,
            },
            FieldType {
                element_type: StorageType::Val(ValType::Ref(wasm_encoder::RefType {
                    nullable: false,
                    heap_type: HeapType::Concrete(array_type_index),
                })),
                mutable: false,
            },
        ]);
        self.type_map.insert(hir::Ty::Str, struct_type_index);
        self.next_type_index += 1;
    }
    
    /// Convert HIR type to Wasm ValType
    fn hir_ty_to_valtype(&self, ty: &hir::Ty<'a>) -> ValType {
        match ty {
            hir::Ty::I32 | hir::Ty::Bool => ValType::I32,
            hir::Ty::I64 => ValType::I64,
            hir::Ty::F64 => ValType::F64,
            hir::Ty::Str => {
                if let Some(type_index) = self.type_map.get(ty) {
                    // Return a reference to the string type
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(*type_index),
                    })
                } else {
                    ValType::I32 // Fallback
                }
            }
            hir::Ty::Array(_) | hir::Ty::Adt { .. } => {
                if let Some(type_index) = self.type_map.get(ty) {
                    // Return a reference to the GC type
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(*type_index),
                    })
                } else {
                    ValType::I32 // Fallback
                }
            }
            _ => ValType::I32,
        }
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
            mir::Rvalue::StructInit { path, fields } => {
                // Look up the type_index for the struct
                let adt_type = hir::Ty::Adt {
                    name: vec![path],
                    generics: vec![],
                };
                
                if let Some(type_index) = self.type_map.get(&adt_type) {
                    // Get the field names in order
                    if let Some(field_names) = self.struct_defs.get(path) {
                        // Iterate through the struct's fields in their defined order
                        for field_name in field_names {
                            if let Some(operand) = fields.get(field_name) {
                                self.build_operand(operand, f, func);
                            } else {
                                // If field is missing, push a default value (0 for i32)
                                f.instruction(&Instruction::I32Const(0));
                            }
                        }
                        
                        // Use proper WasmGC instruction to create the struct
                        f.instruction(&Instruction::StructNew(*type_index));
                    }
                } else {
                    // Fallback: just push a placeholder value
                    f.instruction(&Instruction::I32Const(0));
                }
            },
            mir::Rvalue::Array(elements) => {
                // Look up the type_index for the array
                let array_type = hir::Ty::Array(Box::new(hir::Ty::I32)); // Assume i32 elements for now
                
                if let Some(type_index) = self.type_map.get(&array_type) {
                    // Push the number of elements as an i32 constant onto the stack
                    f.instruction(&Instruction::I32Const(elements.len() as i32));
                    
                    // Iterate through the elements and generate instructions to push each element's value onto the stack
                    for element in elements {
                        self.build_operand(element, f, func);
                    }
                    
                    // Use proper WasmGC instruction to create the array
                    f.instruction(&Instruction::ArrayNew(*type_index));
                } else {
                    // Fallback: just push a placeholder value
                    f.instruction(&Instruction::I32Const(0));
                }
            },
            mir::Rvalue::Projection { base, field } => {
                // Generate Instruction::LocalGet for the base Place to push the struct reference onto the stack
                f.instruction(&Instruction::LocalGet(base.local.0 as u32));
                
                // Get the actual type of the base variable
                let local = &func.locals[&base.local];
                let base_type = &local.ty;
                
                // Look up the type_index of the struct's type
                if let Some(type_index) = self.type_map.get(base_type) {
                    // Find the struct name from the type
                    if let hir::Ty::Adt { name, .. } = base_type {
                        if let Some(struct_name) = name.first() {
                            // Determine the field_idx by finding the field's position in the original struct definition
                            if let Some(field_names) = self.struct_defs.get(struct_name) {
                                if let Some(field_idx) = field_names.iter().position(|&f| f == *field) {
                                    // Use proper WasmGC instruction to get the field
                                    f.instruction(&Instruction::StructGet {
                                        struct_type_index: *type_index,
                                        field_index: field_idx as u32,
                                    });
                                } else {
                                    f.instruction(&Instruction::I32Const(0)); // Field not found
                                }
                            } else {
                                f.instruction(&Instruction::I32Const(0)); // Struct definition not found
                            }
                        } else {
                            f.instruction(&Instruction::I32Const(0)); // No struct name
                        }
                    } else {
                        f.instruction(&Instruction::I32Const(0)); // Not an ADT type
                    }
                } else {
                    f.instruction(&Instruction::I32Const(0)); // Type not found
                }
            },
            // ... Other Rvalue types
            _ => {
                // For any other Rvalue types, push a placeholder value
                f.instruction(&Instruction::I32Const(0));
            }
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
                ast::Literal::Str(_) => {
                    // Return the correct string type
                    if let Some(type_index) = self.type_map.get(&hir::Ty::Str) {
                        ValType::Ref(wasm_encoder::RefType {
                            nullable: true,
                            heap_type: HeapType::Concrete(*type_index),
                        })
                    } else {
                        ValType::I32 // Fallback
                    }
                },
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
                    ast::Literal::F64(v) => Instruction::F64Const((*v).into()),
                    ast::Literal::Str(s) => {
                        // Handle string literals with GC support
                        self.build_string_literal(s, f);
                        return; // Return early since we've already emitted the instruction
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
    
    /// Build a string literal using ArrayNewFixed
    fn build_string_literal(&self, s: &str, f: &mut Function) {
        // Get type indices
        let array_type_index = *self.type_map.get(&hir::Ty::Array(Box::new(hir::Ty::I32))).expect("array i32 type");
        let struct_type_index = *self.type_map.get(&hir::Ty::Str).expect("string struct type");
        let len = s.len() as i32;

        // Push string length
        f.instruction(&Instruction::I32Const(len));
        
        // Push each byte of the string onto the stack
        for byte in s.bytes() {
            f.instruction(&Instruction::I32Const(byte as i32));
        }
        
        // Create array with the fixed size and initial values
        f.instruction(&Instruction::ArrayNewFixed { array_type_index, array_size: len as u32 });
        
        // Create struct (size, array)
        f.instruction(&Instruction::StructNew(struct_type_index));
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
            crate::hir::Ty::Str => {
                if let Some(type_index) = self.type_map.get(ty) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(*type_index),
                    })
                } else {
                    ValType::I32 // Fallback
                }
            }
            crate::hir::Ty::Array(_) | crate::hir::Ty::Adt { .. } => {
                if let Some(type_index) = self.type_map.get(ty) {
                    ValType::Ref(wasm_encoder::RefType {
                        nullable: true,
                        heap_type: HeapType::Concrete(*type_index),
                    })
                } else {
                    ValType::I32 // Fallback
                }
            }
            _ => ValType::I32,
        }
    }

    fn is_unit_type(&self, ty: &crate::hir::Ty) -> bool {
        matches!(ty, crate::hir::Ty::Unit)
    }
} 