#!/bin/bash

# Unified test runner for Basalt compiler pipeline
# This script runs all .bst files in the tests directory and reports results for different stages
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

# Test type configuration (compatible with bash 3.2)
# Format: test_type:command:display_name
TEST_TYPES="ast:parse:AST hir:hir:HIR mir:mir:MIR cir:cir:Cranelift IR build:build:Compilation"

# Default test type
DEFAULT_TEST_TYPE="ast"

echo -e "${BLUE}=== Basalt Unified Test Suite ===${NC}"
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
    local test_type="$2"
    local test_name=$(basename "$test_file" .bst)
    echo "$SNAPSHOTS_DIR/${test_name}.${test_type}"
}

# Function to get test type info
get_test_type_info() {
    local test_type="$1"
    local info=$(echo "$TEST_TYPES" | tr ' ' '\n' | grep "^$test_type:" | head -1)
    if [ -z "$info" ]; then
        echo "Unknown test type: $test_type" >&2
        exit 1
    fi
    echo "$info"
}

# Function to get command from test type
get_command() {
    local test_type="$1"
    local info=$(get_test_type_info "$test_type")
    echo "$info" | cut -d: -f2
}

# Function to get display name from test type
get_display_name() {
    local test_type="$1"
    local info=$(get_test_type_info "$test_type")
    echo "$info" | cut -d: -f3
}

# Function to run a test and capture output
run_test_command() {
    local test_file="$1"
    local test_type="$2"
    local command=$(get_command "$test_type")
    
    case "$test_type" in
        "ast"|"hir"|"mir"|"cir")
            # For these types, we pass the file as an argument
            ./target/debug/basalt "$command" "$test_file" 2>&1
            ;;
        "compile")
            # For compile tests, we need to handle expected exit codes
            local expected_code=$(grep -o '// expected: [0-9]*' "$test_file" | sed 's/\/\/ expected: //')
            if [ -z "$expected_code" ]; then
                echo "ERROR: No '// expected: ' comment found in $test_file"
                return 1
            fi
            local output=$(./target/debug/basalt "$command" "$test_file" 2>/dev/null)
            local actual_code=$(echo "$output" | tail -n 1)
            if [[ "$actual_code" == "$expected_code" ]]; then
                echo "PASSED: Expected $expected_code, got $actual_code"
                return 0
            else
                echo "FAILED: Expected $expected_code, got $actual_code"
                return 1
            fi
            ;;
        *)
            echo "ERROR: Unknown test type: $test_type"
            return 1
            ;;
    esac
}

# Function to generate snapshots
generate_snapshots() {
    local test_type="$1"
    local display_name=$(get_display_name "$test_type")
    
    echo -e "${PURPLE}=== Generating ${display_name} Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file" "$test_type")
        
        echo -n "Generating ${test_type} snapshot for ${test_name}... "
        
        # Run the command and capture output
        local exit_code=0
        local output=""
        output=$(run_test_command "$test_file" "$test_type") || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Success case - save the output
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
    echo -e "${GREEN}Generated ${SNAPSHOT_TESTS} ${test_type} snapshots in $SNAPSHOTS_DIR${NC}"
    echo -e "${YELLOW}Please review the snapshots and commit them to version control${NC}"
}

# Function to compare snapshots
compare_snapshots() {
    local test_type="$1"
    local display_name=$(get_display_name "$test_type")
    
    echo -e "${PURPLE}=== Comparing ${display_name} Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file" "$test_type")
        
        echo -n "Comparing ${test_type} snapshot for ${test_name}... "
        
        if [ ! -f "$snapshot_file" ]; then
            echo -e "${RED}✗ (no snapshot)${NC}"
            ((FAILED_TESTS++))
            ((TOTAL_TESTS++))
            continue
        fi
        
        # Run the command and capture output
        local exit_code=0
        local output=""
        output=$(run_test_command "$test_file" "$test_type") || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Compare with snapshot
            if diff -q <(echo "$output") "$snapshot_file" >/dev/null 2>&1; then
                echo -e "${GREEN}✓${NC}"
                ((PASSED_TESTS++))
            else
                echo -e "${RED}✗ (mismatch)${NC}"
                if [ "$DETAILED" = true ]; then
                    echo "  Differences:"
                    diff -u "$snapshot_file" <(echo "$output") | sed 's/^/    /'
                fi
                ((FAILED_TESTS++))
            fi
        else
            # Check if error matches snapshot
            if diff -q <(echo "ERROR: $output") "$snapshot_file" >/dev/null 2>&1; then
                echo -e "${GREEN}✓ (expected error)${NC}"
                ((PASSED_TESTS++))
            else
                echo -e "${RED}✗ (unexpected error)${NC}"
                if [ "$DETAILED" = true ]; then
                    echo "  Expected:"
                    cat "$snapshot_file" | sed 's/^/    /'
                    echo "  Got:"
                    echo "ERROR: $output" | sed 's/^/    /'
                fi
                ((FAILED_TESTS++))
            fi
        fi
        
        ((TOTAL_TESTS++))
    done
}

# Function to run basic test
run_test() {
    local test_file="$1"
    local test_type="$2"
    local test_name=$(basename "$test_file" .bst)
    
    echo -n "Testing ${test_name} (${test_type})... "
    
    local exit_code=0
    local output=""
    output=$(run_test_command "$test_file" "$test_type") || exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}✗${NC}"
        if [ "$DETAILED" = true ]; then
            echo "  Error: $output"
        fi
        ((FAILED_TESTS++))
    fi
    
    ((TOTAL_TESTS++))
}

# Function to show help
show_help() {
    echo "Usage: $0 [OPTIONS] [TEST_TYPE]"
    echo
    echo "Test Types:"
    echo "$TEST_TYPES" | tr ' ' '\n' | while IFS=: read -r test_type command display_name; do
        echo "  $test_type: $display_name"
    done
    echo
    echo "Options:"
    echo "  --snapshot, -s    Generate expected outputs for all tests"
    echo "  --compare, -c     Validate current outputs against snapshots"
    echo "  --detailed, -d    Run with detailed output"
    echo "  --help, -h        Show this help message"
    echo
    echo "Modes:"
    echo "  Default:          Run basic tests (no snapshot validation)"
    echo "  --snapshot:       Generate expected outputs for all tests"
    echo "  --compare:        Validate current outputs against snapshots"
    echo
    echo "Examples:"
    echo "  $0 ast            # Run AST tests"
    echo "  $0 hir --snapshot # Generate HIR snapshots"
    echo "  $0 mir --compare  # Compare MIR with snapshots"
    echo "  $0 compile        # Run compilation tests"
    echo "  $0 --detailed     # Run with detailed output"
}

# Parse command line arguments
SNAPSHOT_MODE=false
COMPARE_MODE=false
DETAILED=false
TEST_TYPE="$DEFAULT_TEST_TYPE"

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
        ast|hir|mir|cir|compile)
            TEST_TYPE="$1"
            shift
            ;;
        *)
            echo "Unknown option: $1"
            show_help
            exit 1
            ;;
    esac
done

# Validate test type
if [ -z "$(get_test_type_info "$TEST_TYPE" 2>/dev/null)" ]; then
    echo "Unknown test type: $TEST_TYPE"
    show_help
    exit 1
fi

# Run appropriate mode
if [ "$SNAPSHOT_MODE" = true ]; then
    generate_snapshots "$TEST_TYPE"
elif [ "$COMPARE_MODE" = true ]; then
    compare_snapshots "$TEST_TYPE"
else
    # Default mode: run basic tests
    display_name=$(get_display_name "$TEST_TYPE")
    echo -e "${BLUE}=== Running ${display_name} Tests ===${NC}"
    echo
    
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
        run_test "$test_file" "$TEST_TYPE"
    done
fi

echo
echo -e "${BLUE}=== Test Summary ===${NC}"
echo -e "Test type: $(get_display_name "$TEST_TYPE")"
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
