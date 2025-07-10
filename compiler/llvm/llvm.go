package llvm

import (
	"fmt"
	"os"

	"tinygo.org/x/go-llvm"
)

// InitializeAndExecuteLLVM initializes LLVM, executes the main function, and prints the result.
func InitializeAndExecuteLLVM(mod llvm.Module) {
	err := llvm.InitializeNativeTarget()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize native target: %s\n", err)
		os.Exit(1)
	}

	err = llvm.InitializeNativeAsmPrinter()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize native asm printer: %s\n", err)
		os.Exit(1)
	}

	executionEngine, err := llvm.NewExecutionEngine(mod)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to create JIT compiler: %s\n", err)
		os.Exit(1)
	}
	defer executionEngine.Dispose()

	funcResult := executionEngine.RunFunction(mod.NamedFunction("main"), []llvm.GenericValue{})
	fmt.Printf("Result: %d\n", funcResult.Int(false))
}
