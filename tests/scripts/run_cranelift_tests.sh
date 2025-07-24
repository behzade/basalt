#!/bin/bash

# Test runner for Basalt Cranelift IR code generation
# This script runs all .bst files in the tests directory and reports Cranelift IR generation results
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
SNAPSHOTS_DIR="tests/cranelift_snapshots"
TEMP_DIR="tests/cranelift_temp"

echo -e "${BLUE}=== Basalt Cranelift IR Code Generation Test Suite ===${NC}"
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
    echo -e "${PURPLE}=== Generating Cranelift IR Code Generation Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Generating Cranelift IR snapshot for ${test_name}... "
        
        # Run the Cranelift IR generation and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt cranelift "$test_file" 2>&1) || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Success case - save the Cranelift IR output
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
    echo -e "${GREEN}Generated ${SNAPSHOT_TESTS} Cranelift IR snapshots in $SNAPSHOTS_DIR${NC}"
    echo -e "${YELLOW}Please review the snapshots and commit them to version control${NC}"
}

# Function to compare snapshots
compare_snapshots() {
    echo -e "${PURPLE}=== Comparing Cranelift IR Code Generation Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Comparing Cranelift IR snapshot for ${test_name}... "
        
        # Run the Cranelift IR generation and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt cranelift "$test_file" 2>&1) || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Success case - compare with snapshot
            if [ -f "$snapshot_file" ]; then
                local current_file="$TEMP_DIR/${test_name}.current"
                echo "$output" > "$current_file"
                
                if diff -q "$snapshot_file" "$current_file" > /dev/null; then
                    echo -e "${GREEN}✓${NC}"
                    ((PASSED_TESTS++))
                else
                    echo -e "${RED}✗${NC}"
                    echo -e "${RED}  Snapshot mismatch for ${test_name}${NC}"
                    ((FAILED_TESTS++))
                fi
            else
                echo -e "${YELLOW}⚠ (no snapshot)${NC}"
                ((ERROR_TESTS++))
            fi
        else
            # Error case - compare with snapshot
            if [ -f "$snapshot_file" ]; then
                local current_file="$TEMP_DIR/${test_name}.current"
                echo "ERROR: $output" > "$current_file"
                
                if diff -q "$snapshot_file" "$current_file" > /dev/null; then
                    echo -e "${GREEN}✓${NC}"
                    ((PASSED_TESTS++))
                else
                    echo -e "${RED}✗${NC}"
                    echo -e "${RED}  Error snapshot mismatch for ${test_name}${NC}"
                    ((FAILED_TESTS++))
                fi
            else
                echo -e "${YELLOW}⚠ (no error snapshot)${NC}"
                ((ERROR_TESTS++))
            fi
        fi
        
        ((TOTAL_TESTS++))
    done
    
    echo
    echo -e "${BLUE}=== Cranelift IR Code Generation Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    echo -e "Errors: ${YELLOW}${ERROR_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ] && [ $ERROR_TESTS -eq 0 ]; then
        echo -e "${GREEN}All Cranelift IR code generation tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some Cranelift IR code generation tests failed!${NC}"
        exit 1
    fi
}

# Function to run basic tests
run_basic_tests() {
    echo -e "${PURPLE}=== Running Cranelift IR Code Generation Tests ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -n "Testing Cranelift IR generation for ${test_name}... "
        
        # Run the Cranelift IR generation
        local exit_code=0
        ./target/debug/basalt cranelift "$test_file" > /dev/null 2>&1 || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            echo -e "${GREEN}✓${NC}"
            ((PASSED_TESTS++))
        else
            echo -e "${RED}✗${NC}"
            ((FAILED_TESTS++))
        fi
        
        ((TOTAL_TESTS++))
    done
    
    echo
    echo -e "${BLUE}=== Cranelift IR Code Generation Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All Cranelift IR code generation tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some Cranelift IR code generation tests failed!${NC}"
        exit 1
    fi
}

# Function to show detailed output
show_detailed() {
    echo -e "${PURPLE}=== Detailed Cranelift IR Code Generation Test Output ===${NC}"
    echo
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -e "${BLUE}=== Test: ${test_name} ===${NC}"
        echo -e "${YELLOW}Source:${NC}"
        cat "$test_file"
        echo
        echo -e "${YELLOW}Cranelift IR Output:${NC}"
        ./target/debug/basalt cranelift "$test_file" 2>&1 || echo "ERROR: Cranelift IR generation failed"
        echo
        echo "----------------------------------------"
        echo
    done
}

# Function to run validation tests
run_validation_tests() {
    echo -e "${PURPLE}=== Running Cranelift IR Validation Tests ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -n "Validating Cranelift IR for ${test_name}... "
        
        # Run the Cranelift IR generation and validation
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt cranelift --validate "$test_file" 2>&1) || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            echo -e "${GREEN}✓${NC}"
            ((PASSED_TESTS++))
        else
            echo -e "${RED}✗${NC}"
            echo -e "${RED}  Validation failed: $output${NC}"
            ((FAILED_TESTS++))
        fi
        
        ((TOTAL_TESTS++))
    done
    
    echo
    echo -e "${BLUE}=== Cranelift IR Validation Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All Cranelift IR validation tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some Cranelift IR validation tests failed!${NC}"
        exit 1
    fi
}

# Function to run performance tests
run_performance_tests() {
    echo -e "${PURPLE}=== Running Cranelift IR Performance Tests ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -n "Performance test for ${test_name}... "
        
        # Run the Cranelift IR generation with timing
        local start_time=$(date +%s%N)
        local exit_code=0
        ./target/debug/basalt cranelift "$test_file" > /dev/null 2>&1 || exit_code=$?
        local end_time=$(date +%s%N)
        
        if [ $exit_code -eq 0 ]; then
            local duration=$(( (end_time - start_time) / 1000000 ))  # Convert to milliseconds
            echo -e "${GREEN}✓ (${duration}ms)${NC}"
            ((PASSED_TESTS++))
        else
            echo -e "${RED}✗${NC}"
            ((FAILED_TESTS++))
        fi
        
        ((TOTAL_TESTS++))
    done
    
    echo
    echo -e "${BLUE}=== Cranelift IR Performance Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All Cranelift IR performance tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some Cranelift IR performance tests failed!${NC}"
        exit 1
    fi
}

# Function to clean up temporary files
cleanup() {
    echo -e "${PURPLE}=== Cleaning Up Temporary Files ===${NC}"
    rm -rf "$TEMP_DIR"
    echo -e "${GREEN}Cleanup complete${NC}"
}

# Main script logic
case "${1:-}" in
    --snapshot)
        generate_snapshots
        ;;
    --compare)
        compare_snapshots
        ;;
    --detailed)
        show_detailed
        ;;
    --validate)
        run_validation_tests
        ;;
    --performance)
        run_performance_tests
        ;;
    --cleanup)
        cleanup
        ;;
    --help|-h)
        echo "Usage: $0 [OPTION]"
        echo
        echo "Options:"
        echo "  --snapshot     Generate snapshot files from current Cranelift IR output"
        echo "  --compare      Compare current Cranelift IR output with saved snapshots"
        echo "  --detailed     Show detailed output for all tests"
        echo "  --validate     Run validation tests on generated Cranelift IR"
        echo "  --performance  Run performance tests with timing"
        echo "  --cleanup      Clean up temporary test files"
        echo "  --help, -h     Show this help message"
        echo
        echo "If no option is provided, runs basic tests (pass/fail only)"
        ;;
    *)
        run_basic_tests
        ;;
esac 