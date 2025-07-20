package parser

import (
	"testing"

	"github.com/behzade/basalt/lexer"
)

func TestGenericTypeAnnotations(t *testing.T) {
	tests := []struct {
		name     string
		input    string
		expected string
	}{
		{
			name:     "Simple Hash Map Declaration",
			input:    "let scores: HashMap<string, int64>;",
			expected: "let scores: HashMap<string, int64>;",
		},
		{
			name:     "Nested Generic Declaration",
			input:    "let data: Result<List<int64>, string>;",
			expected: "let data: Result<List<int64>, string>;",
		},
		{
			name:     "Hash Map Literal (No generics in value)",
			input:    `let scores = {"math": 100};`,
			expected: `let scores = {"math": 100};`,
		},
		{
			name:     "Generic with pointer type",
			input:    "let ptr: HashMap<string, *int64>;",
			expected: "let ptr: HashMap<string, *int64>;",
		},
		{
			name:     "Pointer to generic type",
			input:    "let ptr: *HashMap<string, int64>;",
			expected: "let ptr: *HashMap<string, int64>;",
		},
		{
			name:     "Multiple generic parameters",
			input:    "let complex: MyType<string, int64, bool>;",
			expected: "let complex: MyType<string, int64, bool>;",
		},
		{
			name:     "Deeply nested generics",
			input:    "let nested: HashMap<string, Result<List<int64>, Error>>;",
			expected: "let nested: HashMap<string, Result<List<int64>, Error>>;",
		},
		{
			name:     "Generic with module path",
			input:    "let map: std::HashMap<string, int64>;",
			expected: "let map: std::HashMap<string, int64>;",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			l := lexer.New(tt.input)
			p := New(l)
			program := p.ParseProgram()

			if len(p.Errors()) > 0 {
				t.Fatalf("Parser errors: %v", p.Errors())
			}

			actual := program.String()
			if actual != tt.expected {
				t.Errorf("Expected AST: %s, got: %s", tt.expected, actual)
			}
		})
	}
}

func TestGenericTypeAnnotationErrors(t *testing.T) {
	tests := []struct {
		name  string
		input string
	}{
		{
			name:  "Unclosed generic parameters",
			input: "let scores: HashMap<string, int64;",
		},
		{
			name:  "Empty generic parameters",
			input: "let scores: HashMap<>;",
		},
		{
			name:  "Missing comma in generic parameters",
			input: "let scores: HashMap<string int64>;",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			l := lexer.New(tt.input)
			p := New(l)
			program := p.ParseProgram()

			// These should produce parser errors
			if len(p.Errors()) == 0 {
				t.Errorf("Expected parser errors for input: %s, but got none. AST: %s", tt.input, program.String())
			}
		})
	}
}
