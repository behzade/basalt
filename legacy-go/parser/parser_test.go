package parser

import (
	"testing"

	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/testutil"
)

func TestArithmetic(t *testing.T) {
	runTestFile(t, "../tests/arithmetic.test")
}

func TestArrays(t *testing.T) {
	runTestFile(t, "../tests/arrays.test")
}

func TestBooleans(t *testing.T) {
	runTestFile(t, "../tests/booleans.test")
}

func TestControlFlow(t *testing.T) {
	runTestFile(t, "../tests/control_flow.test")
}

func TestFunctions(t *testing.T) {
	runTestFile(t, "../tests/functions.test")
}

func TestStructs(t *testing.T) {
	runTestFile(t, "../tests/structs.test")
}

func TestHashmaps(t *testing.T) {
	runTestFile(t, "../tests/hashmaps.test")
}

// runTestFile executes tests from a specific test file
func runTestFile(t *testing.T, filepath string) {
	testCases, err := testutil.ParseTestFile(filepath)
	if err != nil {
		t.Fatalf("Failed to parse test file %s: %v", filepath, err)
	}

	for _, tc := range testCases {
		t.Run(tc.Name, func(t *testing.T) {
			switch tc.Type {
			case "AST":
				runASTTest(t, tc)
			case "EVAL", "ERROR":
				// Skip evaluation and error tests in parser
				t.Skip("Skipping non-AST test in parser")
			default:
				t.Fatalf("Unknown test type: %s", tc.Type)
			}
		})
	}
}

// runASTTest executes an AST parsing test
func runASTTest(t *testing.T, tc testutil.TestCase) {
	l := lexer.New(tc.Input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	actual := program.String()
	if actual != tc.Expected {
		t.Errorf("Expected AST: %s, got: %s", tc.Expected, actual)
	}
}
