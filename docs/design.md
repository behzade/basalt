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


### **Part 4: Advanced Design & Ecosystem Philosophy**

This section outlines the planned architecture for concurrency, metaprogramming, and the developer ecosystem. These features are designed to provide a powerful, ergonomic, and scalable experience, building upon the core principles of safety and explicitness.

---

### **\#\# Concurrency: Lightweight Green Threads**

To provide powerful and easy-to-use concurrency without the ergonomic issues of "colored functions" (`async`/`await`), the language will adopt a **Go-style M:N concurrency model.**

* **Green Threads:** Concurrency will be achieved via lightweight green threads, managed by a built-in language runtime scheduler. This allows for spawning hundreds of thousands or even millions of concurrent tasks efficiently, making it ideal for I/O-bound applications like web servers and network clients.  
* **Cooperative & Preemptive Scheduling:** The runtime scheduler will map green threads onto a smaller pool of OS threads. To ensure fairness and prevent greedy threads from monopolizing resources, the compiler will inject cooperative preemption points into the code, ensuring that tight loops or long-running computations yield to the scheduler.  
* **GC Integration:** The garbage collector will be fully integrated with the scheduler, capable of safely pausing and resuming all green threads to perform memory collection.

This model prioritizes developer ergonomics, allowing any function to be run concurrently without altering its signature, while providing the performance necessary for modern, scalable software.

---

### **\#\# Metaprogramming: Compile-Time Code Generation**

To eliminate boilerplate and provide powerful code generation capabilities without resorting to external tools, the language will implement a **Zig-style compile-time function execution (`comptime`) system.**

* **Purity Mandate:** `comptime` functions are **required to be pure**. Their output must depend solely on their inputs, with no observable side effects such as I/O. This ensures that builds are completely deterministic and reproducible while still allowing for powerful type inspection and code generation.  
* **Seamless Integration:** Metaprogramming will use the exact same syntax as regular code. There is no separate macro language to learn. This provides a low barrier to entry for writing custom code generators, such as deriving a `Serializable` implementation for a `struct`.  
* **The Builder Pattern:** This system is the ideal tool for solving the "Big Struct" initialization problem. A `comptime` function will be able to generate a fluent **Builder** for any given struct, providing an ergonomic and safe way to construct complex objects.

---

### **\#\# Initialization and Data Handling**

The language enforces strict and safe patterns for data initialization and updates.

* **No Implicit Defaults:** To eliminate an entire class of runtime bugs, variables are not permitted to be read before they are explicitly initialized. Types do not have "zero values." This guarantees that an object can never exist in a logically invalid state.  
* **The `Default` Interface:** To solve the ergonomic challenge of creating complex objects, a built-in `Default` interface will be provided. A developer can implement the `default()` method for their `struct` to define a standard, baseline instance. This is an explicit, opt-in mechanism for default construction.

**Struct Update Syntax:** To support ergonomic use of immutable data, the language will adopt Rust's **struct update syntax**. This allows a developer to create a new instance of a struct based on an old one while changing only the necessary fields, avoiding verbose and error-prone manual copying.  
Go  
let config1 \= AppConfig::default();

// Create a new config with a different port, copying all other fields from config1.  
let config2 \= AppConfig {  
    port: 9090,  
    ..config1  
};

* 

---

### **\#\# Ecosystem: The All-in-One Toolchain**

A successful language requires a thriving ecosystem supported by first-class tooling. The language will be distributed with a single, canonical command-line tool, **`basalt`**, that serves as the entry point for the entire developer experience.

* **Integrated Tooling:** The `basalt` tool will handle all common development tasks, including:  
  * `basalt build`: Compiling projects into a final binary.  
  * `basalt run`: Compiling and running code directly.  
  * `basalt test`: Running the test suite for a project.  
  * `basalt fmt`: Applying canonical, non-negotiable code formatting.  
  * `basalt lint`: Checking for common style and logic errors.  
* **Package Management:** The tool will manage dependencies using a **decentralized, Go-style model**. Packages will be fetched from their source repositories (e.g., GitHub), with versions managed through a manifest file. To ensure build security and reproducibility, this system will be supported by a **checksum database or proxy**, preventing issues like deleted repositories or rewritten git histories.
