package main

import (
	"fmt"
	"io/ioutil"
	"os"

	"github.com/behzade/zerolang/compiler/codegen"
	"github.com/behzade/zerolang/compiler/lexer"
	ll "github.com/behzade/zerolang/compiler/llvm"
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

	ll.InitializeAndExecuteLLVM(cg.Module())
}