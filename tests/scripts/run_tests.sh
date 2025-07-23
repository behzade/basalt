#!/bin/bash

# Test runner for Basalt parser
# This script runs all .bst files in the tests directory and reports results
# Supports snapshot testing with --snapshot and --compare modes

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
NC='\033[0m' # No Color

# Test counters
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
ERROR_TESTS=0
SNAPSHOT_TESTS=0

# Directories
SNAPSHOTS_DIR="tests/snapshots"
TEMP_DIR="tests/temp"

echo -e "${BLUE}=== Basalt Parser Test Suite ===${NC}"
echo

devbox run -- cargo build

# Function to create directories if they don't exist
setup_directories() {
    mkdir -p "$SNAPSHOTS_DIR"
    mkdir -p "$TEMP_DIR"
}

# Function to get snapshot file path
get_snapshot_path() {
    local test_file="$1"
    local test_name=$(basename "$test_file" .bst)
    echo "$SNAPSHOTS_DIR/${test_name}.snapshot"
}

# Function to generate snapshots
generate_snapshots() {
    echo -e "${PURPLE}=== Generating Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Generating snapshot for ${test_name}... "
        
        # Run the parser and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt parse < "$test_file" 2>&1) || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Success case - save the AST output
            echo "$output" > "$snapshot_file"
            echo -e "${GREEN}✓${NC}"
            ((SNAPSHOT_TESTS++))
        else
            # Error case - save the error output
            echo "ERROR: $output" > "$snapshot_file"
            echo -e "${YELLOW}⚠ (error)${NC}"
            ((SNAPSHOT_TESTS++))
        fi
        
        ((TOTAL_TESTS++))
    done
    
    echo
    echo -e "${GREEN}Generated ${SNAPSHOT_TESTS} snapshots in $SNAPSHOTS_DIR${NC}"
    echo -e "${YELLOW}Please review the snapshots and commit them to version control${NC}"
}

# Function to compare snapshots
compare_snapshots() {
    echo -e "${PURPLE}=== Comparing Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Comparing snapshot for ${test_name}... "
        
        # Check if snapshot exists
        if [ ! -f "$snapshot_file" ]; then
            echo -e "${RED}✗ (no snapshot)${NC}"
            echo "  No snapshot file found: $snapshot_file"
            ((FAILED_TESTS++))
            ((TOTAL_TESTS++))
            continue
        fi
        
        # Run the parser and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt parse < "$test_file" 2>&1) || exit_code=$?
        
        # Create temporary file for current output
        local temp_file="$TEMP_DIR/${test_name}.current"
        if [ $exit_code -eq 0 ]; then
            echo "$output" > "$temp_file"
        else
            echo "ERROR: $output" > "$temp_file"
        fi
        
        # Compare with snapshot
        if diff -q "$snapshot_file" "$temp_file" > /dev/null; then
            echo -e "${GREEN}✓${NC}"
            ((PASSED_TESTS++))
        else
            echo -e "${RED}✗${NC}"
            echo "  Snapshot differs from current output"
            echo "  Expected: $snapshot_file"
            echo "  Current:  $temp_file"
            ((FAILED_TESTS++))
        fi
        
        ((TOTAL_TESTS++))
    done
}

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

# Function to show help
show_help() {
    echo "Usage: $0 [OPTIONS]"
    echo
    echo "Options:"
    echo "  --snapshot, -s    Generate snapshots from current parser output"
    echo "  --compare, -c     Compare current parser output with snapshots"
    echo "  --detailed, -d    Run tests in detailed mode (shows more output)"
    echo "  --help, -h        Show this help message"
    echo
    echo "Modes:"
    echo "  Default:          Run basic tests (no snapshot validation)"
    echo "  --snapshot:       Generate expected outputs for all tests"
    echo "  --compare:        Validate current outputs against snapshots"
    echo
    echo "Examples:"
    echo "  $0                # Run basic tests"
    echo "  $0 --snapshot     # Generate snapshots"
    echo "  $0 --compare      # Compare with snapshots"
    echo "  $0 --detailed     # Run with detailed output"
}

# Parse command line arguments
SNAPSHOT_MODE=false
COMPARE_MODE=false
DETAILED=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --snapshot|-s)
            SNAPSHOT_MODE=true
            shift
            ;;
        --compare|-c)
            COMPARE_MODE=true
            shift
            ;;
        --detailed|-d)
            DETAILED=true
            shift
            ;;
        --help|-h)
            show_help
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Run appropriate mode
if [ "$SNAPSHOT_MODE" = true ]; then
    generate_snapshots
elif [ "$COMPARE_MODE" = true ]; then
    compare_snapshots
else
    # Default mode: run basic tests
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
fi

echo
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "Total tests: ${TOTAL_TESTS}"
echo -e "${GREEN}Passed: ${PASSED_TESTS}${NC}"
echo -e "${RED}Failed: ${FAILED_TESTS}${NC}"
echo -e "${YELLOW}Unexpected results: ${ERROR_TESTS}${NC}"
if [ "$SNAPSHOT_MODE" = true ]; then
    echo -e "${PURPLE}Snapshots generated: ${SNAPSHOT_TESTS}${NC}"
fi

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
