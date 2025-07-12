package parser

import (
	"testing"

	"github.com/behzade/basalt/lexer"
)

func TestEnumLiteral(t *testing.T) {
	input := `let Option = enum {
    Some(int64),
    None,
};`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	if len(program.Statements) != 1 {
		t.Fatalf("Expected 1 statement, got %d", len(program.Statements))
	}

	expected := "let Option = enum { Some(int64), None };"
	actual := program.String()
	if actual != expected {
		t.Errorf("Expected: %s, got: %s", expected, actual)
	}
}

func TestEnumInstantiation(t *testing.T) {
	input := `let some_val = Option::Some(42);`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	if len(program.Statements) != 1 {
		t.Fatalf("Expected 1 statement, got %d", len(program.Statements))
	}

	expected := "let some_val = Option::Some(42);"
	actual := program.String()
	if actual != expected {
		t.Errorf("Expected: %s, got: %s", expected, actual)
	}
}

func TestEnumInstantiationWithoutPayload(t *testing.T) {
	input := `let none_val = Option::None;`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	if len(program.Statements) != 1 {
		t.Fatalf("Expected 1 statement, got %d", len(program.Statements))
	}

	expected := "let none_val = Option::None;"
	actual := program.String()
	if actual != expected {
		t.Errorf("Expected: %s, got: %s", expected, actual)
	}
}

func TestMatchExpression(t *testing.T) {
	input := `let result = match value {
    Option::Some(x) => x + 1,
    Option::None => 0,
};`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	if len(program.Statements) != 1 {
		t.Fatalf("Expected 1 statement, got %d", len(program.Statements))
	}

	expected := "let result = match value { Option::Some(x) => (x + 1), Option::None => 0 };"
	actual := program.String()
	if actual != expected {
		t.Errorf("Expected: %s, got: %s", expected, actual)
	}
}

func TestMatchExpressionSingleArm(t *testing.T) {
	input := `let result = match value {
    Option::Some(x) => x,
};`

	l := lexer.New(input)
	p := New(l)
	program := p.ParseProgram()

	if len(p.Errors()) > 0 {
		t.Fatalf("Parser errors: %v", p.Errors())
	}

	if len(program.Statements) != 1 {
		t.Fatalf("Expected 1 statement, got %d", len(program.Statements))
	}

	expected := "let result = match value { Option::Some(x) => x };"
	actual := program.String()
	if actual != expected {
		t.Errorf("Expected: %s, got: %s", expected, actual)
	}
}
