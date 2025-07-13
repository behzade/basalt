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

// internalResolver holds the state for a single, top-level resolution pass.
// It is kept internal to the package and is not exposed.
type internalResolver struct {
	basePaths      []string                // Directories to search for modules, e.g., ["./", "std"].
	loadedModules  map[string]*ast.Program // Caches fully resolved modules to prevent re-parsing. Key: "std/runtime".
	resolvedInPass map[string]bool         // Tracks modules resolved in the current pass to prevent circular dependencies.
}

// Resolve is the single function exposed by the module package.
// It takes the main program's AST, finds all `import` statements, and replaces them
// with the code from the corresponding modules.
func Resolve(program *ast.Program) error {
	resolver := &internalResolver{
		// By default, search for modules in the current directory.
		basePaths:      []string{"."},
		loadedModules:  make(map[string]*ast.Program),
		resolvedInPass: make(map[string]bool),
	}

	// For convenience, automatically add the 'std' directory to the search path if it exists.
	if _, err := os.Stat("std"); err == nil {
		resolver.basePaths = append(resolver.basePaths, "std")
	}

	// Begin the recursive resolution process.
	finalStatements, err := resolver.resolveStatements(program.Statements)
	if err != nil {
		return err
	}

	// Replace the original program's statements with the fully resolved ones.
	program.Statements = finalStatements
	return nil
}

// resolveStatements is the core recursive function that processes a list of statements.
func (r *internalResolver) resolveStatements(statements []ast.Statement) ([]ast.Statement, error) {
	var expandedStatements []ast.Statement

	for _, stmt := range statements {
		importStmt, isImport := stmt.(*ast.ImportStatement)
		if !isImport {
			// Not an import, just keep the statement.
			expandedStatements = append(expandedStatements, stmt)
			continue
		}

		modulePath := convertAstPathToFSPath(importStmt.Path)

		// If already processed in this pass, skip to avoid circular dependency loops.
		if r.resolvedInPass[modulePath] {
			continue
		}
		// Mark as resolved for this pass *before* loading it.
		r.resolvedInPass[modulePath] = true

		// Load the module from disk (or cache). This handles nested resolution.
		moduleProgram, err := r.loadModule(importStmt.Path)
		if err != nil {
			return nil, fmt.Errorf("failed to load module '%s': %w", modulePath, err)
		}

		// Add the resolved statements from the imported module.
		expandedStatements = append(expandedStatements, moduleProgram.Statements...)
	}

	return expandedStatements, nil
}

// loadModule finds, parses, and resolves a module based on its AST path.
func (r *internalResolver) loadModule(path *ast.PathExpression) (*ast.Program, error) {
	modulePath := convertAstPathToFSPath(path)

	// 1. Check cache first to avoid re-doing work.
	if program, exists := r.loadedModules[modulePath]; exists {
		return program, nil
	}

	// 2. Find the module directory on disk by searching base paths.
	moduleDir, err := r.findModuleDirectory(modulePath)
	if err != nil {
		return nil, err
	}

	// 3. Read all .bst files in the directory and parse them into one program.
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

	// 4. IMPORTANT: Recursively resolve any imports *within* this new module.
	resolvedStmts, err := r.resolveStatements(combinedProgram.Statements)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve nested imports in module '%s': %w", modulePath, err)
	}
	combinedProgram.Statements = resolvedStmts

	// 5. Cache the fully resolved module and return it.
	r.loadedModules[modulePath] = combinedProgram
	return combinedProgram, nil
}

// findModuleDirectory searches for a module directory (e.g., "std/runtime") in the base paths.
func (r *internalResolver) findModuleDirectory(modulePath string) (string, error) {
	for _, basePath := range r.basePaths {
		// If base path is "std", and module path is "std/runtime", we don't want "std/std/runtime".
		// We can handle this by checking if the module path already starts with the base path.
		// However, a simpler approach is to have distinct search roots.
		// E.g. basePaths: [".", "std"]
		// Module `My::App` -> `my/app`, found in `./my/app`
		// Module `Std::Fmt` -> `std/fmt`, found in `std/std/fmt` (wrong).
		// Let's adjust the logic slightly. The base path is a root.
		// `Std` modules are special and should be found in the `std` root.

		var fullPath string
		if strings.HasPrefix(modulePath, "std"+string(filepath.Separator)) && basePath == "std" {
			// This prevents `std/std/runtime`.
			fullPath = modulePath
		} else {
			fullPath = filepath.Join(basePath, modulePath)
		}

		if _, err := os.Stat(fullPath); err == nil {
			return fullPath, nil
		}
	}
	return "", fmt.Errorf("module not found. Looked for '%s' in search paths %v", modulePath, r.basePaths)
}

// convertAstPathToFSPath converts an AST path like `Std::Runtime` to a file path like `std/runtime`.
func convertAstPathToFSPath(path *ast.PathExpression) string {
	segments := make([]string, len(path.Segments))
	for i, segment := range path.Segments {
		// Special case for 'Std' -> 'std', otherwise lowercase everything.
		if i == 0 && segment.Value == "Std" {
			segments[i] = "std"
		} else {
			segments[i] = strings.ToLower(segment.Value)
		}
	}
	return filepath.Join(segments...)
}
