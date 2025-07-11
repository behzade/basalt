package evaluator

import (
	"fmt"
	"strconv"
	"strings"
	"testing"

	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/object"
	"github.com/behzade/basalt/parser"
	"github.com/behzade/basalt/testutil"
)

func TestArithmetic(t *testing.T) {
	runTestFile(t, "../tests/arithmetic.test")
}

func TestArrays(t *testing.T) {
	runTestFile(t, "../tests/arrays.test")
}

func TestStrings(t *testing.T) {
	runTestFile(t, "../tests/strings.test")
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

func TestErrors(t *testing.T) {
	runTestFile(t, "../tests/errors.test")
}

func TestStructs(t *testing.T) {
	runTestFile(t, "../tests/structs.test")
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
			case "EVAL":
				runEvalTest(t, tc)
			case "ERROR":
				runErrorTest(t, tc)
			case "AST":
				// Skip AST tests in evaluator
				t.Skip("Skipping AST test in evaluator")
			default:
				t.Fatalf("Unknown test type: %s", tc.Type)
			}
		})
	}
}

// runEvalTest executes an evaluation test
func runEvalTest(t *testing.T, tc testutil.TestCase) {
	evaluated := testEval(tc.Input)

	// Handle different expected value types
	switch tc.Expected {
	case "null":
		testOptionObject(t, evaluated, nil)
	case "None":
		testOptionObject(t, evaluated, nil)
	case "function":
		// Just check if it's a function object
		if _, ok := evaluated.(*object.Function); !ok {
			t.Errorf("Expected function object, got %T", evaluated)
		}
	default:
		// Try to parse as different types
		if strings.HasPrefix(tc.Expected, "\"") && strings.HasSuffix(tc.Expected, "\"") {
			// String literal
			expected := strings.Trim(tc.Expected, "\"")
			testOptionObject(t, evaluated, expected)
		} else if strings.HasPrefix(tc.Expected, "[") && strings.HasSuffix(tc.Expected, "]") {
			// Array literal
			expected := parseArrayLiteral(tc.Expected)
			testOptionObject(t, evaluated, expected)
		} else if tc.Expected == "true" {
			testOptionObject(t, evaluated, true)
		} else if tc.Expected == "false" {
			testOptionObject(t, evaluated, false)
		} else if val, err := strconv.ParseInt(tc.Expected, 10, 64); err == nil {
			testOptionObject(t, evaluated, val)
		} else if val, err := strconv.ParseFloat(tc.Expected, 64); err == nil {
			testOptionObject(t, evaluated, val)
		} else {
			t.Errorf("Unknown expected value format: %s", tc.Expected)
		}
	}
}

// runErrorTest executes an error test
func runErrorTest(t *testing.T, tc testutil.TestCase) {
	evaluated := testEval(tc.Input)

	errObj, ok := evaluated.(*object.Error)
	if !ok {
		t.Errorf("Expected error object, got %T(%+v)", evaluated, evaluated)
		return
	}

	if errObj.Message != tc.Expected {
		t.Errorf("Expected error message: %s, got: %s", tc.Expected, errObj.Message)
	}
}

// testEval evaluates input and returns the result
func testEval(input string) object.Object {
	l := lexer.New(input)
	p := parser.New(l)
	program := p.ParseProgram()

	// Check for parser errors
	if len(p.Errors()) > 0 {
		return &object.Error{Message: fmt.Sprintf("parser errors: %v", p.Errors())}
	}

	env := object.NewEnvironment()
	evaluated := Eval(program, env)

	// Unwrap return values to get the actual object for testing
	if returnValue, ok := evaluated.(*object.ReturnValue); ok {
		return returnValue.Value
	}

	return evaluated
}

// testOptionObject tests Option objects (Some/None)
func testOptionObject(t *testing.T, obj object.Object, expected interface{}) bool {
	if expected == nil {
		// Expecting a None object
		if obj != NONE {
			t.Errorf("object is not NONE. got=%T (%+v)", obj, obj)
			return false
		}
		return true
	}

	// Expecting a Some object
	some, ok := obj.(*object.Some)
	if !ok {
		t.Errorf("object is not Some. got=%T (%+v)", obj, obj)
		return false
	}

	// Test the value contained within Some
	return testLiteralObject(t, some.Value, expected)
}

// testLiteralObject tests literal objects
func testLiteralObject(t *testing.T, obj object.Object, expected interface{}) bool {
	switch v := expected.(type) {
	case int:
		return testIntegerObject(t, obj, int64(v))
	case int64:
		return testIntegerObject(t, obj, v)
	case float64:
		return testFloatObject(t, obj, v)
	case bool:
		return testBooleanObject(t, obj, v)
	case string:
		return testStringObject(t, obj, v)
	case []int64:
		return testArrayObject(t, &object.Some{Value: obj}, v)
	default:
		t.Errorf("type of expected not handled. got=%T", expected)
		return false
	}
}

// testIntegerObject tests integer objects
func testIntegerObject(t *testing.T, obj object.Object, expected int64) bool {
	result, ok := obj.(*object.Integer)
	if !ok {
		t.Errorf("object is not Integer. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%d, want=%d", result.Value, expected)
		return false
	}
	return true
}

// testFloatObject tests float objects
func testFloatObject(t *testing.T, obj object.Object, expected float64) bool {
	result, ok := obj.(*object.Float)
	if !ok {
		t.Errorf("object is not Float. got=%T (%+v)", obj, obj)
		return false
	}
	// Use tolerance-based comparison for floating-point values
	const tolerance = 1e-9
	if abs(result.Value-expected) > tolerance {
		t.Errorf("object has wrong value. got=%f, want=%f", result.Value, expected)
		return false
	}
	return true
}

// abs returns the absolute value of a float64
func abs(x float64) float64 {
	if x < 0 {
		return -x
	}
	return x
}

// testBooleanObject tests boolean objects
func testBooleanObject(t *testing.T, obj object.Object, expected bool) bool {
	result, ok := obj.(*object.Boolean)
	if !ok {
		t.Errorf("object is not Boolean. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%t, want=%t", result.Value, expected)
		return false
	}
	return true
}

// testStringObject tests string objects
func testStringObject(t *testing.T, obj object.Object, expected string) bool {
	result, ok := obj.(*object.String)
	if !ok {
		t.Errorf("object is not String. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%q, want=%q", result.Value, expected)
		return false
	}
	return true
}

// testArrayObject tests array objects
func testArrayObject(t *testing.T, obj object.Object, expected []int64) bool {
	result, ok := obj.(*object.Some)
	if !ok {
		t.Errorf("object is not Some. got=%T (%+v)", obj, obj)
		return false
	}

	array, ok := result.Value.(*object.Array)
	if !ok {
		// Check if it's a slice instead
		if _, ok := result.Value.(*object.Slice); ok {
			return testSliceObject(t, obj, expected)
		}
		t.Errorf("object is not Array or Slice. got=%T (%+v)", result.Value, result.Value)
		return false
	}

	if len(array.Elements) != len(expected) {
		t.Errorf("wrong num of elements. want=%d, got=%d", len(expected), len(array.Elements))
		return false
	}

	for i, expectedElem := range expected {
		elem, ok := array.Elements[i].(*object.Some)
		if !ok {
			t.Errorf("array element %d is not Some. got=%T (%+v)", i, array.Elements[i], array.Elements[i])
			return false
		}

		intObj, ok := elem.Value.(*object.Integer)
		if !ok {
			t.Errorf("array element %d is not Integer. got=%T (%+v)", i, elem.Value, elem.Value)
			return false
		}

		if intObj.Value != expectedElem {
			t.Errorf("array element %d wrong value. want=%d, got=%d", i, expectedElem, intObj.Value)
			return false
		}
	}

	return true
}

// testSliceObject tests slice objects
func testSliceObject(t *testing.T, obj object.Object, expected []int64) bool {
	result, ok := obj.(*object.Some)
	if !ok {
		t.Errorf("object is not Some. got=%T (%+v)", obj, obj)
		return false
	}

	slice, ok := result.Value.(*object.Slice)
	if !ok {
		t.Errorf("object is not Slice. got=%T (%+v)", result.Value, result.Value)
		return false
	}

	if len(slice.Elements) != len(expected) {
		t.Errorf("wrong num of elements. want=%d, got=%d", len(expected), len(slice.Elements))
		return false
	}

	for i, expectedElem := range expected {
		elem, ok := slice.Elements[i].(*object.Some)
		if !ok {
			t.Errorf("slice element %d is not Some. got=%T (%+v)", i, slice.Elements[i], slice.Elements[i])
			return false
		}

		intObj, ok := elem.Value.(*object.Integer)
		if !ok {
			t.Errorf("slice element %d is not Integer. got=%T (%+v)", i, elem.Value, elem.Value)
			return false
		}

		if intObj.Value != expectedElem {
			t.Errorf("slice element %d wrong value. want=%d, got=%d", i, expectedElem, intObj.Value)
			return false
		}
	}

	return true
}

// parseArrayLiteral parses a string like "[1, 2, 3]" into []int64
func parseArrayLiteral(s string) []int64 {
	s = strings.Trim(s, "[]")
	if s == "" {
		return []int64{}
	}

	parts := strings.Split(s, ",")
	result := make([]int64, 0, len(parts))

	for _, part := range parts {
		part = strings.TrimSpace(part)
		if val, err := strconv.ParseInt(part, 10, 64); err == nil {
			result = append(result, val)
		}
	}

	return result
}
