package module

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/behzade/basalt/ast"    // Adjust to your project's path
	"github.com/behzade/basalt/lexer"  // Adjust to your project's path
	"github.com/behzade/basalt/parser" // Adjust to your project's path
)

// internalResolver remains the same.
type internalResolver struct {
	basePaths      []string
	loadedModules  map[string]*ast.Program
	resolvedInPass map[string]bool
}

// Resolve remains the same.
func Resolve(program *ast.Program) error {
	resolver := &internalResolver{
		basePaths:      []string{"."},
		loadedModules:  make(map[string]*ast.Program),
		resolvedInPass: make(map[string]bool),
	}
	if _, err := os.Stat("std"); err == nil {
		resolver.basePaths = append(resolver.basePaths, "std")
	}
	finalStatements, err := resolver.resolveStatements(program.Statements)
	if err != nil {
		return err
	}
	program.Statements = finalStatements
	return nil
}

// MODIFIED: This is the core logic change.
func (r *internalResolver) resolveStatements(statements []ast.Statement) ([]ast.Statement, error) {
	var expandedStatements []ast.Statement

	for _, stmt := range statements {
		importStmt, isImport := stmt.(*ast.ImportStatement)
		if !isImport {
			expandedStatements = append(expandedStatements, stmt)
			continue
		}

		modulePath := convertAstPathToFSPath(importStmt.Path)

		if r.resolvedInPass[modulePath] {
			// Circular dependency detected, but it's not an error.
			// The module object will be created by the first import encountered.
			// We just skip this import statement.
			continue
		}
		r.resolvedInPass[modulePath] = true

		// Load the module from disk (or cache), which includes resolving its own imports.
		moduleProgram, err := r.loadModule(importStmt.Path)
		if err != nil {
			return nil, fmt.Errorf("failed to load module '%s': %w", modulePath, err)
		}

		// NEW: Instead of injecting statements, create a ModuleStatement.
		// This wraps the entire module's AST under a single name.
		moduleAlias := getAliasFromPath(importStmt)

		moduleNode := &ast.ModuleStatement{
			Token:  importStmt.Token, // Preserve the original import token for position info
			Name:   moduleAlias,
			Module: moduleProgram,
		}

		expandedStatements = append(expandedStatements, moduleNode)
	}

	return expandedStatements, nil
}

// loadModule remains largely the same.
func (r *internalResolver) loadModule(path *ast.PathExpression) (*ast.Program, error) {
	modulePath := convertAstPathToFSPath(path)
	if program, exists := r.loadedModules[modulePath]; exists {
		return program, nil
	}

	moduleDir, err := r.findModuleDirectory(modulePath)
	if err != nil {
		return nil, err
	}

	files, err := os.ReadDir(moduleDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read module directory '%s': %w", moduleDir, err)
	}

	var allStatements []ast.Statement
	hasFiles := false
	for _, file := range files {
		if !strings.HasSuffix(file.Name(), ".bst") {
			continue
		}
		hasFiles = true
		filePath := filepath.Join(moduleDir, file.Name())
		content, err := os.ReadFile(filePath)
		if err != nil {
			return nil, fmt.Errorf("failed to read module file '%s': %w", filePath, err)
		}

		l := lexer.New(string(content))
		p := parser.New(l)
		program := p.ParseProgram()
		if len(p.Errors()) > 0 {
			return nil, fmt.Errorf("parser errors in '%s': %v", filePath, p.Errors())
		}
		allStatements = append(allStatements, program.Statements...)
	}

	if !hasFiles {
		return nil, fmt.Errorf("no .bst files found in module directory '%s'", moduleDir)
	}

	combinedProgram := &ast.Program{Statements: allStatements}

	// IMPORTANT: Recursively resolve imports within this module *before* caching.
	resolvedStmts, err := r.resolveStatements(combinedProgram.Statements)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve nested imports in module '%s': %w", modulePath, err)
	}
	combinedProgram.Statements = resolvedStmts

	r.loadedModules[modulePath] = combinedProgram
	return combinedProgram, nil
}

// findModuleDirectory remains the same.
func (r *internalResolver) findModuleDirectory(modulePath string) (string, error) {
	for _, basePath := range r.basePaths {
		var fullPath string
		// This logic is slightly tricky. Let's simplify and make it robust.
		// We join the base path and module path, then clean it.
		fullPath = filepath.Join(basePath, modulePath)

		// This handles cases like basePath="." and modulePath="std/runtime" -> "./std/runtime"
		// And basePath="std" and modulePath="runtime" -> "std/runtime" (if you adjust the import path)
		if _, err := os.Stat(fullPath); err == nil {
			return fullPath, nil
		}
	}
	return "", fmt.Errorf("module not found. Looked for '%s' in search paths %v", modulePath, r.basePaths)
}

// convertAstPathToFSPath remains the same.
func convertAstPathToFSPath(path *ast.PathExpression) string {
	segments := make([]string, len(path.Segments))
	for i, segment := range path.Segments {
		segments[i] = strings.ToLower(segment.Value)
	}
	return filepath.Join(segments...)
}

// NEW: Helper to determine the module's name in the code.
// Handles aliasing `import Std::Fmt as Formatter` -> `Formatter`
// and default naming `import Std::Fmt` -> `Fmt`
func getAliasFromPath(imp *ast.ImportStatement) *ast.Identifier {
	if imp.Alias != nil {
		return imp.Alias
	}

	// No alias, so use the last part of the path. e.g., Std::Fmt -> Fmt
	lastSegment := imp.Path.Segments[len(imp.Path.Segments)-1]
	return &ast.Identifier{Token: lastSegment.Token, Value: lastSegment.Value}
}
