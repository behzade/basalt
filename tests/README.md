# Basalt Parser Tests

This directory contains tests for the Basalt parser, including both test files (`.bst`) and their expected outputs (snapshots).

## Test Files

- **`*.bst`**: Basalt source code files that test various language features
- **`snapshots/*.snapshot`**: Expected parser outputs for each test
- **`temp/*.current`**: Temporary files containing current parser outputs during comparison

## Test Categories

### Basic Tests (01-16)
These tests validate that the parser correctly handles various language constructs:

- `01-literals.bst` - Basic literal values (numbers, strings, booleans)
- `02-typed-literals.bst` - Literals with explicit type annotations
- `03-basic-functions.bst` - Simple function definitions
- `04-expressions.bst` - Arithmetic and logical expressions
- `05-arrays.bst` - Array literals and operations
- `06-maps.bst` - Map literals and operations
- `07-structs.bst` - Struct definitions and instantiations
- `08-traits.bst` - Trait definitions
- `09-impls.bst` - Implementation blocks
- `10-generics.bst` - Generic type parameters
- `11-imports.bst` - Import statements
- `12-control-flow.bst` - If statements, loops, match expressions
- `13-enums.bst` - Enum definitions and variants
- `14-effects.bst` - Effect definitions and operations
- `15-extern.bst` - External function declarations
- `16-complex.bst` - Complex combinations of features

### Error Tests (error-*)
These tests validate that the parser correctly reports errors for invalid code:

- `error-01-syntax.bst` - Syntax errors
- `error-02-incomplete.bst` - Incomplete/invalid constructs

## Running Tests

The test runner (`../run_tests.sh`) supports several modes:

### Basic Testing
```bash
./run_tests.sh
```
Runs all tests and reports pass/fail based on whether the parser succeeds or fails as expected.

### Snapshot Generation
```bash
./run_tests.sh --snapshot
```
Generates snapshot files from the current parser output. Use this when:
- Adding new tests
- Making intentional changes to parser behavior
- Setting up the test suite for the first time

### Snapshot Comparison
```bash
./run_tests.sh --compare
```
Compares current parser output against saved snapshots. This validates that:
- The parser produces the expected AST structure
- Error messages are consistent
- No unintended changes have been introduced

### Detailed Output
```bash
./run_tests.sh --detailed
```
Shows detailed output for each test, including the test content and parser output.

## Workflow

### Adding New Tests

1. Create a new `.bst` file in the `tests/` directory
2. Run `./run_tests.sh --snapshot` to generate snapshots
3. Review the generated snapshots in `tests/snapshots/`
4. Commit both the test file and its snapshot

### Making Parser Changes

1. Make your changes to the parser
2. Run `./run_tests.sh --compare` to see what changed
3. If the changes are intentional:
   - Run `./run_tests.sh --snapshot` to update snapshots
   - Review the changes
   - Commit the updated snapshots
4. If the changes are unintentional:
   - Fix the parser to maintain expected behavior
   - Re-run comparison to verify

### Continuous Integration

The snapshot testing system ensures that:
- Parser changes don't break existing functionality
- AST structure remains consistent
- Error messages are predictable
- Tests are deterministic and reproducible

## Snapshot Format

Snapshots contain the exact output from the parser, including:
- Successfully parsed AST in debug format
- Error messages with full context
- Any additional output from the parser

This ensures that even subtle changes in the parser output are detected and can be reviewed.

## Troubleshooting

### Snapshot Mismatches

If snapshots don't match:
1. Check if the change is intentional
2. Use `diff tests/snapshots/test.snapshot tests/temp/test.current` to see differences
3. Update snapshots with `--snapshot` if changes are correct
4. Fix parser if changes are incorrect

### Missing Snapshots

If a test has no snapshot:
1. Run `--snapshot` to generate it
2. Review the generated snapshot
3. Commit it to version control

### Error Tests

Error tests should have snapshots that contain error messages. The test runner handles these appropriately by prefixing error output with "ERROR:". 