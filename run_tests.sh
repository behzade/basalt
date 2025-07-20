#!/bin/bash

# Test runner for Basalt parser
# This script runs all .bst files in the tests directory and reports results

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
ERROR_TESTS=0

echo -e "${BLUE}=== Basalt Parser Test Suite ===${NC}"
echo

devbox run -- cargo build

# Function to run a test
run_test() {
    local test_file="$1"
    local test_name=$(basename "$test_file" .bst)
    local expected_to_fail=false
    
    # Check if this is an error test (should fail)
    if [[ "$test_name" == error-* ]]; then
        expected_to_fail=true
    fi
    
    echo -n "Testing ${test_name}... "
    
    # Run the parser and capture exit code
    local exit_code=0
    local output=""
    output=$(./target/debug/basalt parse < "$test_file" 2>&1) || exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        if [ "$expected_to_fail" = true ]; then
            echo -e "${YELLOW}UNEXPECTED PASS${NC}"
            echo "  Expected to fail but passed"
            ((ERROR_TESTS++))
        else
            echo -e "${GREEN}PASS${NC}"
            ((PASSED_TESTS++))
        fi
    else
        if [ "$expected_to_fail" = true ]; then
            echo -e "${GREEN}EXPECTED FAIL${NC}"
            ((PASSED_TESTS++))
        else
            echo -e "${RED}FAIL${NC}"
            echo "  Error: $output"
            ((FAILED_TESTS++))
        fi
    fi
    
    ((TOTAL_TESTS++))
}

# Function to run a test and capture detailed output
run_test_detailed() {
    local test_file="$1"
    local test_name=$(basename "$test_file" .bst)
    local expected_to_fail=false
    
    # Check if this is an error test (should fail)
    if [[ "$test_name" == error-* ]]; then
        expected_to_fail=true
    fi
    
    echo -e "${BLUE}=== Testing ${test_name} ===${NC}"
    echo "File: $test_file"
    echo "Expected to fail: $expected_to_fail"
    echo
    
    # Show first few lines of the test file
    echo "Test content (first 10 lines):"
    head -10 "$test_file" | sed 's/^/  /'
    echo
    
    # Run the parser and capture exit code
    local exit_code=0
    local output=""
    output=$(./target/debug/basalt parse < "$test_file" 2>&1) || exit_code=$?
    
    echo "Parser output:"
    if [ $exit_code -eq 0 ]; then
        if [ "$expected_to_fail" = true ]; then
            echo -e "${YELLOW}UNEXPECTED PASS${NC}"
            echo "  Expected to fail but passed"
            ((ERROR_TESTS++))
        else
            echo -e "${GREEN}PASS${NC}"
            echo "$output" | head -20 | sed 's/^/  /'
            ((PASSED_TESTS++))
        fi
    else
        if [ "$expected_to_fail" = true ]; then
            echo -e "${GREEN}EXPECTED FAIL${NC}"
            echo "$output" | head -10 | sed 's/^/  /'
            ((PASSED_TESTS++))
        else
            echo -e "${RED}FAIL${NC}"
            echo "$output" | head -10 | sed 's/^/  /'
            ((FAILED_TESTS++))
        fi
    fi
    
    ((TOTAL_TESTS++))
    echo
}

# Check if we should run in detailed mode
DETAILED=false
if [[ "$1" == "--detailed" || "$1" == "-d" ]]; then
    DETAILED=true
fi

# Find all .bst files in tests directory
test_files=($(find tests -name "*.bst" | sort))

if [ ${#test_files[@]} -eq 0 ]; then
    echo -e "${RED}No .bst files found in tests directory${NC}"
    exit 1
fi

echo -e "${BLUE}Found ${#test_files[@]} test files${NC}"
echo

# Run tests
for test_file in "${test_files[@]}"; do
    if [ "$DETAILED" = true ]; then
        run_test_detailed "$test_file"
    else
        run_test "$test_file"
    fi
done

echo
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "Total tests: ${TOTAL_TESTS}"
echo -e "${GREEN}Passed: ${PASSED_TESTS}${NC}"
echo -e "${RED}Failed: ${FAILED_TESTS}${NC}"
echo -e "${YELLOW}Unexpected results: ${ERROR_TESTS}${NC}"

# Calculate success rate
if [ $TOTAL_TESTS -gt 0 ]; then
    success_rate=$((PASSED_TESTS * 100 / TOTAL_TESTS))
    echo -e "Success rate: ${success_rate}%"
fi

echo

# Exit with error if any tests failed unexpectedly
if [ $FAILED_TESTS -gt 0 ] || [ $ERROR_TESTS -gt 0 ]; then
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
else
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
fi 
