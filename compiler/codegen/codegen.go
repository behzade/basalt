package codegen

import (
	"fmt"

	"tinygo.org/x/go-llvm"

	"github.com/behzade/zerolang/compiler/ast"
)

type CodeGen struct {
	context llvm.Context
	module  llvm.Module
	builder llvm.Builder
}

func New() *CodeGen {
	context := llvm.NewContext()
	module := context.NewModule("zerolang")
	builder := context.NewBuilder()

	return &CodeGen{
		context: context,
		module:  module,
		builder: builder,
	}
}

func (c *CodeGen) GenerateCode(program *ast.Program) {
	mainFuncType := llvm.FunctionType(c.context.Int32Type(), []llvm.Type{}, false)
	mainFunc := llvm.AddFunction(c.module, "main", mainFuncType)
	mainBlock := c.context.AddBasicBlock(mainFunc, "entry")
	c.builder.SetInsertPointAtEnd(mainBlock)

	var lastExpr llvm.Value
	for i, stmt := range program.Statements {
		if exprStmt, ok := stmt.(*ast.ExpressionStatement); ok {
			val := c.genExpression(exprStmt.Expression)
			if i == len(program.Statements)-1 { // Check if it's the last statement
				lastExpr = val
			}
		} else {
			c.genStatement(stmt)
		}
	}

	// If the last statement was an expression, return its value
	// Otherwise, return 0
	if lastExpr.IsNil() {
		c.builder.CreateRet(llvm.ConstInt(c.context.Int32Type(), 0, false))
	} else {
		// Convert the last expression to i32 before returning
		if lastExpr.Type().IntTypeWidth() == 1 {
			// If it's a boolean (i1), zero-extend to i32
			lastExpr = c.builder.CreateZExt(lastExpr, c.context.Int32Type(), "zexttmp")
		} else if lastExpr.Type().IntTypeWidth() == 64 {
			// If it's an i64, truncate to i32
			lastExpr = c.builder.CreateTrunc(lastExpr, c.context.Int32Type(), "trunctmp")
		}
		c.builder.CreateRet(lastExpr)
	}
}

func (c *CodeGen) genStatement(stmt ast.Statement) {
	switch stmt := stmt.(type) {
	case *ast.LetStatement:
		// TODO: Implement let statement code generation
	default:
		fmt.Printf("Unknown statement type: %T\n", stmt)
	}
}

func (c *CodeGen) genExpression(expr ast.Expression) llvm.Value {
	switch expr := expr.(type) {
	case *ast.IntegerLiteral:
		return llvm.ConstInt(c.context.Int64Type(), uint64(expr.Value), false)
	case *ast.InfixExpression:
		return c.genInfixExpression(expr)
	default:
		fmt.Printf("Unknown expression type: %T\n", expr)
		return llvm.Value{}
	}
}

func (c *CodeGen) genInfixExpression(expr *ast.InfixExpression) llvm.Value {
	fmt.Printf("Generating infix expression for operator: %s\n", expr.Operator)
	left := c.genExpression(expr.Left)
	right := c.genExpression(expr.Right)


	switch expr.Operator {
	case "+":
		return c.builder.CreateAdd(left, right, "addtmp")
	case "-":	
		return c.builder.CreateSub(left, right, "subtmp")
	case "*":
		return c.builder.CreateMul(left, right, "multmp")
	case "/":
		return c.builder.CreateSDiv(left, right, "divtmp")
	case "<":
		return c.builder.CreateICmp(llvm.IntSLT, left, right, "cmptmp")
	case ">":
		return c.builder.CreateICmp(llvm.IntSGT, left, right, "cmptmp")
	case "==":
		return c.builder.CreateICmp(llvm.IntEQ, left, right, "cmptmp")
	case "!=":
		return c.builder.CreateICmp(llvm.IntNE, left, right, "cmptmp")
	default:
		fmt.Printf("Unknown infix operator: %s\n", expr.Operator)
		return llvm.Value{}
	}
}

func (c *CodeGen) Module() llvm.Module {
	return c.module
}

func (c *CodeGen) Verify() error {
	return llvm.VerifyModule(c.module, llvm.AbortProcessAction)
}

func (c *CodeGen) Dump() {
	c.module.Dump()
}

func (c *CodeGen) Dispose() {
	c.builder.Dispose()
	c.context.Dispose()
}
