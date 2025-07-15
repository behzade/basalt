package compiler

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

// compileFunctionDefinition compiles a function definition (let funcName = fn(...) {...})
func (c *Compiler) compileFunctionDefinition(funcName string, expr *ast.FunctionLiteral) error {
	llvmFuncName := funcName
	// Mangle the function name if inside a module
	if c.currentModulePrefix != "" {
		llvmFuncName = fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
	}

	var paramTypes []types.Type
	for _, param := range expr.Parameters {
		if param.Type == nil {
			return fmt.Errorf("function parameter %s must have type annotation", param.Name.Value)
		}
		paramTypes = append(paramTypes, c.typeAnnotationToLLVMType(param.Type))
	}

	var returnType types.Type = types.Void
	if expr.ReturnType != nil {
		returnType = c.typeAnnotationToLLVMType(expr.ReturnType)
	}

	var params []*ir.Param
	for i, param := range expr.Parameters {
		params = append(params, ir.NewParam(param.Name.Value, paramTypes[i]))
	}

	fn := c.module.NewFunc(llvmFuncName, returnType, params...)
	c.functionTable[llvmFuncName] = fn

	savedFunc, savedBlock, savedSymbolTable := c.currentFunc, c.currentBlock, c.symbolTable
	c.currentFunc = fn
	c.symbolTable = make(map[string]value.Value)
	entryBlock := fn.NewBlock("entry")
	c.currentBlock = entryBlock

	for i, param := range expr.Parameters {
		alloca := c.createEntryAlloca(paramTypes[i])
		c.currentBlock.NewStore(fn.Params[i], alloca)
		c.symbolTable[param.Name.Value] = alloca
	}

	bodyValue, err := c.compileBlockStatement(expr.Body)
	if err != nil {
		return err
	}

	if c.currentBlock.Term == nil {
		if returnType == types.Void {
			c.currentBlock.NewRet(nil)
		} else {
			c.currentBlock.NewRet(bodyValue)
		}
	}

	c.currentFunc, c.currentBlock, c.symbolTable = savedFunc, savedBlock, savedSymbolTable
	return nil
}

// compileFunctionDeclaration compiles a function declaration (for two-pass compilation)
func (c *Compiler) compileFunctionDeclaration(funcName string, expr *ast.FunctionLiteral) error {
	llvmFuncName := funcName
	if c.currentModulePrefix != "" {
		llvmFuncName = fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
	}

	// Avoid re-declaration
	if _, exists := c.functionTable[llvmFuncName]; exists {
		return nil
	}

	var paramTypes []types.Type
	for _, param := range expr.Parameters {
		paramTypes = append(paramTypes, c.typeAnnotationToLLVMType(param.Type))
	}

	var returnType types.Type = types.I64 // Default to i64
	if expr.ReturnType != nil {
		returnType = c.typeAnnotationToLLVMType(expr.ReturnType)
	}

	var params []*ir.Param
	for i, param := range expr.Parameters {
		params = append(params, ir.NewParam(param.Name.Value, paramTypes[i]))
	}

	fn := c.module.NewFunc(llvmFuncName, returnType, params...)
	c.functionTable[llvmFuncName] = fn
	return nil
}

// compileFunctionBody compiles a function body (for two-pass compilation)
func (c *Compiler) compileFunctionBody(funcName string, expr *ast.FunctionLiteral) error {
	llvmFuncName := funcName
	if c.currentModulePrefix != "" {
		llvmFuncName = fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
	}

	fn, ok := c.functionTable[llvmFuncName]
	if !ok {
		return fmt.Errorf("internal compiler error: function %s not found in table", llvmFuncName)
	}

	// Function body is already compiled if it has blocks, skip re-compiling.
	if len(fn.Blocks) > 0 {
		return nil
	}

	savedFunc, savedBlock, savedSymbolTable := c.currentFunc, c.currentBlock, c.symbolTable
	c.currentFunc = fn
	c.symbolTable = make(map[string]value.Value)
	entryBlock := fn.NewBlock("entry")
	c.currentBlock = entryBlock

	for i, param := range expr.Parameters {
		alloca := c.createEntryAlloca(fn.Params[i].Typ)
		c.currentBlock.NewStore(fn.Params[i], alloca)
		c.symbolTable[param.Name.Value] = alloca
	}

	bodyValue, err := c.compileBlockStatement(expr.Body)
	if err != nil {
		return err
	}

	if c.currentBlock.Term == nil {
		if fn.Sig.RetType.Equal(types.Void) {
			c.currentBlock.NewRet(nil)
		} else {
			c.currentBlock.NewRet(bodyValue)
		}
	}

	c.currentFunc, c.currentBlock, c.symbolTable = savedFunc, savedBlock, savedSymbolTable
	return nil
}
