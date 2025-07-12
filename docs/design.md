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
  * **Enums:** Discriminated unions with pattern matching (`match`).  
* **Control Flow:**  
  * `if`/`else` expressions.  
  * `for` loops.  
* **Functions:** User-defined functions with parameters and return values.  
* **Error Handling:** A partially implemented `Result` struct with `Ok()`/`Err()` constructors.

---

### **Part 3: Multi-Phase Task List**

#### **Phase 1: Foundational Compiler (Current Phase)**

The goal of this phase is to build a "v1.0" feature-complete compiler.

* ✅ Lexer, Parser, Type Checker  
* ✅ LLVM Backend Setup & Build Orchestration  
* ✅ Core Constructs: Expressions, Variables, Functions  
* ✅ Control Flow: `if-else`, `for`  
* ✅ Data Structures: Strings, Arrays, Structs, Enums, `match`  
* 🚧 **Next Major Tasks:**  
  * **Implement Generics:** Add support for generic functions and types using monomorphization. This is a prerequisite for a clean standard library.  
  * **Implement Hash Maps:** The last major built-in data structure, essential for bootstrapping.  
  * **Integrate a Garbage Collector:** Replace `malloc` in the C runtime with a GC library (like Boehm GC) to eliminate memory leaks.  
* **Future Tasks for Phase 1:**  
  * Implement Floating-Point Numbers: Add `f64` as a primitive type.  
  * Implement the `?` Operator: Add the special error-propagation logic for the `Result` type.

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

---

### **Part 4: Advanced Design & Ecosystem Philosophy**

This section outlines the planned architecture for generics, concurrency, metaprogramming, and the developer ecosystem. These features are designed to provide a powerful, ergonomic, and scalable experience.

### **Generics: Compile-Time Specialization via Monomorphization**

To eliminate redundant code (e.g., `print_int`, `print_bool`) and enable powerful, type-safe abstractions like `Option<T>` or `List<T>`, the language will implement **generics through monomorphization**, similar to Rust or C++.

* **The Approach:** Monomorphization means the compiler generates a specialized, concrete version of a generic function or data structure for every unique set of types it's used with.  
  1. A generic function `fn print<T>(value: T)` used with `int64` and `bool` will cause the compiler to generate two separate, optimized functions internally, as if they were written `fn print_int64(value: int64)` and `fn print_bool(value: bool)`.  
  2. **Benefit:** This approach achieves maximum runtime performance with zero-cost abstractions, as there is no overhead from boxing, dynamic dispatch, or pointer indirection. The cost is a potentially larger binary size.

**Proposed Syntax:** The syntax will be familiar, using angle brackets for generic parameters.  
Go  
// Generic function  
fn identity\<T\>(arg: T) \-\> T {  
    arg  
}

// Generic struct  
let List\<T\> \= struct {  
    items: \[T\]  
};

// Usage  
let num \= identity\<int64\>(42);  
let str \= identity\<string\>("hello");

*   
* **Implementation Plan:**  
  1. **Parser:** Update the parser to recognize `<T>` syntax on `fn`, `struct`, and `enum` definitions, as well as on type instantiations like `List<int64>`.  
  2. **Type Checker:** This is the most significant change. The checker must be able to work with unbound generic types (`T`). When a generic function is called (`identity<int64>(...)`), the checker will substitute `T` with `int64` to create a concrete function signature and validate the call.  
  3. **Compiler:** The compiler will perform the monomorphization. When it encounters the first call to `identity<int64>`, it will generate the LLVM IR for that specific version. Subsequent calls will reuse the generated function. This requires a name-mangling scheme to create unique symbols (e.g., `identity_int64`).

**Constraints with Interfaces:** To make generics useful, we need to place constraints on them. For example, a generic `add` function should only work on types that support addition. This will be solved using the **`interface`** system.  
Go  
// Define an interface for types that can be added  
interface Add {  
    fn add(self, other: Self) \-\> Self;  
}

// A generic function constrained by the 'Add' interface  
fn sum\<T: Add\>(a: T, b: T) \-\> T {  
    a.add(b) // Use the method defined in the interface  
}

* 

---

### **Concurrency: Lightweight Green Threads**

To provide powerful and easy-to-use concurrency without the ergonomic issues of "colored functions" (`async`/`await`), the language will adopt a **Go-style M:N concurrency model.**

* **Green Threads:** Concurrency will be achieved via lightweight green threads, managed by a built-in language runtime scheduler.  
* **Cooperative & Preemptive Scheduling:** The runtime scheduler will map green threads onto a smaller pool of OS threads.

---

### **Metaprogramming: Compile-Time Code Generation**

To eliminate boilerplate, the language will implement a **Zig-style compile-time function execution (`comptime`) system.**

* **Purity Mandate:** `comptime` functions are **required to be pure**.  
* **Seamless Integration:** Metaprogramming will use the exact same syntax as regular code.

---

### **Ecosystem: The All-in-One Toolchain**

A successful language requires a thriving ecosystem supported by first-class tooling. The language will be distributed with a single, canonical command-line tool, **`basalt`**.

* **Integrated Tooling:** The `basalt` tool will handle `build`, `run`, `test`, `fmt`, and `lint` commands.  
* **Package Management:** The tool will manage dependencies using a **decentralized, Go-style model**, secured by a checksum database.

