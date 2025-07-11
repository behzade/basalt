# LLVM Backend Tests

This directory contains test files for validating the LLVM compiler backend implementation.

## Test Files

### 01_basic_expressions.bst
Tests basic language features:
- Variable declarations and assignments
- Arithmetic expressions (+, -, *, /)
- Boolean expressions and comparisons
- Print statements

**Expected Output:**
```
52
420
true
```

### 02_control_flow.bst
Tests control flow features:
- If-else expressions
- Conditional branching
- PHI nodes for value merging

**Expected Output:**
```
10
5
```

### 03_functions.bst
Tests user-defined functions:
- Function definitions with type annotations
- Function calls with parameters
- Recursive functions
- Return values

**Expected Output:**
```
8
120
```

## Running Tests

To run all tests and verify the LLVM backend is working correctly:

```bash
# Test basic expressions
go run main.go llvm_tests/01_basic_expressions.bst --compile test1 && ./test1

# Test control flow
go run main.go llvm_tests/02_control_flow.bst --compile test2 && ./test2

# Test functions
go run main.go llvm_tests/03_functions.bst --compile test3 && ./test3

# Clean up
rm test1 test2 test3
```

## Features Validated

These tests validate the following LLVM backend features:

1. **Code Generation**: AST to LLVM IR translation
2. **Type System**: Proper LLVM type mapping (i64, i1, double)
3. **Memory Management**: Stack allocation and load/store operations
4. **Control Flow**: Basic blocks, conditional branches, and PHI nodes
5. **Function Compilation**: Function definitions, calls, and recursion
6. **External Functions**: C runtime integration for I/O
7. **Build Pipeline**: IR generation → llc → clang → executable

## Implementation Status

✅ **Completed Features:**
- Variables and expressions
- Arithmetic and boolean operations
- Control flow (if-else)
- User-defined functions
- Function calls and recursion
- Print statements via C runtime
- Complete compilation pipeline

The LLVM backend successfully compiles Basalt programs to native executables using the llir/llvm library and system toolchain (llc, clang). 