# AGENTS.md

This file provides information for agentic coding agents operating in this repository.

## Build/Lint/Test Commands

*   `go run ./compiler <file.zl>`: Runs the compiler on the given file.
*   `go test ./...`: Runs all tests.
*   `go test -run <test_name> ./...`: Runs a single test.

## Code Style Guidelines

*   C-style syntax with curly braces `{}` for blocks.
*   Immutable by default: use `let` for immutable variables and `let mut` for mutable variables.
*   Static & Structural Typing.
*   No Null: use `Option<T>` and `Result<T, E>` for optionality and error handling.
*   Error propagation operator (`?`) for `Result` handling.
*   Everything is an Expression.

## Error Handling

*   Use `Result<T, E>` for error handling.
*   Use the error propagation operator `?` to streamline `Result` handling.
