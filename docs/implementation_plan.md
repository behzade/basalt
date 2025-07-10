# Zerolang Compiler Implementation Plan

## Overview
This document outlines the current status and future steps for the Zerolang compiler, focusing on the Go-based implementation of the lexer and parser.

## Current State
We have a functional lexer that can tokenize Zerolang source code. The parser is under active development and can currently parse basic expressions and statements.

## What's Done

### Lexer
-   Token definition (`compiler/token/token.go`)
-   Lexer implementation (`compiler/lexer/lexer.go`)

### Parser
-   **AST Nodes (`compiler/ast/ast.go`):**
    -   `Program`
    -   `LetStatement`
    -   `Identifier`
    -   `ExpressionStatement`
    -   `IntegerLiteral`
    -   `PrefixExpression`
    -   `InfixExpression`
    -   `Boolean`
    -   `IfExpression`
    -   `BlockStatement`
    -   `ReturnStatement`
    -   `FunctionLiteral`
    -   `CallExpression`

-   **Parser Functions (`compiler/parser/parser.go`):**
    -   `New` (parser constructor)
    -   `Errors`
    -   `nextToken`
    -   `ParseProgram`
    -   `parseStatement` (dispatching)
    -   `parseLetStatement`
    -   `parseExpressionStatement`
    -   `parseExpression` (with precedence handling)
    -   `parseIntegerLiteral`
    -   `parseIdentifier`
    -   `parsePrefixExpression`
    -   `parseInfixExpression`
    -   `parseBoolean`
    -   `parseGroupedExpression`
    -   `parseIfExpression`
    -   `parseBlockStatement`
    -   `parseReturnStatement`
    -   `parseFunctionLiteral`
    -   `parseFunctionParameters`
    -   `parseCallExpression`
    -   `parseCallArguments`
    -   Precedence table and helper functions (`peekPrecedence`, `curPrecedence`)
    -   Prefix and Infix parse function registration

### Parser
-   **AST Nodes (`compiler/ast/ast.go`):**
    -   `Program`
    -   `LetStatement`
    -   `Identifier`
    -   `ExpressionStatement`
    -   `IntegerLiteral`
    -   `PrefixExpression`
    -   `InfixExpression`
    -   `Boolean`
    -   `IfExpression`
    -   `BlockStatement`
    -   `ReturnStatement`
    -   `FunctionLiteral`
    -   `CallExpression`
    -   `StringLiteral`
    -   `ArrayLiteral`
    -   `HashLiteral`
    -   `IndexExpression`
    -   `ErrorPropagation`

-   **Parser Functions (`compiler/parser/parser.go`):**
    -   `New` (parser constructor)
    -   `Errors`
    -   `nextToken`
    -   `ParseProgram`
    -   `parseStatement` (dispatching)
    -   `parseLetStatement`
    -   `parseExpressionStatement`
    -   `parseExpression` (with precedence handling)
    -   `parseIntegerLiteral`
    -   `parseIdentifier`
    -   `parsePrefixExpression`
    -   `parseInfixExpression`
    -   `parseBoolean`
    -   `parseGroupedExpression`
    -   `parseIfExpression`
    -   `parseBlockStatement`
    -   `parseReturnStatement`
    -   `parseFunctionLiteral`
    -   `parseFunctionParameters`
    -   `parseCallExpression`
    -   `parseCallArguments`
    -   `parseStringLiteral`
    -   `parseArrayLiteral`
    -   `parseHashLiteral`
    -   `parseIndexExpression`
    -   `parseErrorPropagation`
    -   Precedence table and helper functions (`peekPrecedence`, `curPrecedence`)
    -   Prefix and Infix parse function registration

## What Remains

### Parser
-   Destructuring Assignments
-   Type Declarations (Structural Typing)

## Future Tasks

### LLVM Backend
-   **Phase 1: LLVM Setup and Basic IR Generation**
    -   **Research Go LLVM Bindings:** Completed (`tinygo.org/x/go-llvm` selected).
    -   **Environment Setup:** Ongoing (Devbox integration, CGO flag configuration, `run_compiler.sh` script created).
    -   **Basic LLVM Context and Module:** Completed (Initial `codegen` package).
    -   **Integer Literal IR Generation:** Completed.
    -   **Basic Arithmetic IR Generation:** Completed.
-   **Return Statement IR Generation:** Completed.

-   **Phase 2: Expanding IR Generation and Execution**
    -   Variable Declaration and Assignment.
    -   Function Definition and Calls.
    -   **Control Flow (If Expressions):** Ongoing (Debugging termination issues).
    -   String and Array Literals.
    -   **Execution Engine Integration:** Ongoing (Debugging segmentation faults).

-   **Phase 3: Advanced Features and Testing**
    -   Hash Literals and Index Expressions.
    -   Error Propagation (`?` operator).
    -   End-to-End Testing with LLVM.
    -   `print` Functionality Test.
    -   Refinement and Optimization.

1.  **Testing:**
    -   Write comprehensive unit tests for all new parser features.
    -   Update existing test files (`tests/`) to cover new language constructs.

## Test Updates
As new features are implemented, the existing test files in the `tests/` directory will be updated to include relevant test cases. New test files may be created as needed to ensure full coverage of the parser's capabilities.