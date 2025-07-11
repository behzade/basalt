### **Programming Language Design & Implementation**

#### **Part 1: Design Decisions**

**Core Philosophy**

* **Composable & Data-Oriented: Behavior is tied to data, not rigid class hierarchies.**  
* **Safe & Explicit: Aims for compile-time safety with explicit intent (e.g., mutability).**  
* **Fast & Simple: Prioritizes quick compilation/execution and a minimal, consistent rule set.**  
* **Batteries-Included: A comprehensive standard library is provided.**

---

**Memory Model**

* **Garbage Collected: The language is garbage-collected (GC), providing memory safety without manual management. Pointers are managed references.**  
* **Pass-by-Value: Structs and primitive types use pass-by-value by default for clarity. The compiler will use optimizations like copy-on-write (CoW) to mitigate performance costs for large types.**

---

**Syntax & Semantics**

* **C-Style Syntax: Uses curly braces `{}` for blocks.**  
* **Immutable by Default: Variables are declared with `let` and are immutable. Mutable variables must be explicitly declared with `let mut`.**  
* **Modern Ergonomics:**  
  * **Destructuring Assignments: `let { name, age } = user;`**  
  * **Concise Lambdas: `(x) => x + 1;`**

---

**Type System**

* **Static & Structural Typing: Interfaces are satisfied implicitly by matching method signatures.**  
  * **Refinement: An optional `implements` keyword can be used for explicit interface satisfaction to prevent accidental matches in large projects.**  
* **No Null: `Option<T>` and `Result<T, E>` are used for optionality and error handling.**  
  * **Refinement: An error propagation operator (`?`) is included to streamline `Result` handling.**  
* **Everything is an Expression: Control flow structures like `if` and `match` return values.**

---

**Ergonomics & Ecosystem**

* **Universal Function Call Syntax (UFCS): Allows free functions to be called with method-like syntax for better chainability.**  
* **Dependency Management: A modern package manager with a central repository is the primary mechanism. Vendoring is supported as an option for reproducible builds.**

---

**IDE Support & Tooling**

* **First-Class Tooling: The ecosystem is built around a single command-line tool for building, testing, formatting, and dependency management.**  
* **Editor Integration: Core support for the Language Server Protocol (LSP) and TreeSitter grammars is planned from the start.**

---

**Concurrency**

* **Lightweight Tasks: A built-in runtime manages lightweight concurrency tasks (similar to goroutines).**  
* **Async/Await: `async/await` provides the primary syntax for writing asynchronous code.**  
* **Channels: Channels are the preferred method for safe, message-based communication between tasks.**

---

#### **Part 2: Current Implementation Status**

**✅ Completed Features**

* **Lexical Analysis:** Complete tokenization of source code including keywords, operators, identifiers, integers, and booleans
* **Parsing:** Full Pratt parser implementation supporting:
  * Let statements (`let x = 5;`)
  * Return statements (`return x + 1;`)
  * Expression statements
  * Prefix expressions (`-x`, `!true`)
  * Infix expressions (`x + y`, `a == b`, etc.)
  * Function literals (`fn(x, y) { x + y }`)
  * Function calls (`myFunc(1, 2)`)
  * Grouped expressions with parentheses
* **Tree-Walking Interpreter:** Complete evaluation engine supporting:
  * Integer and boolean literal evaluation
  * Variable binding and lookup with scoped environments
  * All mathematical and comparison operators
  * Function object creation and storage
  * Function call evaluation with argument binding
  * Block statement evaluation
  * **Closures:** Functions correctly capture and remember their creation environment
  * Return statement handling with proper unwrapping

**🚧 In Progress / Next Steps**

* Built-in functions (print, len, etc.)
* Conditional expressions (if/else)
* Error handling and reporting
* String literals and operations
* Array/list data structures

---

#### **Part 3: Multi-Phase Task List**

**Phase 1: Compiler in Go**

**Build a minimal proof-of-concept compiler in Go to bootstrap the language.**

* **Tasks:**  
  * **✅ Lexer/parser for core syntax (tokens, statements, expressions, functions)**  
  * **✅ Tree-walking interpreter with basic evaluation**  
  * **✅ Function literals and first-class functions with closures**  
  * **✅ Environment-based variable scoping**  
  * **Type system implementation with structs, interfaces, and optional `implements` checks.**  
  * **Implement expression-based control flow (`if`/`match`).**  
  * **Implement the Garbage Collector (GC) for managed pointers.**  
  * **Implement the `?` operator for error handling.**  
  * **Build a minimal standard library with `Option<T>` and `Result<T, E>`.**  
  * **Create the initial TreeSitter grammar and basic LSP support.**

---

**Phase 2: Self-Hosted Compiler**

**Rewrite the compiler in the new language itself to dogfood and prove its capabilities.**

* **Tasks:**  
  * **Port the lexer, parser, and type checker from the Go version.**  
  * **Implement the backend/code generation for the target architecture.**  
  * **Build the `async/await` runtime and implement channel functionality.**  
  * **Expand the standard library (e.g., HTTP, JSON, filesystem APIs).**  
  * **Enhance LSP features (e.g., autocompletion, go-to-definition).**

---

**Phase 3: Ecosystem Development**

**Polish the language, grow the standard library, and build the community ecosystem.**

* **Tasks:**  
  * **Refine compiler error messages to be exceptionally helpful.**  
  * **Write comprehensive documentation, tutorials, and examples.**  
  * **Build and launch the package manager and its central repository.**  
  * **Create an FFI for seamless interoperability with C libraries.**  
* 

