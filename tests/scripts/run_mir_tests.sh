#!/bin/bash

# Test runner for Basalt MIR lowering
# This script runs all .bst files in the tests directory and reports MIR lowering results
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
SNAPSHOTS_DIR="tests/mir_snapshots"
TEMP_DIR="tests/mir_temp"

echo -e "${BLUE}=== Basalt MIR Lowering Test Suite ===${NC}"
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
    echo -e "${PURPLE}=== Generating MIR Lowering Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Generating MIR lowering snapshot for ${test_name}... "
        
        # Run the MIR lowering and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt mir "$test_file" 2>&1) || exit_code=$?
        
        if [ $exit_code -eq 0 ]; then
            # Success case - save the MIR output
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
    echo -e "${GREEN}Generated ${SNAPSHOT_TESTS} MIR lowering snapshots in $SNAPSHOTS_DIR${NC}"
    echo -e "${YELLOW}Please review the snapshots and commit them to version control${NC}"
}

# Function to compare snapshots
compare_snapshots() {
    echo -e "${PURPLE}=== Comparing MIR Lowering Snapshots ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        local snapshot_file=$(get_snapshot_path "$test_file")
        
        echo -n "Comparing MIR lowering snapshot for ${test_name}... "
        
        # Run the MIR lowering and capture output
        local exit_code=0
        local output=""
        output=$(./target/debug/basalt mir "$test_file" 2>&1) || exit_code=$?
        
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
    echo -e "${BLUE}=== MIR Lowering Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    echo -e "Errors: ${YELLOW}${ERROR_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ] && [ $ERROR_TESTS -eq 0 ]; then
        echo -e "${GREEN}All MIR lowering tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some MIR lowering tests failed!${NC}"
        exit 1
    fi
}

# Function to run basic tests
run_basic_tests() {
    echo -e "${PURPLE}=== Running MIR Lowering Tests ===${NC}"
    echo
    
    setup_directories
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -n "Testing MIR lowering for ${test_name}... "
        
        # Run the MIR lowering
        local exit_code=0
        ./target/debug/basalt mir "$test_file" > /dev/null 2>&1 || exit_code=$?
        
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
    echo -e "${BLUE}=== MIR Lowering Test Results ===${NC}"
    echo -e "Total tests: ${TOTAL_TESTS}"
    echo -e "Passed: ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed: ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "${GREEN}All MIR lowering tests passed!${NC}"
        exit 0
    else
        echo -e "${RED}Some MIR lowering tests failed!${NC}"
        exit 1
    fi
}

# Function to show detailed output
show_detailed() {
    echo -e "${PURPLE}=== Detailed MIR Lowering Test Output ===${NC}"
    echo
    
    # Find all .bst files in tests directory
    test_files=($(find tests -name "*.bst" | sort))
    
    for test_file in "${test_files[@]}"; do
        local test_name=$(basename "$test_file" .bst)
        
        echo -e "${BLUE}=== Test: ${test_name} ===${NC}"
        echo -e "${YELLOW}Source:${NC}"
        cat "$test_file"
        echo
        echo -e "${YELLOW}MIR Output:${NC}"
        ./target/debug/basalt mir "$test_file" 2>&1 || echo "ERROR: MIR lowering failed"
        echo
        echo "----------------------------------------"
        echo
    done
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
    --help|-h)
        echo "Usage: $0 [OPTION]"
        echo
        echo "Options:"
        echo "  --snapshot    Generate snapshot files from current MIR lowering output"
        echo "  --compare     Compare current MIR lowering output with saved snapshots"
        echo "  --detailed    Show detailed output for all tests"
        echo "  --help, -h    Show this help message"
        echo
        echo "If no option is provided, runs basic tests (pass/fail only)"
        ;;
    *)
        run_basic_tests
        ;;
esac 