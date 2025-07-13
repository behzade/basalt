package parser

import (
	"strings"
	"testing"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/lexer"
)

func TestAttributeParsing(t *testing.T) {
	input := `#[nogc]
let with_attributes = fn(x: int64) -> int64 {
    x + 1
};`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Errorf("Parser errors:")
		for _, err := range p.Errors() {
			t.Errorf("  Line %d, Col %d: %s", err.Line, err.Col, err.Msg)
		}
		return
	}

	if len(program.Statements) != 1 {
		t.Errorf("Expected 1 statement, got %d", len(program.Statements))
		return
	}

	letStmt, ok := program.Statements[0].(*ast.LetStatement)
	if !ok {
		t.Errorf("Expected LetStatement, got %T", program.Statements[0])
		return
	}

	fnLit, ok := letStmt.Value.(*ast.FunctionLiteral)
	if !ok {
		t.Errorf("Expected FunctionLiteral, got %T", letStmt.Value)
		return
	}

	// Test that the attribute is correctly stored
	if len(fnLit.Attributes) != 1 {
		t.Errorf("Expected 1 attribute, got %d", len(fnLit.Attributes))
		return
	}

	if fnLit.Attributes[0] != "nogc" {
		t.Errorf("Expected attribute 'nogc', got '%s'", fnLit.Attributes[0])
	}

	// Test the IsNoGC helper method
	if !fnLit.IsNoGC() {
		t.Errorf("Expected IsNoGC() to return true")
	}

	// Test that the attribute appears in the string representation
	if !strings.Contains(fnLit.String(), "#[nogc]") {
		t.Errorf("Attribute missing from string representation: %s", fnLit.String())
	}
}

func TestMultipleAttributes(t *testing.T) {
	input := `#[nogc] #[inline]
let with_attributes = fn(x: int64) -> int64 {
    x + 1
};`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	// Should have parser error for unsupported attribute
	if len(p.Errors()) == 0 {
		t.Errorf("Expected parser error for unsupported attribute 'inline'")
	}

	// But should still parse the nogc attribute
	letStmt := program.Statements[0].(*ast.LetStatement)
	fnLit := letStmt.Value.(*ast.FunctionLiteral)

	if !fnLit.IsNoGC() {
		t.Errorf("Expected IsNoGC() to return true even with error")
	}
}

func TestAttributeOnNonFunction(t *testing.T) {
	input := `#[nogc]
let x = 42;`

	l := lexer.New(input)
	p := New(l)
	_ = p.ParseProgram()

	// Should have parser error for attribute on non-function
	if len(p.Errors()) == 0 {
		t.Errorf("Expected parser error for attribute on non-function")
		return
	}

	// Check that the error message is correct
	found := false
	for _, err := range p.Errors() {
		if strings.Contains(err.Msg, "attributes can only be applied to a function definition") {
			found = true
			break
		}
	}
	if !found {
		t.Errorf("Expected error message about attributes only on functions")
	}
}
