package main

import (
	"fmt"
	"io"
	"os"

	"github.com/behzade/basalt/checker"
	"github.com/behzade/basalt/compiler"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/parser"
)

func main() {
	runtimeBytes, err := os.ReadFile("stdlib/runtime.bst") // Adjust path if needed
	if err != nil {
		fmt.Fprintf(os.Stderr, "Error reading runtime file: %v\n", err)
		os.Exit(1)
	}
	runtimeInput := string(runtimeBytes)

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

	input = runtimeInput + "\n" + input

	outputPath := "./output"

	// Check for --compile flag
	for i := range os.Args {
		if i+1 < len(os.Args) && os.Args[i+1][:1] != "-" {
			outputPath = os.Args[i+1]
		}
	}

	err = compileBasalt(input, outputPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Compilation error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Successfully compiled to %s\n", outputPath)
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
