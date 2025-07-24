# Basalt Programming Language

![Status](https://img.shields.io/badge/status-work_in_progress-orange)

Basalt is an experimental, statically-typed programming language designed for creating portable and performant applications. It is compiled ahead-of-time (AOT) and is primarily targeted for the WebAssembly (WASM) and WASI environments.

Its core features include:
* A functional-leaning, data-oriented design using structs and traits (no classes or inheritance).
* Powerful static typing with type inference.
* **Algebraic effects** for handling side effects in a structured and testable way.
* Automatic memory management via a planned tracing garbage collector.

## 🚧 Project Status: Work in Progress

This project is currently in the early stages of development. It is is not yet ready for production use.

**What works:**
* **Frontend:** A complete parser, type checker, and intermediate representation pipeline (AST → HIR → MIR).
* **Native Compilation:** A working backend using **Cranelift** that compiles Basalt code directly to a native executable on your machine.

**What's next (Roadmap):**
* **Garbage Collector:** Implementing a tracing GC for automatic memory management.
* **Algebraic Effects Runtime:** Building the runtime system to fully support `perform` and `handle`.
* **WebAssembly Backend:** Creating an LLVM-based backend to compile code to `.wasm` files.
* **Standard Library:** Fleshing out the modules in `modules/std/`.
* **Self Hosted Compiler:** Rewrite the compiler in bst once the language is complete enough.

## 🚀 Getting Started

Currently, you can compile and run `.bst` files as native executables.

1.  **Clone the repository:**
    ```sh
    git clone https://github.com/behzade/basalt
    cd basalt
    ```

2. Process a Test File
You can process test files using various subcommands with the following structure:


```sh
cargo run -- <parse/hir/mir/cir/build/run> ./tests/<test-file-name>.bst
```
For example, the parse subcommand reads a .bst file and prints its corresponding Abstract Syntax Tree (AST) to standard output. This is useful for verifying the initial parsing stage of the compiler.

## ⚖️ License

This project is licensed under the [Apache 2.0 License](LICENSE).

## 💡 Development Notes

This compiler has been developed with significant AI assistance. AI was used as a productivity multiplier for generating boilerplate code, exploring library APIs (like `chumsky` and `cranelift`), and creating test cases.

While this approach has accelerated development, it can lead to inconsistencies in code quality. A test-driven development (TDD) workflow is being used to mitigate these issues. A partial or full rewrite is planned once the language and standard library reach a more mature state.
