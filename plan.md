## **AST & Parser Fixes**

Your parser is trying to handle comments by treating them as `Stmt::Error`, which is incorrect and causes downstream issues. The lexer should identify comments, and the parser should ignore them.

**Action Items:**

**Define a "Trivia" Parser:** In `lexer.rs`, create a parser that recognizes ignored tokens.  
Rust  
// In lexer() function  
let comment \= ...; // Your existing comment parser  
let trivia \= comment.or(text::whitespace().ignored()).repeated();

1. 

**Use `.padded_by()`:** In `parser.rs`, instead of just chaining parsers or using `.padded()`, use `.padded_by(trivia)` for tokens and rules that can be separated by comments or whitespace. This cleanly consumes them.  
Rust  
// Example in item\_parser()  
let item \= item\_parser().padded\_by(trivia.clone());  
item.repeated().collect()

2.   
3. **Remove Parser Hack:** In `parser.rs`, remove the `.or(select! { Token::Comment(_) => Stmt::Error })` logic from your `block` parser. It's no longer necessary.

---

## **HIR (`hir.rs`) Changes**

The HIR is missing key information that is present in the AST, which prevents correct MIR lowering.

**Action Items:**

**Add Generics to `hir::Function`:** Your language supports generic functions, but the HIR doesn't represent them. Add the generics list to track them before monomorphization.  
Rust  
// In src/hir.rs  
pub struct Function\<'src\> {  
    pub name: &'src str,  
    pub generics: Vec\<&'src str\>, // \<-- ADD THIS  
    pub params: Vec\<(Option\<&'src str\>, Ty\<'src\>)\>,  
    // ...  
}

1.   
2. **Preserve Spans:** For better error reporting in later stages, uncomment the `span` field in `hir::Expr` and other relevant HIR nodes. Ensure this is populated during the AST \-\> HIR conversion.

---

## **MIR (`mir/data.rs`) Changes**

The MIR loses information about variable mutability, which is essential for the type checker and potentially for the codegen/debugger.

**Action Items:**

**Track Mutability in `MirLocal`:** Add a boolean flag to `MirLocal` to track if a variable binding is mutable.  
Rust  
// In src/mir/data.rs  
pub struct MirLocal\<'src\> {  
    pub id: LocalId,  
    pub ty: hir::Ty\<'src\>,  
    pub is\_param: bool,  
    pub is\_mut: bool, // \<-- ADD THIS  
}

1. 

---

## **HIR \-\> MIR Lowering Changes**

The lowering process must be updated to handle generics correctly. This is the most critical fix required.

**Action Items:**

1. **Implement Monomorphization (In Type Checker):** This is a prerequisite for correct MIR. The MIR should only ever see concrete, non-generic functions.  
   * When the type checker encounters a call to a generic function (e.g., `identity(42)`), it must:  
     1. Determine the concrete types (e.g., `T` becomes `i64`).  
     2. Generate a mangled name for the concrete version (e.g., `identity$i64`).  
     3. Create a new, concrete `ast::Function` by duplicating the generic one and substituting all instances of `T` with `i64`.  
     4. Add this new function to the list of items to be fully checked and compiled.  
     5. The `hir::ExprKind::Call` should refer to the new mangled name.  
2. **Propagate Mutability:** In `mir/mod.rs`, when lowering a `hir::Stmt::Let`, read the `is_mut` flag and use it to set the new `is_mut` flag on the `MirLocal` you create in the `MirBuilder`.

---

## **Systematic MIR \-\> Wasm GC Codegen 🛠️**

Instead of hardcoding logic, use a systematic, stateful visitor pattern to walk the MIR and emit Wasm instructions. The guiding principle is that any function building an expression (`Rvalue` or `Operand`) **must** leave its result as a single value on the Wasm stack.

**Systematic Plan:**

**Create a `CodeGenContext`:** This struct will manage state during a function's compilation.  
Rust  
struct CodeGenContext\<'a, 'b\> {  
    wasm\_func: &'b mut wasm\_encoder::Function,  
    mir\_func: &'a mir::MirFunction\<'a\>,  
    local\_map: HashMap\<mir::LocalId, u32\>, // Map MIR LocalId to Wasm local index  
}

1. 

**Main Dispatcher (`build_instruction`):** Create a function that takes the context and a `MirInstruction` and calls the appropriate builder function.  
Rust  
fn build\_instruction(\&mut self, ctx: \&mut CodeGenContext, instr: \&mir::MirInstruction) {  
    match instr {  
        Assign(place, rvalue) \=\> self.build\_assign(ctx, place, rvalue),  
        If { cond, then, else } \=\> self.build\_if(ctx, cond, then, else),  
        Call { ... } \=\> self.build\_call(ctx, ...),  
        // ... etc. ...  
    }  
}

2.   
3. **`Rvalue` Builder (`build_rvalue`):** This is the core. Its job is to generate instructions that compute a value and leave it on the stack.  
   * **`Use(operand)`:** Calls `build_operand`.  
   * **`BinaryOp(op, lhs, rhs)`:** Calls `build_operand(lhs)`, then `build_operand(rhs)`, then emits the correct Wasm instruction (`I32Add`, `I64Sub`, etc.).  
   * **`StructInit { path, fields }`:**  
     1. Look up the struct's type index and field order from your `WasmBuilder`.  
     2. For each field in the defined order, call `build_operand` on its value. This pushes all field values onto the stack.  
     3. Emit `Instruction::StructNew(type_index)`.  
   * **`Projection { base, field }`:**  
     1. Emit `Instruction::LocalGet` for the `base` local.  
     2. Look up the struct's type index and the `field`'s index.  
     3. Emit `Instruction::StructGet { struct_type_index, field_index }`.  
4. **`Operand` Builder (`build_operand`):** Pushes a constant or local variable onto the stack.  
   * **`Constant(lit)`:** Emit `I32Const`, `I64Const`, `F64Const`. For strings, call a helper that emits instructions to create the Wasm GC array and struct (`ArrayNewFixed`, `StructNew`).  
   * **`Copy(place)`:** Emit `Instruction::LocalGet` for the `place`'s Wasm local index.  
5. **Instruction Builders (Examples):**  
   * **`build_assign(ctx, place, rvalue)`:**  
     1. Call `build_rvalue(ctx, rvalue)`. (A value is now on the stack).  
     2. Emit `Instruction::LocalSet` for the `place`'s Wasm local index.  
   * **`build_if(ctx, cond, then_block, else_block)`:**  
     1. Call `build_operand(ctx, cond)`. (A boolean `i32` is now on the stack).  
     2. Emit `Instruction::If(BlockType::Empty)`.  
     3. Recursively call `build_instruction` for every instruction in `then_block`.  
     4. Emit `Instruction::Else`.  
     5. Recursively call `build_instruction` for every instruction in `else_block`.  
     6. Emit `Instruction::End`.


