# Basalt

Basalt is an experimental programming language/compiler playground in Rust.
The focus is on quickly iterating on syntax, typechecking, lowering, and runtime behavior.

- Not a product. No stability guarantees.
- Backward compatibility is not a goal.

## Current Status

The active pipeline in `main` is:

1. Lex + parse to AST
2. Resolve imports/modules
3. Typecheck + lower to typed HIR
4. Execute HIR with the interpreter (`run` command)

`mir` and `build` stages currently exist in the CLI but are not implemented.

## CLI

```sh
# Print parsed owned AST as JSON
cargo run -- ast ./tests/00-imports-and-aliases.bst

# Print typed HIR as JSON
cargo run -- hir ./tests/05-match-and-control-flow.bst

# Typecheck + run via interpreter (exit code comes from program result)
cargo run -- run ./tests/00-imports-and-aliases.bst

# Present in CLI, currently stubs
cargo run -- mir ./tests/00-imports-and-aliases.bst
cargo run -- build ./tests/00-imports-and-aliases.bst
```

## What Is Implemented

- Frontend:
  - Lexer/tokens: `src/lexer.rs`, `src/token.rs`
  - Parser: `src/parser.rs`
- Middle-end:
  - Typechecker and lowering: `src/typechecker/**`
  - Typed IR: `src/hir.rs`
- Runtime:
  - Tree-walking interpreter for HIR: `src/interpreter/**`
- Editor tooling:
  - LSP server: `src/bin/lsp_server/**`

## Language Coverage (Current)

Works through parser/typechecker:

- Imports and aliases
- Structs and field access
- Type aliases including tagged unions
- Functions and function literals
- `if` / `while` / `match`
- Effects, handlers, and `perform` typing rules
- UFCS-style method calls lowered to regular calls

Not currently implemented in interpreter runtime:

- `match` execution
- `perform`/`handle` execution
- Map runtime values

Note: `=` is the assignment operator (older `<-` syntax has been removed).

## Modules and Imports

- `import { self::... }` resolves from `./src`
- Other imports resolve from `./modules`
- Standard-library-like modules in this repo are minimal placeholders under `modules/std/**`

## Tests

Snapshot and stage test runner:

```sh
./tests/run.sh ast
./tests/run.sh hir --compare
./tests/run.sh ast --snapshot
```

Test inputs are in `tests/*.bst`, with snapshots in `tests/snapshots/`.

## Dev Environment

This repo includes a `devbox.json` with scripts for `build`, `test`, `fmt`, and `clippy`.

```sh
devbox run -- cargo build
devbox run -- cargo test
```

## License

Apache-2.0. See `LICENSE`.
