#!/bin/bash

# Test runner for Basalt Cranelift compilation
# This script runs all .bst files in the tests/compile directory,
# compiles them, executes them via JIT, and checks their exit code.

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

echo -e "${BLUE}=== Basalt Cranelift Compilation Test Suite ===${NC}"
echo

# Build the compiler first
if ! cargo build; then
    echo -e "${RED}Compiler build failed. Aborting tests.${NC}"
    exit 1
fi

TEST_DIR="tests/compile"

# Find all .bst files in the compile test directory
test_files=($(find "$TEST_DIR" -name "*.bst" | sort))

for test_file in "${test_files[@]}"; do
    ((TOTAL_TESTS++))
    test_name=$(basename "$test_file" .bst)
    echo -n "Testing ${test_name}... "

    # Extract expected exit code from the test file
    expected_code=$(grep -o '// expected: [0-9]*' "$test_file" | sed 's/\/\/ expected: //')

    if [ -z "$expected_code" ]; then
        echo -e "${YELLOW}SKIPPED (no '// expected: ' comment found)${NC}"
        continue
    fi

    # Run the compiler and capture the output and exit code
    # The last line of output from our compiler should be the result
    output=$(./target/debug/basalt compile "$test_file" 2>/dev/null)
    actual_code=$(echo "$output" | tail -n 1)

    if [[ "$actual_code" == "$expected_code" ]]; then
        echo -e "${GREEN}✓ PASSED${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}✗ FAILED${NC}"
        echo -e "  Expected exit code: ${GREEN}${expected_code}${NC}"
        echo -e "  Actual exit code:   ${RED}${actual_code}${NC}"
        # You can add more detailed error logging here if needed
        # ./target/debug/basalt compile "$test_file" # Rerun for error output
        ((FAILED_TESTS++))
    fi
done

echo
echo -e "${BLUE}=== Test Results ===${NC}"
echo -e "Total: ${TOTAL_TESTS}, Passed: ${GREEN}${PASSED_TESTS}${NC}, Failed: ${RED}${FAILED_TESTS}${NC}"
echo

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}All compilation tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some compilation tests failed!${NC}"
    exit 1
fi 