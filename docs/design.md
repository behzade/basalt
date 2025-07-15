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


### **Part 5: Design Extension \- Lexical Algebraic Effects**

This document outlines the design and implementation strategy for integrating first-class, lexically-scoped algebraic effects into Basalt. This pivot is motivated by the goal of creating an exceptionally safe and expressive language, ideal for AI-assisted development where provable correctness is paramount.

#### **1\. Core Philosophy & Syntax**

We are shifting from ad-hoc solutions for side effects (like Result types or async/await) to a single, unified abstraction. Effects are defined as interfaces, performed by functions, and implemented by handlers.

**New Keywords:**

| Keyword | Purpose |
| :---- | :---- |
| effect | Defines a new effect interface, specifying its operations. |
| handler | Provides an implementation for the operations of one or more effects. |
| perform | Invokes an effect operation, transferring control to the nearest handler. |
| resume | A capability passed to a handler to resume the original computation. |

**Example Syntax: A** State **Effect**

// 1\. Define the effect signature  
effect State\<T\> {  
    get() \-\> T,  
    put(value: T) \-\> none,  
}

// 2\. A function that performs the effect.  
// The signature explicitly declares that it requires the State\<int64\> effect.  
let counter \= fn() \-\> none / {State\<int64\>} {  
    let current \= perform State::get();  
    Fmt.print\_int(current);  
    perform State::put(current \+ 1);  
};

// 3\. Provide a handler to run the effectful code.  
let main \= fn() \-\> none {  
    // The handler provides the implementation for 'get' and 'put'.  
    handler {  
        perform State::put(0); // Initialize state  
        counter();  
        counter();  
        let final \= perform State::get(); // final will be 2  
        Fmt.print\_string("Final value: " \+ final);  
    } with State\<int64\> (let mut state \= \-1) { // Handler has its own state  
        get() \=\> {  
            resume(state) // Resume the computation with the current state value  
        },  
        put(new\_value) \=\> {  
            state \= new\_value;  
            resume(()) // Resume the computation with no value  
        },  
    }  
};

#### **2\. Type System Integration**

The type checker is the key to making effects safe. It will be enhanced to track an "effect row" for every expression.

* **Effect-Polymorphic Signatures:** A function's type will now include the set of effects it may perform. The syntax is fn(Args) \-\> ReturnType / {Effect1, Effect2}. A function with no effects is pure: fn() \-\> int64 / {}.  
* **Ambient Effect Tracking:** The checker will maintain the set of "handled" effects in the current scope.  
  1. Inside a handler { ... } with State\<T\>, the State\<T\> effect is considered handled and available to be performed.  
  2. A function's signature (e.g., fn() / {Log}) acts as a promise that its body will only perform effects from that set.  
* **Validation Rules:**  
  1. A perform E::op() expression is only valid if the effect E is present in the current function's effect signature.  
  2. A handler with E block removes E from the effect signature of the resulting expression. The code *inside* the handler can perform E, but the code *after* it cannot (unless a parent handler also provides E).

#### **3\. Compiler & Runtime Implementation Strategy**

The magic behind effects is **delimited continuations**. Since we have a GC, we can safely allocate continuations on the heap.

**The Execution Flow of** perform**:**

1. **Capture:** When perform State::get() is executed, the compiler generates code to capture the current state of execution (the call stack, registers) up to the boundary of the handler with State. This captured state is the **continuation**.  
2. **Heap Allocation:** The continuation is packaged into a struct on the heap. This struct contains a function pointer to the resumption point and any necessary context (captured local variables).

// Simplified C representation of a continuation object  
struct Continuation {  
    void (\*resume\_point)(void\* env, void\* result);  
    void\* environment; // Pointer to captured variables  
};

3.   
4. **Stack Unwind:** The runtime unwinds the call stack, discarding frames until it reaches the handler's stack frame.  
5. **Handler Invocation:** Control jumps to the appropriate operation clause in the handler (the get() clause). The Continuation object is passed as an implicit argument, which becomes the resume capability.  
6. **Resumption:** When the handler calls resume(value), it's actually calling the function pointer inside the Continuation object. The runtime restores the captured stack, places value where the perform call was, and execution continues from there.

Role of the C Runtime (Bootstrap Phase):

Initially, you will need a small C library with low-level functions to:

* capture\_stack(...): Copy the relevant portion of the stack to a heap buffer.  
* restore\_stack(...): Restore a captured stack and jump to its execution point.

Your Go compiler will generate LLVM IR that calls these C functions. In the self-hosted phase, Basalt will become powerful enough to generate this low-level code itself.

#### **4\. Phased Implementation Plan**

This is a large feature, best tackled in stages.

* **Phase 1: Syntax & AST (1-2 days)**  
  * Add the effect, handler, perform, and resume keywords to the lexer.  
  * Create new AST nodes: EffectStatement, HandlerExpression, PerformExpression.  
  * Update the parser to recognize and build these new AST nodes.  
* **Phase 2: Type Checker (1-2 weeks)**  
  * Modify FunctionType to include an EffectRow (a set of effect types).  
  * Update the TypeEnvironment to track the ambient effects available in the current scope.  
  * Implement the validation rules for perform and handler expressions. This is the most complex static analysis part.  
* **Phase 3: Runtime & Compiler (2-4 weeks)**  
  * **C Runtime:** Implement the basic stack manipulation functions (capture\_stack, restore\_stack). Start with **one-shot continuations** (which can only be resumed once) to simplify the initial logic.  
  * **Compiler:**  
    * When compiling a handler, generate the handler table and its logic.  
    * When compiling perform, generate the calls to the C runtime to capture the continuation and jump to the handler.  
    * When compiling resume, generate the call to restore the continuation.  
* **Phase 4: Standard Library**  
  * Once the core feature is working, re-implement Result as a Fail effect.  
  * Add State, Reader, and other common effects to the standard library to showcase the power of the system.

