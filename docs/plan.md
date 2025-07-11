Project Implementation Plan: basalt Bootstrap Compiler (Test-Driven)This document outlines the plan for building the basalt programming language. It has been updated to reflect the current project status and incorporates the strategic decision to implement function parsing before moving to the evaluator.A Note on TDD & ComplexityThis project follows a Test-Driven Development (TDD) workflow. We will write tests before implementing each new feature of the language.To manage complexity, our initial goal is to build a tree-walking interpreter. This allows us to execute code directly from the Abstract Syntax Tree (AST), proving our language design works without the added complexity of compiling to machine code or another intermediate representation.Phase 1: Bootstrap Compiler in Go (The "Seed")Objective: Create a minimal, functional interpreter in Go that can execute the core features of basalt.Completed Steps:Step 1.1: Project Setup & Token DefinitionInitialized the Go project and defined all the tokens for the language (keywords, operators, identifiers, etc.).Step 1.2: The Lexer (Test-First)Implemented a lexer to turn source code strings into a sequence of tokens.Step 1.3: The AST & Parser (Let Statements)Defined the basic AST structure and implemented parsing for let statements.Step 1.4: The AST & Parser (Return Statements)Extended the AST and parser to support return statements.Step 1.5: Expression Parsing (Identifiers & Integer Literals)Introduced the Pratt parsing technique to handle simple expressions.Step 1.6: Parsing Prefix ExpressionsAdded support for prefix operators like ! and -.Step 1.7: Parsing Infix ExpressionsExtended the parser to handle infix operators like +, -, *, /, and comparison operators.Step 1.8: Operator Precedence, Booleans, and Grouped ExpressionsCompleted the Pratt parser implementation by adding logic for operator precedence, boolean literals (true/false), and grouped expressions with parentheses.Current Step & Next Actions:
Step 1.9: Parsing Function Literals and Call Expressions (Completed)

Phase 2: Evaluation (The "Heart")
Objective: Implement the evaluator to execute the parsed AST, starting with basic expressions and statements, then moving to functions and control flow.

Step 2.1: Basic Expression Evaluation (Completed)
Step 2.2: Let and Return Statement Evaluation (Completed)
Step 2.3: Function Evaluation and Application (Completed)
- Created Function object type with parameters, body, and environment
- Implemented function literal evaluation (fn(x) { ... })
- Implemented function call evaluation with proper argument binding
- Added block statement evaluation for function bodies
- Implemented closure support with environment chaining
Step 2.4: Built-in Functions
Step 2.5: If/Else Expressions
Step 2.6: Error Handling

Phase 3: Standard Library & Advanced Features (The "Brain")
Objective: Build out the standard library and implement more advanced language features.

Step 3.1: String Literals and String Concatenation
Step 3.2: Array Literals and Index Expressions
Step 3.3: Hash Maps
Step 3.4: Macros
Step 3.5: First-Class Environments

Phase 4: Compiler (The "Muscle")
Objective: Implement a compiler to translate Basalt code into bytecode for a custom virtual machine.

Step 4.1: Bytecode Definition
Step 4.2: Compiler Implementation
Step 4.3: Virtual Machine Implementation

Phase 5: REPL & Tooling (The "Interface")
Objective: Create a Read-Eval-Print Loop (REPL) and other developer tools.

Step 5.1: REPL Implementation
Step 5.2: Debugger Integration
Step 5.3: Language Server Protocol (LSP) SupportAs you correctly pointed out, a robust evaluator needs functions and control flow. This is our current focus.1. Parse if-else Expressions: Implement the parsing logic for if (<condition>) { <consequence> } and if (...) { ... } else { ... }.2. Parse Function Literals: Implement parsing for function definitions, e.g., fn(x, y) { return x + y; }.3. Parse Call Expressions: Implement parsing for function calls, e.g., myFunction(2, 3).Step 1.10: The Evaluator (Tree-Walking Interpreter)With a feature-rich parser in place, we will build the evaluator to execute the AST, bringing the language to life.Phase 2: Self-Hosted Compiler (Roadmap)[ ] Port Lexer & Parser from Go to basalt.[ ] Port Type Checker (to be built) to basalt.[ ] Design Code Generation Backend (e.g., LLVM IR or a custom bytecode).[ ] Implement Concurrency features.[ ] Expand the Standard Library.Phase 3: Ecosystem & Tooling (Roadmap)[ ] Refine Compiler & Tooling.[ ] Language Server Protocol (LSP) for editor support.[ ] Package Manager.[ ] Comprehensive Documentation & Community Building.[ ] Foreign Function Interface (FFI).
