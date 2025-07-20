package main

import (
	"fmt"
	"io"
	"log/slog"
	"os"

	"github.com/behzade/basalt/checker"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/parser"
)

func main() {
	var input string

	if len(os.Args) > 1 {
		slog.Debug("reading from file")
		filePath := os.Args[1]
		inputBytes, err := os.ReadFile(filePath)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading file %s: %v\n", filePath, err)
			os.Exit(1)
		}
		input = string(inputBytes)
	} else {
		slog.Debug("reading from stdin")
		inputBytes, err := io.ReadAll(os.Stdin)
		if err != nil {
			fmt.Fprintf(os.Stderr, "Error reading from stdin: %v\n", err)
			os.Exit(1)
		}
		input = string(inputBytes)
	}

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
			fmt.Printf("%v:%v %s\n", err.Line, err.Col, err.Msg)
		}
	}

	c := checker.New()
	_ = c.Check(program)
	for _, err := range c.Errors() {
		fmt.Printf("%v:%v %s\n", err.Token.Line, err.Token.Column, err.Message)
	}
}
