# Basalt Examples

This directory contains example `.bst` files to demonstrate various features of the Basalt programming language.

## Running Examples

You can run any example using the main Basalt interpreter:

```bash
# Run from file
go run main.go examples/hello_world.bst

# Or pipe the code
cat examples/hello_world.bst | go run main.go
```

## Examples

### hello_world.bst
A simple "Hello, World!" program that demonstrates:
- Module imports (`std::io`)
- Basic print functionality

### arithmetic.bst
Demonstrates basic arithmetic operations:
- Variable declarations
- Mathematical expressions
- Print statements with multiple arguments

### strings.bst
Showcases string manipulation functions:
- `strings.contains()` - Check if string contains substring
- `strings.split()` - Split string by separator
- `strings.join()` - Join array of strings with separator

### file_io.bst
Demonstrates file I/O operations with proper error handling:
- `io.write_file()` - Write content to file
- `io.read_file()` - Read file content
- Result type handling for operations that can fail

### functions.bst
Shows function definition and usage:
- Function definitions with `fn`
- Functions with parameters and return values
- Recursive functions
- Higher-order functions

## Building the Interpreter

To build the Basalt interpreter:

```bash
go build -o basalt main.go
```

Then run examples with:

```bash
./basalt examples/hello_world.bst
```

## Standard Library

The examples use the following standard library modules:

- `std::io` - Input/output operations (print, read_file, write_file)
- `std::strings` - String manipulation functions (split, join, contains)

All file operations return `Result` types for proper error handling. 