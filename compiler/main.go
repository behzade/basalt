package main

import (
	"fmt"
	"io/ioutil"
	"os"

	"tinygo.org/x/go-llvm"

	"github.com/behzade/zerolang/compiler/codegen"
	"github.com/behzade/zerolang/compiler/lexer"
	"github.com/behzade/zerolang/compiler/parser"
)

func main() {
	data, err := ioutil.ReadAll(os.Stdin)
	if err != nil {
		fmt.Fprintf(os.Stderr, "error reading from stdin: %s\n", err)
		os.Exit(1)
	}

	l := lexer.New(string(data))
	p := parser.New(l)

	program := p.ParseProgram()
	if len(p.Errors()) != 0 {
		for _, msg := range p.Errors() {
			fmt.Fprintf(os.Stderr, "\t%s\n", msg)
		}
		os.Exit(1)
	}

		cg := codegen.New()

	cg.GenerateCode(program)

	err = cg.Verify()
	if err != nil {
		fmt.Fprintf(os.Stderr, "LLVM verification error: %s\n", err)
		os.Exit(1)
	}

	cg.Dump()

	err = llvm.InitializeNativeTarget()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize native target: %s\n", err)
		os.Exit(1)
	}

	err = llvm.InitializeNativeAsmPrinter()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to initialize native asm printer: %s\n", err)
		os.Exit(1)
	}

	executionEngine, err := llvm.NewExecutionEngine(cg.Module())
	if err != nil {
		fmt.Fprintf(os.Stderr, "Failed to create JIT compiler: %s\n", err)
		os.Exit(1)
	}
	defer executionEngine.Dispose()

	funcResult := executionEngine.RunFunction(cg.Module().NamedFunction("main"), []llvm.GenericValue{})
	fmt.Printf("Result: %d\n", funcResult.Int(false))
}
