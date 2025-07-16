package compiler

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/checker"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

// Compiler holds the LLVM IR module and compilation state
type Compiler struct {
	module        *ir.Module
	currentFunc   *ir.Func
	currentBlock  *ir.Block
	symbolTable   map[string]value.Value // Maps variable names to their allocated stack pointers
	functionTable map[string]*ir.Func    // Maps function names to their IR functions
	typeRegistry  map[string]*StructInfo // Maps struct names to their type information
	enumRegistry  map[string]*EnumInfo   // Maps enum names to their type information
	env           *checker.TypeEnvironment
	blockCounter  int // Counter for generating unique block names

	// ARC management fields
	scopeStack          [][]value.Value // Stack of ARC-managed variables per scope
	isNoGCContext       bool            // True if currently compiling inside a #[nogc] function
	currentModulePrefix string
	moduleAliasMap      map[string]string // Maps module alias to full module path
}

// New creates a new compiler instance
func New() *Compiler {
	c := &Compiler{
		module:              ir.NewModule(),
		symbolTable:         make(map[string]value.Value),
		functionTable:       make(map[string]*ir.Func),
		typeRegistry:        make(map[string]*StructInfo),
		enumRegistry:        make(map[string]*EnumInfo),
		env:                 checker.NewTypeEnvironment(),
		blockCounter:        0,
		scopeStack:          make([][]value.Value, 0),
		isNoGCContext:       false,
		currentModulePrefix: "",
		moduleAliasMap:      make(map[string]string),
	}

	return c
}

// Compile compiles the AST program to LLVM IR
func (c *Compiler) Compile(program *ast.Program) (*ir.Module, error) {
	// First pass for the main program to find all top-level declarations
	for _, stmt := range program.Statements {
		if err := c.collectDeclarations(stmt); err != nil {
			return nil, err
		}
	}

	// Now compile the main function and other implementations
	mainFunc := c.module.NewFunc("main", types.I32)
	c.currentFunc = mainFunc
	entryBlock := mainFunc.NewBlock("entry")
	c.currentBlock = entryBlock

	// Second pass for the main program to compile bodies and statements
	for _, stmt := range program.Statements {
		if err := c.compileImplementation(stmt); err != nil {
			return nil, err
		}
	}

	if c.currentBlock.Term == nil {
		c.currentBlock.NewRet(constant.NewInt(types.I32, 0))
	}

	return c.module, nil
}

// CompileToExecutable compiles the program and creates an executable
func (c *Compiler) CompileToExecutable(program *ast.Program, outputPath string, linkerFlags []string) error {
	// Compile to LLVM IR
	module, err := c.Compile(program)
	if err != nil {
		return fmt.Errorf("compilation failed: %w", err)
	}

	// Create temporary directory for intermediate files
	tempDir, err := os.MkdirTemp("", "basalt-compile-*")
	if err != nil {
		return fmt.Errorf("failed to create temp directory: %w", err)
	}
	defer os.RemoveAll(tempDir)

	// Write LLVM IR to file
	irFile := filepath.Join(tempDir, "output.ll")
	irContent := module.String()
	if err := os.WriteFile(irFile, []byte(irContent), 0o644); err != nil {
		return fmt.Errorf("failed to write IR file: %w", err)
	}

	// Also write IR to debug.ll for inspection
	debugFile := "./dist/debug.ll"
	if err := os.WriteFile(debugFile, []byte(irContent), 0o644); err != nil {
		// Don't fail if we can't write debug file
		fmt.Printf("Warning: couldn't write debug file: %v\n", err)
	}

	// Compile IR to object file using llc
	objFile := filepath.Join(tempDir, "output.o")
	llcCmd := exec.Command("llc", "-filetype=obj", irFile, "-o", objFile)
	var llcStdErr bytes.Buffer
	var llcStdOut bytes.Buffer
	llcCmd.Stdout = &llcStdOut
	llcCmd.Stderr = &llcStdErr
	if err := llcCmd.Run(); err != nil {
		return fmt.Errorf("llc compilation failed: %w\n%v\n%v", err, llcStdOut.String(), llcStdErr.String())
	}

	// Prepare the arguments for the clang command.
	// We start with the basic input/output files and any static flags.
	args := []string{objFile, "-o", outputPath, "-fsanitize=address"}

	// Now, append the dynamic linker flags that were passed into this function.
	// These flags come from pkg-config and tell the linker where to find libgc.
	args = append(args, linkerFlags...)

	// Create the clang command with the combined arguments.
	clangCmd := exec.Command("clang", args...)

	// Capture stderr to provide detailed error messages if linking fails.
	var clangStdErr bytes.Buffer
	clangCmd.Stderr = &clangStdErr

	if err := clangCmd.Run(); err != nil {
		// The original error from `Run()` is often just "exit status 1".
		// The really useful information is what clang printed to its stderr.
		return fmt.Errorf("clang linking failed: %w\n--- clang output ---\n%s", err, clangStdErr.String())
	}

	// --- End of updated section ---

	return nil
}
