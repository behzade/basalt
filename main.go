package main

import (
	"fmt"
	"io"
	"log/slog"
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

	if len(os.Args) < 2 {
		fmt.Printf("Usage: %s <action> <input file>\n", os.Args[0])
		os.Exit(1) // Exit early if not enough args
	}

	action := os.Args[1]
	switch action {
	case "run", "compile":
		slog.Debug("action is", "action", action)
	default:
		fmt.Printf("Invalid action: %s\n", action)
		os.Exit(1)
	}

	if len(os.Args) > 2 {
		slog.Debug("reading from file")
		filePath := os.Args[2]
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

	outputPath := "./dist/output"

	// Get linker flags from pkg-config for bdw-gc.
	// This makes finding the GC library portable across different systems (macOS, Linux).
	pkgConfigFlags, err := exec.Command("pkg-config", "--cflags", "--libs", "bdw-gc").Output()
	if err != nil {
		// If pkg-config fails, print a warning. The compilation might still succeed if
		// the linker can find the library through other means, but it's unlikely.
		fmt.Fprintf(os.Stderr, "Warning: could not run pkg-config for bdw-gc. Linking might fail. Error: %v\n", err)
	}
	// Clean and split the flags into a slice (e.g., ["-I/path", "-L/path", "-lgc"])
	linkerFlags := strings.Fields(string(pkgConfigFlags))

	// Pass the dynamically found flags to the compilation function.
	err = compileBasalt(input, outputPath, linkerFlags)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Compilation error: %v\n", err)
		os.Exit(1)
	}
	slog.Debug("compiled to", "output", outputPath)

	if action == "run" {
		// Run the compiled program
		cmd := exec.Command(outputPath)
		cmd.Stdout = os.Stdout
		cmd.Stderr = os.Stderr
		if err := cmd.Run(); err != nil {
			// No need to print the error again, as cmd.Run() already pipes it to Stderr.
			os.Exit(1)
		}
	}
}

// compileBasalt now accepts linkerFlags to pass them to the compiler.
func compileBasalt(input string, outputPath string, linkerFlags []string) error {
	// Create lexer
	l := lexer.New(input)

	// Create parser
	p := parser.New(l)

	// Parse the program
	program := p.ParseProgram()
	slog.Debug("parsed program")

	// Check for parser errors
	if len(p.Errors()) > 0 {
		fmt.Println("Parser errors:")
		for _, err := range p.Errors() {
			fmt.Printf("  %s %v:%v\n", err.Msg, err.Line, err.Col)
		}
		return fmt.Errorf("parser errors: %d", len(p.Errors()))
	}

	slog.Debug("resolving module imports")
	err := module.Resolve(program)
	if err != nil {
		return fmt.Errorf("module resolution error: %v", err)
	}

	for _, stmt := range program.Statements {
		fmt.Printf("DEBUG: %v\n", stmt.String())
	}

	slog.Debug("type checking")
	// Create type checker and perform type checking
	typeChecker := checker.New()
	typeChecker.Check(program)

	// Check for type errors
	if len(typeChecker.Errors()) > 0 {
		// Create a string slice to hold the error messages.
		errorMessages := make([]string, len(typeChecker.Errors()))

		// Iterate over the slice of TypeError pointers and format each error message.
		for i, typeErr := range typeChecker.Errors() {
			// Format the error message to include the token's position and the error description.
			errorMessages[i] = fmt.Sprintf("Error at line %d, column %d: %s", typeErr.Token.Line, typeErr.Token.Column, typeErr.Message)
		}

		// Join the error messages into a single string, separated by newlines, and return it as an error.
		return fmt.Errorf("type errors:\n%s", strings.Join(errorMessages, "\n"))
	}

	slog.Debug("compiling")
	// Create compiler and compile to executable
	comp := compiler.New()
	// Pass the linker flags to the final compilation stage.
	return comp.CompileToExecutable(program, outputPath, linkerFlags)
}
