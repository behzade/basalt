package parser

import (
	"testing"

	"github.com/behzade/zerolang/compiler/ast"
	"github.com/behzade/zerolang/compiler/lexer"
)

func TestParsingStringLiteralExpression(t *testing.T) {
	input := `"hello world";`

	l := lexer.New(input)
	p := New(l)

	program := p.ParseProgram()
	checkParserErrors(t, p)

	if len(program.Statements) != 1 {
		t.Fatalf("program has not enough statements. got=%d",
			len(program.Statements))
	}

	stmt, ok := program.Statements[0].(*ast.ExpressionStatement)
	if !ok {
		t.Fatalf("program.Statements[0] is not ast.ExpressionStatement. got=%T",
			program.Statements[0])
	}

	literal, ok := stmt.Expression.(*ast.StringLiteral)
	if !ok {
		t.Fatalf("exp not *ast.StringLiteral. got=%T", stmt.Expression)
	}

	if literal.Value != "hello world" {
		t.Errorf("literal.Value not %q. got=%q", "hello world", literal.Value)
	}
}

func TestParsingArrayLiterals(t *testing.T) {
	input := "[1, 2, 3]"

	l := lexer.New(input)
	p := New(l)

	program := p.ParseProgram()
	checkParserErrors(t, p)

	stmt, ok := program.Statements[0].(*ast.ExpressionStatement)
	if !ok {
		t.Fatalf("program.Statements[0] is not ast.ExpressionStatement. got=%T",
			program.Statements[0])
	}

	array, ok := stmt.Expression.(*ast.ArrayLiteral)
	if !ok {
		t.Fatalf("exp not ast.ArrayLiteral. got=%T", stmt.Expression)
	}

	if len(array.Elements) != 3 {
		t.Fatalf("len(array.Elements) not 3. got=%d", len(array.Elements))
	}

	// Test the elements of the array
	// For simplicity, we'll just check the literal values for now
	// More robust testing would involve a helper function like testIntegerLiteral
	if array.Elements[0].TokenLiteral() != "1" {
		t.Errorf("array.Elements[0] not 1. got=%s", array.Elements[0].TokenLiteral())
	}
	if array.Elements[1].TokenLiteral() != "2" {
		t.Errorf("array.Elements[1] not 2. got=%s", array.Elements[1].TokenLiteral())
	}
	if array.Elements[2].TokenLiteral() != "3" {
		t.Errorf("array.Elements[2] not 3. got=%s", array.Elements[2].TokenLiteral())
	}
}

func TestParsingHashLiterals(t *testing.T) {
	input := `{"one": 1, "two": 2, "three": 3}`

	l := lexer.New(input)
	p := New(l)

	program := p.ParseProgram()
	checkParserErrors(t, p)

	stmt := program.Statements[0].(*ast.ExpressionStatement)
	hash, ok := stmt.Expression.(*ast.HashLiteral)
	if !ok {
		t.Fatalf("exp not ast.HashLiteral. got=%T", stmt.Expression)
	}

	if len(hash.Pairs) != 3 {
		t.Errorf("hash.Pairs has wrong num of pairs. got=%d", len(hash.Pairs))
	}

	expected := map[string]int64{
		"one":   1,
		"two":   2,
		"three": 3,
	}

	for key, value := range hash.Pairs {
		literal, ok := key.(*ast.StringLiteral)
		if !ok {
			t.Errorf("key is not ast.StringLiteral. got=%T", key)
			continue
		}
		expectedValue := expected[literal.Value]

		integer, ok := value.(*ast.IntegerLiteral)
		if !ok {
			t.Errorf("value is not ast.IntegerLiteral. got=%T", value)
			continue
		}

		if integer.Value != expectedValue {
			t.Errorf("value not %d. got=%d", expectedValue, integer.Value)
		}
	}
}

func TestParsingIndexExpressions(t *testing.T) {
	input := `myArray[1 + 1]`

	l := lexer.New(input)
	p := New(l)

	program := p.ParseProgram()
	checkParserErrors(t, p)

	stmt, ok := program.Statements[0].(*ast.ExpressionStatement)
	if !ok {
		t.Fatalf("Expected ExpressionStatement. Got %T", program.Statements[0])
	}

	indexExp, ok := stmt.Expression.(*ast.IndexExpression)
	if !ok {
		t.Fatalf("Expected IndexExpression. Got %T", stmt.Expression)
	}

	if indexExp.Left.TokenLiteral() != "myArray" {
		t.Errorf("Expected Left to be 'myArray'. Got %s", indexExp.Left.TokenLiteral())
	}

	if indexExp.Index.String() != "(1 + 1)" {
		t.Errorf("Expected Index to be '(1 + 1)'. Got %s", indexExp.Index.String())
	}
}

func checkParserErrors(t *testing.T, p *Parser) {
	errors := p.Errors()
	if len(errors) == 0 {
		return
	}

	t.Errorf("parser has %d errors", len(errors))
	for _, msg := range errors {
		t.Errorf("parser error: %q", msg)
	}
	t.FailNow()
}
