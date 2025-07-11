package main

import (
	"fmt"
	"io"
	"os"

	"github.com/behzade/basalt/checker"
	"github.com/behzade/basalt/compiler"
	"github.com/behzade/basalt/evaluator"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/object"
	"github.com/behzade/basalt/parser"
)

func main() {
	var input string

	if len(os.Args) > 1 {
		// Read from file
		filePath := os.Args[1]
		inputBytes, err := os.ReadFile(filePath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading file %s: %v\n", filePath, err)
			os.Exit(1)
		}
		input = string(inputBytes)
	} else {
		// Read from stdin
		inputBytes, err := io.ReadAll(os.Stdin)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading from stdin: %v\n", err)
			os.Exit(1)
		}
		input = string(inputBytes)
	}

	// Check if we should compile or interpret
	// For now, let's use a simple heuristic: if there's a --compile flag, compile
	shouldCompile := false
	outputPath := "./output"

	// Check for --compile flag
	for i, arg := range os.Args {
		if arg == "--compile" {
			shouldCompile = true
			// Check if output path is specified
			if i+1 < len(os.Args) && os.Args[i+1][:1] != "-" {
				outputPath = os.Args[i+1]
			}
			break
		}
	}

	if shouldCompile {
		// Compile mode
		err := compileBasalt(input, outputPath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Compilation error: %v\n", err)
			os.Exit(1)
		}
		fmt.Printf("Successfully compiled to %s\n", outputPath)
	} else {
		// Interpret mode (existing behavior)
		result, err := runBasalt(input)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}

		// Print the result if it's not None
		if result != nil && result.Type() != object.NONE_OBJ {
			fmt.Println(result.Inspect())
		}
	}
}

func compileBasalt(input string, outputPath string) error {
	// Create lexer
	l := lexer.New(input)

	// Create parser
	p := parser.New(l)

	// Parse the program
	program := p.ParseProgram()

	// Check for parser errors
	if len(p.Errors()) > 0 {
		return fmt.Errorf("parser errors: %v", p.Errors())
	}

	// Create type checker and perform type checking
	typeChecker := checker.New()
	typeChecker.Check(program)

	// Check for type errors
	if len(typeChecker.Errors()) > 0 {
		errorMessages := make([]string, len(typeChecker.Errors()))
		for i, err := range typeChecker.Errors() {
			errorMessages[i] = err.Error()
		}
		return fmt.Errorf("type errors: %v", errorMessages)
	}

	// Create compiler and compile to executable
	comp := compiler.New()
	return comp.CompileToExecutable(program, outputPath)
}

func runBasalt(input string) (object.Object, error) {
	// Create lexer
	l := lexer.New(input)

	// Create parser
	p := parser.New(l)

	// Parse the program
	program := p.ParseProgram()

	// Check for parser errors
	if len(p.Errors()) > 0 {
		return nil, fmt.Errorf("parser errors: %v", p.Errors())
	}

	// Create type checker and perform type checking
	typeChecker := checker.New()
	typeChecker.Check(program)

	// Check for type errors
	if len(typeChecker.Errors()) > 0 {
		errorMessages := make([]string, len(typeChecker.Errors()))
		for i, err := range typeChecker.Errors() {
			errorMessages[i] = err.Error()
		}
		return nil, fmt.Errorf("type errors: %v", errorMessages)
	}

	// Create environment and set up built-ins
	env := object.NewEnvironment()
	evaluator.SetupBuiltins(env)

	// Evaluate the program
	result := evaluator.Eval(program, env)

	// Check for evaluation errors
	if result.Type() == object.ERROR_OBJ {
		return nil, fmt.Errorf("evaluation error: %s", result.Inspect())
	}

	return result, nil
}
