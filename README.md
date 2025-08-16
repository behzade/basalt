## Basalt

An exploration of programming language and compiler design. This repo is a lab for trying different language designs and seeing how those choices ripple through the frontend (lexer/parser), the type system and inference, lowering, and backend code generation.

Basalt is a statically typed, functional leaning language with a focus on simplicity and ergonomics.
the language.

- **Not a product. Do not use this to build software.** Things change frequently, features are incomplete, and stability is not a goal.

### Iterations

1. **Go prototype**: first pass on syntax/type ideas to validate design quickly.
2. **Rust + Cranelift**: IR-based backend experiments and codegen trade‑offs.
3. **Rust → direct WebAssembly**: skipping an IR to emit WASM directly.
4. **Rust → Interpreter (current)**: exploring a more direct approach to execution. Since the language will have metaprogramming capabilities, we need an interpreter which will run the "meta" blocks of code in ``resolve`` step of the pipeline. We will use the same interpreter to execute the code before we have a proper llvm backend.
5. **Self-Hosted -> LLVM (future)**: a self-hosted compiler that emits LLVM IR. The interpreter written in rust will be the bootstrap compiler.

### What you will find here

- **Lexer and tokens**: `src/token.rs`
- **Parser**: `src/parser.rs`
- **Type system and checking**:
  - Core checker: `src/typechecker/checker.rs`
  - Lowering to typed IR: `src/typechecker/lowering/expr.rs`, `src/typechecker/lowering/stmt.rs`
  - Type/definition registry: `src/typechecker/registry.rs`
- **Design notes and experiments**: `design.bst`
- **Small, focused test inputs**: `tests/*.bst` (e.g., `tests/00-imports-and-aliases.bst`, `tests/02-interfaces-and-impls.bst`)

The code aims to surface trade‑offs rather than hide them behind abstractions.

### Peeking at the pipeline

Run selective stages to inspect artifacts:

```sh
cargo run -- <parse|resolve|hir> ./tests/<file>.bst
```

- `parse`: print the AST
- `resolve`: name resolution and scopes
- `hir`: typed high‑level IR

Other subcommands (like `mir` or `build`) may be stubs during this iteration.

### Scope and non-goals

- **No stability guarantees** and **no standard library**
- No promises about error messages or ergonomics
- Backwards compatibility is not considered
- Contributions are not solicited; issues/PRs may be ignored

### License

Apache-2.0. See `LICENSE`.
