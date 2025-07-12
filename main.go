package main

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/checker"
	"github.com/behzade/basalt/compiler"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/parser"
)

// ModuleResolver handles module resolution and loading
type ModuleResolver struct {
	loadedModules map[string]*ast.Program
	stdPath       string
}

func NewModuleResolver() *ModuleResolver {
	return &ModuleResolver{
		loadedModules: make(map[string]*ast.Program),
		stdPath:       "std",
	}
}

// ResolveModule resolves a module path like "Std::Fmt" to a file path and loads it
func (mr *ModuleResolver) ResolveModule(path *ast.PathExpression) (*ast.Program, error) {
	// Convert path segments to file path
	// Std::Fmt -> std/fmt
	pathSegments := make([]string, len(path.Segments))
	for i, segment := range path.Segments {
		// Convert first letter to lowercase except for the first segment
		segmentValue := segment.Value
		if i == 0 && segmentValue == "Std" {
			pathSegments[i] = "std"
		} else {
			pathSegments[i] = strings.ToLower(segmentValue)
		}
	}

	modulePath := strings.Join(pathSegments, "/")

	// Check if already loaded
	if program, exists := mr.loadedModules[modulePath]; exists {
		return program, nil
	}

	// Find all .bst files in the module directory
	moduleDir := filepath.Join(mr.stdPath, strings.Join(pathSegments[1:], "/"))

	if _, err := os.Stat(moduleDir); os.IsNotExist(err) {
		return nil, fmt.Errorf("module directory not found: %s", moduleDir)
	}

	// Read all .bst files in the directory
	files, err := os.ReadDir(moduleDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read module directory %s: %v", moduleDir, err)
	}

	// Combine all .bst files into a single program
	var allStatements []ast.Statement

	for _, file := range files {
		if !strings.HasSuffix(file.Name(), ".bst") {
			continue
		}

		filePath := filepath.Join(moduleDir, file.Name())
		content, err := os.ReadFile(filePath)
		if err != nil {
			return nil, fmt.Errorf("failed to read module file %s: %v", filePath, err)
		}

		// Parse the file
		l := lexer.New(string(content))
		p := parser.New(l)
		program := p.ParseProgram()

		if len(p.Errors()) > 0 {
			return nil, fmt.Errorf("parser errors in %s: %v", filePath, p.Errors())
		}

		allStatements = append(allStatements, program.Statements...)
	}

	// Create combined program
	combinedProgram := &ast.Program{
		Statements: allStatements,
	}

	// Resolve any nested imports in the module
	err = resolveImports(combinedProgram, mr)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve nested imports in module %s: %v", modulePath, err)
	}

	// Cache the loaded module
	mr.loadedModules[modulePath] = combinedProgram

	return combinedProgram, nil
}

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

	outputPath := "./output"

	// Check for --compile flag
	for i := range os.Args {
		if i+1 < len(os.Args) && os.Args[i+1][:1] != "-" {
			outputPath = os.Args[i+1]
		}
	}

	err := compileBasalt(input, outputPath)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Compilation error: %v\n", err)
		os.Exit(1)
	}
	fmt.Printf("Successfully compiled to %s\n", outputPath)
}

func compileBasalt(input string, outputPath string) error {
	// Create module resolver
	moduleResolver := NewModuleResolver()

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

	// Resolve and load all imported modules
	err := resolveImports(program, moduleResolver)
	if err != nil {
		return fmt.Errorf("module resolution error: %v", err)
	}

	// Debug: Print the resolved program structure (disabled for now)
	// fmt.Printf("DEBUG: Resolved program has %d statements\n", len(program.Statements))
	// for i, stmt := range program.Statements {
	// 	fmt.Printf("DEBUG: Statement %d: %T\n", i, stmt)
	// 	if importStmt, ok := stmt.(*ast.ImportStatement); ok {
	// 		fmt.Printf("  Import: %s\n", importStmt.Path.String())
	// 	}
	// 	if letStmt, ok := stmt.(*ast.LetStatement); ok {
	// 		fmt.Printf("  Let: %s\n", letStmt.Name.Value)
	// 	}
	// 	if externStmt, ok := stmt.(*ast.ExternStatement); ok {
	// 		fmt.Printf("  Extern: %s\n", externStmt.Function.Value)
	// 	}
	// }

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

// resolveImports finds all import statements and loads the corresponding modules
func resolveImports(program *ast.Program, resolver *ModuleResolver) error {
	var imports []*ast.ImportStatement

	// Find all import statements
	for _, stmt := range program.Statements {
		if importStmt, ok := stmt.(*ast.ImportStatement); ok {
			imports = append(imports, importStmt)
		}
	}

	// Load each imported module and add its statements to the program
	for _, importStmt := range imports {
		moduleProgram, err := resolver.ResolveModule(importStmt.Path)
		if err != nil {
			return fmt.Errorf("failed to resolve module %s: %v", importStmt.Path.String(), err)
		}

		// Create a module context by adding the import statement followed by the module's statements
		var newStatements []ast.Statement

		// Add the import statement first
		newStatements = append(newStatements, importStmt)

		// Add module statements immediately after the import
		newStatements = append(newStatements, moduleProgram.Statements...)

		// Add original statements (excluding the import statement we already added)
		for _, stmt := range program.Statements {
			if stmt != importStmt {
				newStatements = append(newStatements, stmt)
			}
		}

		program.Statements = newStatements
	}

	return nil
}
