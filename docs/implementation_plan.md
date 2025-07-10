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

## What Remains

### Parser
-   **AST Nodes:**
    -   String Literals
    -   Array Literals
    -   Hash Literals
    -   Index Expressions

-   **Parser Functions:**
    -   Parsing of String Literals
    -   Parsing of Array Literals
    -   Parsing of Hash Literals
    -   Parsing of Index Expressions

## Future Tasks

1.  **Implement String Literal Parsing:**
    -   Add `StringLiteral` AST node.
    -   Implement `parseStringLiteral` function.
    -   Register `parseStringLiteral` as a prefix parse function for `token.STRING`.
2.  **Implement Array Literal Parsing:**
    -   Add `ArrayLiteral` AST node.
    -   Implement `parseArrayLiteral` function.
    -   Register `parseArrayLiteral` as a prefix parse function for `token.LBRACKET`.
3.  **Implement Hash Literal Parsing:**
    -   Add `HashLiteral` AST node.
    -   Implement `parseHashLiteral` function.
    -   Register `parseHashLiteral` as a prefix parse function for `token.LBRACE`.
4.  **Implement Index Expression Parsing:**
    -   Add `IndexExpression` AST node.
    -   Implement `parseIndexExpression` function.
    -   Register `parseIndexExpression` as an infix parse function for `token.LBRACKET`.
5.  **Error Handling Improvements:**
    -   More robust error reporting and recovery.
6.  **Testing:**
    -   Write comprehensive unit tests for all new parser features.
    -   Update existing test files (`tests/`) to cover new language constructs.

## Test Updates
As new features are implemented, the existing test files in the `tests/` directory will be updated to include relevant test cases. New test files may be created as needed to ensure full coverage of the parser's capabilities.