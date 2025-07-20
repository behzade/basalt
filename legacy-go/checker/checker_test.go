package checker

import (
	"testing"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/parser"
)

// Helper function to parse and check a program
func parseAndCheck(input string) (*Checker, *ast.Program) {
	l := lexer.New(input)
	p := parser.New(l)
	program := p.ParseProgram()
	checker := New()
	checker.Check(program)
	return checker, program
}

// Test Case 1: Valid Literal Inference
func TestValidHashMapLiteralInference(t *testing.T) {
	input := `let scores = {"math": 95, "history": 88};`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) != 0 {
		t.Errorf("Expected no errors, got %d", len(checker.Errors()))
		for _, err := range checker.Errors() {
			t.Errorf("Error: %s", err.Message)
		}
	}

	// Check that the variable has the correct type
	scoresType, ok := checker.env.Get("scores")
	if !ok {
		t.Error("Variable 'scores' not found in environment")
		return
	}

	hashMapType, ok := scoresType.(*HashMapType)
	if !ok {
		t.Errorf("Expected HashMapType, got %T", scoresType)
		return
	}

	if !hashMapType.KeyType.Equals(&StringType{}) {
		t.Errorf("Expected key type string, got %s", hashMapType.KeyType.String())
	}

	if !hashMapType.ValueType.Equals(&IntegerType{}) {
		t.Errorf("Expected value type int64, got %s", hashMapType.ValueType.String())
	}
}

// Test Case 2: Invalid Literal - Inconsistent Types
func TestInvalidHashMapLiteralInconsistentTypes(t *testing.T) {
	input := `let scores = {"math": 95, 88: "history"};`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) == 0 {
		t.Error("Expected type errors for inconsistent key/value types")
		return
	}

	// Should have errors for both key and value type mismatches
	hasKeyError := false
	hasValueError := false

	for _, err := range checker.Errors() {
		if containsString(err.Message, "hash key") && containsString(err.Message, "expected string") {
			hasKeyError = true
		}
		if containsString(err.Message, "hash value") && containsString(err.Message, "expected int64") {
			hasValueError = true
		}
	}

	if !hasKeyError {
		t.Error("Expected error about inconsistent key type")
	}
	if !hasValueError {
		t.Error("Expected error about inconsistent value type")
	}
}

// Test Case 3: Valid Access and Assignment
func TestValidHashMapAccessAndAssignment(t *testing.T) {
	input := `
	let mut scores: HashMap<string, int64> = {};
	scores["math"] = 100;
	let score = scores["math"];
	`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) != 0 {
		t.Errorf("Expected no errors, got %d", len(checker.Errors()))
		for _, err := range checker.Errors() {
			t.Errorf("Error: %s", err.Message)
		}
	}

	// Check that the score variable has the correct type
	scoreType, ok := checker.env.Get("score")
	if !ok {
		t.Error("Variable 'score' not found in environment")
		return
	}

	if !scoreType.Equals(&IntegerType{}) {
		t.Errorf("Expected score type int64, got %s", scoreType.String())
	}
}

// Test Case 4: Invalid Access/Assignment - Wrong Key Type
func TestInvalidHashMapAccessWrongKeyType(t *testing.T) {
	input := `
	let mut scores: HashMap<string, int64> = {};
	scores[123] = 100;
	`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) == 0 {
		t.Error("Expected type error for wrong key type")
		return
	}

	hasKeyError := false
	for _, err := range checker.Errors() {
		if containsString(err.Message, "hash map key must be string") && containsString(err.Message, "got int64") {
			hasKeyError = true
		}
	}

	if !hasKeyError {
		t.Error("Expected error about wrong key type in assignment")
	}
}

// Test Case 5: Empty Hash Map Type Resolution
func TestEmptyHashMapTypeResolution(t *testing.T) {
	input := `let mut scores: HashMap<string, int64> = {};`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) != 0 {
		t.Errorf("Expected no errors, got %d", len(checker.Errors()))
		for _, err := range checker.Errors() {
			t.Errorf("Error: %s", err.Message)
		}
	}

	// Check that the variable has the correct type
	scoresType, ok := checker.env.Get("scores")
	if !ok {
		t.Error("Variable 'scores' not found in environment")
		return
	}

	hashMapType, ok := scoresType.(*HashMapType)
	if !ok {
		t.Errorf("Expected HashMapType, got %T", scoresType)
		return
	}

	if !hashMapType.KeyType.Equals(&StringType{}) {
		t.Errorf("Expected key type string, got %s", hashMapType.KeyType.String())
	}

	if !hashMapType.ValueType.Equals(&IntegerType{}) {
		t.Errorf("Expected value type int64, got %s", hashMapType.ValueType.String())
	}
}

// Test Case 6: HashMap Type Annotation Error
func TestHashMapTypeAnnotationError(t *testing.T) {
	input := `let scores: HashMap = {};`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) == 0 {
		t.Error("Expected type error for HashMap without generic parameters")
		return
	}

	hasGenericError := false
	for _, err := range checker.Errors() {
		if containsString(err.Message, "HashMap requires generic parameters") {
			hasGenericError = true
		}
	}

	if !hasGenericError {
		t.Error("Expected error about HashMap requiring generic parameters")
	}
}

// Test Case 7: Invalid Value Assignment
func TestInvalidValueAssignment(t *testing.T) {
	input := `
	let mut scores: HashMap<string, int64> = {};
	scores["math"] = "not_a_number";
	`

	checker, _ := parseAndCheck(input)

	if len(checker.Errors()) == 0 {
		t.Error("Expected type error for wrong value type")
		return
	}

	hasValueError := false
	for _, err := range checker.Errors() {
		if containsString(err.Message, "cannot assign string to hash map value of type int64") {
			hasValueError = true
		}
	}

	if !hasValueError {
		t.Error("Expected error about wrong value type in assignment")
	}
}

// Helper function to check if a string contains a substring
func containsString(s, substr string) bool {
	return len(s) >= len(substr) && s[:len(substr)] == substr ||
		(len(s) > len(substr) && containsString(s[1:], substr))
}
