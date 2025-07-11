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
		// This case is not used yet but is here for completeness
		t.Errorf("string literal testing not implemented yet")
		return false
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
