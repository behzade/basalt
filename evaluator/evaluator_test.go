package evaluator

import (
	"testing"

	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/object"
	"github.com/behzade/basalt/parser"
)

func TestEvalIntegerExpression(t *testing.T) {
	tests := []struct {
		input    string
		expected int64
	}{
		{"5", 5},
		{"10", 10},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestEvalBooleanExpression(t *testing.T) {
	tests := []struct {
		input    string
		expected bool
	}{
		{"true", true},
		{"false", false},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestReturnStatements(t *testing.T) {
	tests := []struct {
		input    string
		expected int64
	}{
		{"return 10;", 10},
		{"return 10; 9;", 10},
		{"return 2 * 5;", 10},
		{"9; return 2 * 5; 9;", 10},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestBangOperator(t *testing.T) {
	tests := []struct {
		input    string
		expected bool
	}{
		{"!true", false},
		{"!false", true},
		{"!5", true},
		{"!!true", true},
		{"!!false", false},
		{"!!5", false},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestIntegerExpression(t *testing.T) {
	tests := []struct {
		input    string
		expected int64
	}{
		{"5", 5},
		{"10", 10},
		{"-5", -5},
		{"-10", -10},
		{"5 + 5 + 5 + 5 - 10", 10},
		{"2 * 2 * 2 * 2 * 2", 32},
		{"-50 + 100 + -50", 0},
		{"5 * 2 + 10", 20},
		{"5 + 2 * 10", 25},
		{"20 + 2 * -10", 0},
		{"50 / 2 * 2 + 10", 60},
		{"2 * (5 + 10)", 30},
		{"3 * 3 * 3 + 10", 37},
		{"3 * (3 * 3) + 10", 37},
		{"(5 + 10 * 2 + 15 / 3) * 2 + -10", 50},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestBooleanExpression(t *testing.T) {
	tests := []struct {
		input    string
		expected bool
	}{
		{"true", true},
		{"false", false},
		{"1 < 2", true},
		{"1 > 2", false},
		{"1 < 1", false},
		{"1 > 1", false},
		{"1 == 1", true},
		{"1 != 1", false},
		{"1 == 2", false},
		{"1 != 2", true},
		{"true == true", true},
		{"false == false", true},
		{"true == false", false},
		{"true != false", true},
		{"false != true", true},
		{"(1 < 2) == true", true},
		{"(1 < 2) == false", false},
		{"(1 > 2) == true", false},
		{"(1 > 2) == false", true},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestLetStatements(t *testing.T) {
	tests := []struct {
		input    string
		expected int64
	}{
		{"let a = 5; a;", 5},
		{"let a = 5 * 5; a;", 25},
		{"let a = 5; let b = a; b;", 5},
		{"let a = 5; let b = a; let c = a + b + 5; c;", 15},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestErrorHandling(t *testing.T) {
	tests := []struct {
		input           string
		expectedMessage string
	}{
		{
			"5 + true;",
			"type mismatch: INTEGER + BOOLEAN",
		},
		{
			"true - false;",
			"unknown operator: -",
		},
		{
			"foobar",
			"identifier not found: foobar",
		},
		{
			"if (10 > 1) { true + false; }",
			"unknown operator: +",
		},
		{
			"5 + true; 5;",
			"type mismatch: INTEGER + BOOLEAN",
		},
		{
			"-true",
			"unknown operator: -SOME",
		},
		{
			"5 + true + 10;",
			"type mismatch: INTEGER + BOOLEAN",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)

		errObj, ok := evaluated.(*object.Error)
		if !ok {
			t.Errorf("no error object returned. got=%T(%+v)", evaluated, evaluated)
			continue
		}

		if errObj.Message != tt.expectedMessage {
			t.Errorf("wrong error message. expected=%q, got=%q",
				tt.expectedMessage, errObj.Message)
		}
	}
}

func testEval(input string) object.Object {
	l := lexer.New(input)
	p := parser.New(l)
	program := p.ParseProgram()
	env := object.NewEnvironment()
	evaluated := Eval(program, env)

	// Unwrap return values to get the actual object for testing
	if returnValue, ok := evaluated.(*object.ReturnValue); ok {
		return returnValue.Value
	}

	return evaluated
}

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

func testLiteralObject(t *testing.T, obj object.Object, expected interface{}) bool {
	switch v := expected.(type) {
	case int:
		return testIntegerObject(t, obj, int64(v))
	case int64:
		return testIntegerObject(t, obj, v)
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

func testIntegerObject(t *testing.T, obj object.Object, expected int64) bool {
	result, ok := obj.(*object.Integer)
	if !ok {
		t.Errorf("object is not Integer. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%d, want=%d",
			result.Value, expected)
		return false
	}
	return true
}

func testBooleanObject(t *testing.T, obj object.Object, expected bool) bool {
	result, ok := obj.(*object.Boolean)
	if !ok {
		t.Errorf("object is not Boolean. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%t, want=%t",
			result.Value, expected)
		return false
	}
	return true
}

func testStringObject(t *testing.T, obj object.Object, expected string) bool {
	result, ok := obj.(*object.String)
	if !ok {
		t.Errorf("object is not String. got=%T (%+v)", obj, obj)
		return false
	}
	if result.Value != expected {
		t.Errorf("object has wrong value. got=%q, want=%q",
			result.Value, expected)
		return false
	}
	return true
}

func TestFunctionObject(t *testing.T) {
	input := "fn(x) { x + 2; };"

	evaluated := testEval(input)
	fn, ok := evaluated.(*object.Function)
	if !ok {
		t.Fatalf("object is not Function. got=%T (%+v)", evaluated, evaluated)
	}

	if len(fn.Parameters) != 1 {
		t.Fatalf("function has wrong parameters. Parameters=%+v", fn.Parameters)
	}

	if fn.Parameters[0].String() != "x" {
		t.Fatalf("parameter is not 'x'. got=%q", fn.Parameters[0])
	}

	expectedBody := "{(x + 2)}"

	if fn.Body.String() != expectedBody {
		t.Fatalf("body is not %q. got=%q", expectedBody, fn.Body.String())
	}
}

func TestFunctionApplication(t *testing.T) {
	tests := []struct {
		input    string
		expected int64
	}{
		{"let identity = fn(x) { x; }; identity(5);", 5},
		{"let double = fn(x) { x * 2; }; double(5);", 10},
		{"let add = fn(x, y) { x + y; }; add(5, 5);", 10},
		{"fn(x) { x; }(5)", 5},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestClosures(t *testing.T) {
	input := `
let newAdder = fn(x) {
  fn(y) { x + y };
};
let addTwo = newAdder(2);
addTwo(3);
`

	evaluated := testEval(input)
	testOptionObject(t, evaluated, 5)
}

func TestEvalIfElseExpressions(t *testing.T) {
	tests := []struct {
		input    string
		expected interface{}
	}{
		{"if 10 > 1 { 10 }", 10},
		{"if 1 > 10 { 10 }", nil},
		{"if 1 > 10 { 10 } else { 20 }", 20},
		{"if 1 < 10 { 10 } else { 20 }", 10},
		{"if false { 10 } else { 20 }", 20},
		{"if true { 10 }", 10},
		{"if 0 { 10 } else { 20 }", 10}, // 0 is truthy in our language
		{"if 1 { 10 }", 10},
		{"if true { 10 } else { 20 }", 10},
		{"if false { 10 }", nil},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestImportStatements(t *testing.T) {
	// Test Case 1: Standard Import
	input1 := "import std::io; io;"
	evaluated1 := testEval(input1)

	module1, ok := evaluated1.(*object.Module)
	if !ok {
		t.Fatalf("object is not Module. got=%T (%+v)", evaluated1, evaluated1)
	}

	// Verify the module has the expected environment
	if module1.Env == nil {
		t.Fatalf("module environment is nil")
	}

	// Test Case 2: Aliased Import
	input2 := "import std::io as console; console;"
	evaluated2 := testEval(input2)

	module2, ok := evaluated2.(*object.Module)
	if !ok {
		t.Fatalf("object is not Module. got=%T (%+v)", evaluated2, evaluated2)
	}

	// Verify both modules are the same (same reference from registry)
	if module1 != module2 {
		t.Fatalf("aliased import should return the same module")
	}

	// Test Case 3: Import Error
	input3 := "import std::nonexistent;"
	evaluated3 := testEval(input3)

	errObj, ok := evaluated3.(*object.Error)
	if !ok {
		t.Fatalf("object is not Error. got=%T (%+v)", evaluated3, evaluated3)
	}

	expectedMessage := "module not found: std::nonexistent"
	if errObj.Message != expectedMessage {
		t.Fatalf("wrong error message. expected=%q, got=%q", expectedMessage, errObj.Message)
	}
}

func TestImportModuleBasicAccess(t *testing.T) {
	// Test importing a module and accessing it
	input := `
import std::io;
io;
`

	evaluated := testEval(input)

	// We should get a Module object
	module, ok := evaluated.(*object.Module)
	if !ok {
		t.Fatalf("object is not Module. got=%T (%+v)", evaluated, evaluated)
	}

	// Verify the module has an environment with the puts function
	putsObj, ok := module.Env.Get("puts")
	if !ok {
		t.Fatalf("puts function not found in module environment")
	}

	// Verify puts is a builtin function
	builtin, ok := putsObj.(*object.Builtin)
	if !ok {
		t.Fatalf("puts is not a Builtin. got=%T (%+v)", putsObj, putsObj)
	}

	// Verify it's actually a function
	if builtin.Fn == nil {
		t.Fatalf("builtin function is nil")
	}
}

func TestMemberAccessExpressions(t *testing.T) {
	tests := []struct {
		input        string
		expected     interface{}
		expectError  bool
		errorMessage string
	}{
		// Test Case 1: Successful Access
		{
			input:    "import std::io; io.VERSION",
			expected: 1,
		},
		// Test Case 2: Member Not Found
		{
			input:        "import std::io; io.nonexistent",
			expectError:  true,
			errorMessage: "undefined member 'nonexistent' on module",
		},
		// Test Case 3: Invalid Access on non-module
		{
			input:        "1.foo",
			expectError:  true,
			errorMessage: "member access not supported on type SOME",
		},
	}

	for i, tt := range tests {
		evaluated := testEval(tt.input)

		if tt.expectError {
			errObj, ok := evaluated.(*object.Error)
			if !ok {
				t.Errorf("test %d: expected Error object. got=%T (%+v)", i, evaluated, evaluated)
				continue
			}
			if errObj.Message != tt.errorMessage {
				t.Errorf("test %d: wrong error message. expected=%q, got=%q", i, tt.errorMessage, errObj.Message)
			}
		} else {
			// For successful access, we expect an Integer
			intObj, ok := evaluated.(*object.Integer)
			if !ok {
				t.Errorf("test %d: expected Integer object. got=%T (%+v)", i, evaluated, evaluated)
				continue
			}
			expectedInt, ok := tt.expected.(int)
			if !ok {
				t.Errorf("test %d: expected value should be int. got=%T", i, tt.expected)
				continue
			}
			if intObj.Value != int64(expectedInt) {
				t.Errorf("test %d: wrong integer value. expected=%d, got=%d", i, expectedInt, intObj.Value)
			}
		}
	}
}

func TestStringLiteralExpression(t *testing.T) {
	input := `"hello world!"`

	evaluated := testEval(input)
	testOptionObject(t, evaluated, "hello world!")
}

func TestStringConcatenation(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{`"Hello" + " " + "World!"`, "Hello World!"},
		{`"foo" + "bar"`, "foobar"},
		{`"" + "test"`, "test"},
		{`"test" + ""`, "test"},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestArrayLiterals(t *testing.T) {
	input := "[1, 2 * 2, 3 + 3]"

	evaluated := testEval(input)
	testOptionObject(t, evaluated, []int64{1, 4, 6})
}

func TestArrayIndexExpressions(t *testing.T) {
	tests := []struct {
		input    string
		expected interface{}
	}{
		{
			"[1, 2, 3][0]",
			1,
		},
		{
			"[1, 2, 3][1]",
			2,
		},
		{
			"[1, 2, 3][2]",
			3,
		},
		{
			"let i = 0; [1][i];",
			1,
		},
		{
			"[1, 2, 3][1 + 1];",
			3,
		},
		{
			"let myArray = [1, 2, 3]; myArray[2];",
			3,
		},
		{
			"let myArray = [1, 2, 3]; myArray[0] + myArray[1] + myArray[2];",
			6,
		},
		{
			"let myArray = [1, 2, 3]; let i = myArray[0]; myArray[i]",
			2,
		},
		{
			"[1, 2, 3][3]",
			"index out of bounds",
		},
		{
			"[1, 2, 3][-1]",
			"index out of bounds",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		switch expected := tt.expected.(type) {
		case int:
			testOptionObject(t, evaluated, int64(expected))
		case string:
			errObj, ok := evaluated.(*object.Error)
			if !ok {
				t.Errorf("object is not Error. got=%T (%+v)", evaluated, evaluated)
				continue
			}
			if errObj.Message != expected {
				t.Errorf("wrong error message. expected=%q, got=%q", expected, errObj.Message)
			}
		}
	}
}

func TestArrayTypeEnforcement(t *testing.T) {
	tests := []struct {
		input           string
		expectedMessage string
	}{
		{
			"[1, \"hello\"]",
			"array elements must be of the same type",
		},
		{
			"[true, 42]",
			"array elements must be of the same type",
		},
		{
			"[1, 2, \"three\"]",
			"array elements must be of the same type",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)

		errObj, ok := evaluated.(*object.Error)
		if !ok {
			t.Errorf("object is not Error. got=%T (%+v)", evaluated, evaluated)
			continue
		}

		if errObj.Message != tt.expectedMessage {
			t.Errorf("wrong error message. expected=%q, got=%q",
				tt.expectedMessage, errObj.Message)
		}
	}
}

func TestArrayMethodAccess(t *testing.T) {
	tests := []struct {
		input    string
		expected interface{}
	}{
		{
			"[1, 2, 3, 4].len()",
			4,
		},
		{
			"[].len()",
			0,
		},
		{
			"let arr = [1, 2, 3]; arr.len()",
			3,
		},
		{
			"[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].len()",
			10,
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		switch expected := tt.expected.(type) {
		case int:
			testOptionObject(t, evaluated, int64(expected))
		}
	}
}

func TestStringMethodAccess(t *testing.T) {
	tests := []struct {
		input    string
		expected interface{}
	}{
		{
			"\"hello world\".len()",
			11,
		},
		{
			"\"\".len()",
			0,
		},
		{
			"let str = \"test\"; str.len()",
			4,
		},
		{
			"\"hello\".len() + \"world\".len()",
			10,
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		switch expected := tt.expected.(type) {
		case int:
			testOptionObject(t, evaluated, int64(expected))
		}
	}
}

func TestMethodAccessErrors(t *testing.T) {
	tests := []struct {
		input           string
		expectedMessage string
	}{
		{
			"[1, 2, 3].foo()",
			"undefined method 'foo' on array",
		},
		{
			"\"hello\".bar()",
			"undefined method 'bar' on string",
		},
		{
			"42.len()",
			"member access not supported on type SOME",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)

		errObj, ok := evaluated.(*object.Error)
		if !ok {
			t.Errorf("object is not Error. got=%T (%+v)", evaluated, evaluated)
			continue
		}

		if errObj.Message != tt.expectedMessage {
			t.Errorf("wrong error message. expected=%q, got=%q",
				tt.expectedMessage, errObj.Message)
		}
	}
}

func testArrayObject(t *testing.T, obj object.Object, expected []int64) bool {
	result, ok := obj.(*object.Some)
	if !ok {
		t.Errorf("object is not Some. got=%T (%+v)", obj, obj)
		return false
	}

	array, ok := result.Value.(*object.Array)
	if !ok {
		t.Errorf("object is not Array. got=%T (%+v)", result.Value, result.Value)
		return false
	}

	if len(array.Elements) != len(expected) {
		t.Errorf("wrong num of elements. want=%d, got=%d",
			len(expected), len(array.Elements))
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

func TestArraySlicing(t *testing.T) {
	tests := []struct {
		input    string
		expected []int64
	}{
		{
			"[1, 2, 3, 4][1:3]",
			[]int64{2, 3},
		},
		{
			"[1, 2, 3, 4][:2]",
			[]int64{1, 2},
		},
		{
			"[1, 2, 3, 4][2:]",
			[]int64{3, 4},
		},
		{
			"[1, 2, 3, 4][:]",
			[]int64{1, 2, 3, 4},
		},
		{
			"[1, 2, 3, 4][1:1]",
			[]int64{},
		},
		{
			"let arr = [1, 2, 3, 4]; arr[1:3]",
			[]int64{2, 3},
		},
		{
			"[1, 2, 3, 4, 5][1:4]",
			[]int64{2, 3, 4},
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testSliceObject(t, evaluated, tt.expected)
	}
}

func TestStringSlicing(t *testing.T) {
	tests := []struct {
		input    string
		expected string
	}{
		{
			"\"hello\"[1:4]",
			"ell",
		},
		{
			"\"hello\"[:3]",
			"hel",
		},
		{
			"\"hello\"[2:]",
			"llo",
		},
		{
			"\"hello\"[:]",
			"hello",
		},
		{
			"\"hello\"[1:1]",
			"",
		},
		{
			"let str = \"world\"; str[1:4]",
			"orl",
		},
		{
			"\"testing\"[0:4]",
			"test",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		testOptionObject(t, evaluated, tt.expected)
	}
}

func TestSlicingErrors(t *testing.T) {
	tests := []struct {
		input           string
		expectedMessage string
	}{
		{
			"[1, 2, 3][5:10]",
			"start index out of bounds",
		},
		{
			"[1, 2, 3][-1:2]",
			"start index out of bounds",
		},
		{
			"[1, 2, 3][1:10]",
			"end index out of bounds",
		},
		{
			"[1, 2, 3][1:-1]",
			"end index out of bounds",
		},
		{
			"\"hello\"[10:15]",
			"start index out of bounds",
		},
		{
			"\"hello\"[-1:3]",
			"start index out of bounds",
		},
		{
			"\"hello\"[1:10]",
			"end index out of bounds",
		},
		{
			"\"hello\"[1:-1]",
			"end index out of bounds",
		},
		{
			"42[1:3]",
			"slice operator not supported: INTEGER",
		},
		{
			"[1, 2, 3][\"a\":2]",
			"start index must be an integer",
		},
		{
			"[1, 2, 3][1:\"b\"]",
			"end index must be an integer",
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)

		errObj, ok := evaluated.(*object.Error)
		if !ok {
			t.Errorf("object is not Error. got=%T (%+v)", evaluated, evaluated)
			continue
		}

		if errObj.Message != tt.expectedMessage {
			t.Errorf("wrong error message. expected=%q, got=%q",
				tt.expectedMessage, errObj.Message)
		}
	}
}

func TestSliceMethodAccess(t *testing.T) {
	tests := []struct {
		input    string
		expected interface{}
	}{
		{
			"[1, 2, 3, 4][1:3].len()",
			2,
		},
		{
			"[1, 2, 3, 4][:2].len()",
			2,
		},
		{
			"[1, 2, 3, 4][2:].len()",
			2,
		},
		{
			"[1, 2, 3, 4][1:1].len()",
			0,
		},
		{
			"let arr = [1, 2, 3, 4, 5]; arr[1:4].len()",
			3,
		},
	}

	for _, tt := range tests {
		evaluated := testEval(tt.input)
		switch expected := tt.expected.(type) {
		case int:
			testOptionObject(t, evaluated, int64(expected))
		}
	}
}

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
		t.Errorf("wrong num of elements. want=%d, got=%d",
			len(expected), len(slice.Elements))
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
