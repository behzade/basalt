package main

import (
	"fmt"
	"io"
	"os"
	"os/exec"
	"strings"

	"github.com/behzade/basalt/checker"
	"github.com/behzade/basalt/compiler"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/module"
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

	outputPath := "./dist/output"

	err := compileBasalt(input, outputPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Compilation error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Successfully compiled to %s\n", outputPath)

	if len(os.Args) > 2 && os.Args[2] == "--run" {
		// Run the compiled program
		cmd := exec.Command(outputPath)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			fmt.Fprintf(os.Stderr, "Error running program: %v\n", err)
			os.Exit(1)
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
		fmt.Println("Parser errors:")
		for _, err := range p.Errors() {
			fmt.Printf("  %s %v:%v\n", err.Msg, err.Line, err.Col)
		}
		return fmt.Errorf("parser errors: %v", p.Errors())
	}

	err := module.Resolve(program)
	if err != nil {
		return fmt.Errorf("module resolution error: %v", err)
	}

	// Create type checker and perform type checking
	typeChecker := checker.New()
	typeChecker.Check(program)

	// Check for type errors
	if len(typeChecker.Errors()) > 0 {
		// Create a string slice to hold the error messages.
		errorMessages := make([]string, len(typeChecker.Errors()))

		// Iterate over the slice of TypeError pointers and format each error message.
		for i, err := range typeChecker.Errors() {
			// Format the error message to include the token's position and the error description.
			errorMessages[i] = fmt.Sprintf("Error at line %d, column %d: %s", err.Token.Line, err.Token.Column, err.Message)
		}

		// Join the error messages into a single string, separated by newlines, and return it as an error.
		return fmt.Errorf("type errors:\n%s", strings.Join(errorMessages, "\n"))
	}

	// Create compiler and compile to executable
	comp := compiler.New()
	return comp.CompileToExecutable(program, outputPath)
}
