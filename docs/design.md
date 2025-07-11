### **Basalt: Programming Language Design & Implementation**

#### **Part 1: Core Design**

**Core Philosophy**

* **Composable & Data-Oriented:** Behavior is tied to data. The language provides powerful data structures like structs, arrays, and hashmaps as first-class citizens.  
* **Safe & Explicit:** Aims for compile-time safety and clarity. Intent must be explicit (e.g., `let mut` for mutability).  
* **Fast & Simple:** Prioritizes quick compilation and a minimal, consistent rule set.  
* **Batteries-Included:** A comprehensive standard library is provided.

---

**Memory Model**

* **Garbage Collected (Goal):** The language will be garbage-collected (GC) for automatic memory management.  
* **Current State:** The compiler uses stack allocation for primitives and structs. Heap-allocated types (strings, arrays) are managed via a C runtime and currently leak memory, pending GC integration.  
* **Pass-by-Value:** Structs and primitive types use pass-by-value semantics.

---

**Syntax & Semantics**

* **C-Style Syntax:** Uses curly braces `{}` for blocks.  
* **Immutable by Default:** Variables declared with `let` are immutable. Mutability is opt-in via `let mut`.  
* **Everything is an Expression:** Control flow structures like `if` and `match` return values.

---

**Type System**

* **Static Typing:** The language is statically typed. A type-checking pass runs before compilation, preventing type errors.  
* **No Null:** `Option<T>` and `Result<T, E>` (to be built on the `enum` system) are used for optionality and error handling.  
* **Error Propagation:** An error propagation operator (`?`) will be implemented to streamline `Result` handling.

---

**Future Goals (Design)**

* **Ergonomics:** Universal Function Call Syntax (UFCS), destructuring assignments.  
* **Ecosystem:** A first-class package manager and build tool.  
* **Tooling:** Language Server Protocol (LSP) for editor integration.  
* **Concurrency:** A lightweight, task-based concurrency model with `async/await`.

---

### **Part 2: Current Implementation Status**

The project has rapidly evolved from an interpreter to a powerful ahead-of-time (AOT) compiler.

**✅ Completed Features**

* **Lexer & Parser:** Full Pratt parser for the language syntax.  
* **Static Type Checker:** A dedicated pass that verifies type correctness before compilation.  
* **LLVM Compiler Backend:** The compiler generates LLVM IR from the AST.  
* **C Runtime Interop:** A small C library provides runtime support for advanced features.  
* **Build Orchestration:** The main program automatically orchestrates the full `Basalt -> LLVM IR -> Object File -> Executable` pipeline.

**✅ Implemented Language Constructs (in Compiler)**

* **Primitive Types:** `i64` integers and booleans.  
* **Data Structures:**  
  * **Strings:** Heap-allocated via the C runtime, with support for concatenation and comparison.  
  * **Arrays:** Dynamically-sized integer arrays managed by the C runtime, with support for creation, indexing, and `.len()`.  
  * **Structs:** User-defined, stack-allocated aggregate types with field access.  
* **Control Flow:**  
  * `if`/`else` expressions.  
  * `for` loops.  
* **Functions:** User-defined functions with parameters and return values.  
* **Error Handling:** A partially implemented `Result` struct with `Ok()`/`Err()` constructors.

---

### **Part 3: Multi-Phase Task List**

#### **Phase 1: Foundational Compiler (Current Phase)**

The goal of this phase is to build a "v1.0" feature-complete compiler.

* **✅ Lexer, Parser, Type Checker**  
* **✅ LLVM Backend Setup & Build Orchestration**  
* **✅ Core Constructs:** Expressions, Variables, Functions  
* **✅ Control Flow:** `if-else`, `for`  
* **✅ Data Structures:** Strings, Arrays, Structs  
* **🚧 Next Major Task:**  
  * **Implement General-Purpose Enums and Pattern Matching (`match`):** This is the next architectural step.  
    * **Part 1 (Enums):** Implement the parsing, type-checking, and memory layout for user-defined discriminated unions (e.g., `enum MyType { VariantA(i64), VariantB }`).  
    * **Part 2 (Pattern Matching):** Implement a `match` expression to safely destructure enum variants. This will involve compiling to a `switch` on the enum's tag and generating branches for each pattern.  
* **Future Tasks for Phase 1:**  
  * **Implement Floating-Point Numbers:** Add `f64` as a primitive type.  
  * **Implement the `?` Operator:** Add the special error-propagation logic for the `Result` type.  
  * **Implement Hash Maps:** The last major built-in data structure.  
  * **Integrate a Garbage Collector:** Replace `malloc` in the C runtime with a GC library (like Boehm GC) to eliminate memory leaks.

#### **Phase 2: Self-Hosted Compiler**

The goal is to prove the language's capability by rewriting the compiler in Basalt itself.

* **Tasks:**  
  * Port the lexer, parser, type checker, and LLVM code generator from Go to Basalt.  
  * Achieve the bootstrap milestone: the Basalt compiler can compile itself.

#### **Phase 3: Ecosystem & Growth**

With a self-hosted compiler, focus shifts to tooling and community.

* **Tasks:**  
  * Develop a package manager and central repository.  
  * Build a Language Server Protocol (LSP) for rich editor support.  
  * Create a Foreign Function Interface (FFI) for C interoperability.  
  * Expand the standard library.

