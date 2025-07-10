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

	hasExplicitReturn := false
	for _, stmt := range program.Statements {
		if _, ok := stmt.(*ast.ReturnStatement); ok {
			hasExplicitReturn = true
		}
		c.genStatement(stmt)
	}

	// If no explicit return statement was encountered, and the last instruction is not a terminator,
	// add a default return 0.
	// This handles cases where the program ends with an expression statement or no statements.
	if !hasExplicitReturn && (c.builder.GetInsertBlock().LastInstruction().IsNil() || c.builder.GetInsertBlock().LastInstruction().Opcode() == llvm.Ret) {
		c.builder.CreateRet(llvm.ConstInt(c.context.Int32Type(), 0, false))
	}
}

func (c *CodeGen) genStatement(stmt ast.Statement) {
	switch stmt := stmt.(type) {
	case *ast.LetStatement:
		// TODO: Implement let statement code generation
	case *ast.ExpressionStatement:
		c.genExpression(stmt.Expression) // Just generate the expression, its value might be used later or optimized out
	case *ast.ReturnStatement:
		retVal := c.genExpression(stmt.ReturnValue)
		// Convert the return value to i32 before returning
		if retVal.Type().IntTypeWidth() == 1 {
			retVal = c.builder.CreateZExt(retVal, c.context.Int32Type(), "zexttmp")
		} else if retVal.Type().IntTypeWidth() == 64 {
			retVal = c.builder.CreateTrunc(retVal, c.context.Int32Type(), "trunctmp")
		}
		c.builder.CreateRet(retVal)
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
	case *ast.IfExpression:
		return c.genIfExpression(expr)
	case *ast.Boolean:
		return llvm.ConstInt(c.context.Int1Type(), uint64(0), expr.Value)
	default:
		fmt.Printf("Unknown expression type: %T\n", expr)
		return llvm.Value{}
	}
}

func (c *CodeGen) genInfixExpression(expr *ast.InfixExpression) llvm.Value {
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

func (c *CodeGen) genIfExpression(expr *ast.IfExpression) llvm.Value {
	// Generate condition
	cond := c.genExpression(expr.Condition)

	// Convert condition to boolean (i1)
	if cond.Type().IntTypeWidth() != 1 {
		// If the condition is not already an i1, compare it to 0
		// This assumes non-zero is true, zero is false
		cond = c.builder.CreateICmp(llvm.IntNE, cond, llvm.ConstInt(cond.Type(), 0, false), "condtmp")
	}

	// Get current function
	currentFunc := c.builder.GetInsertBlock().Parent()

	// Create basic blocks for then, else, and merge
	thenBlock := c.context.AddBasicBlock(currentFunc, "then")
	elseBlock := c.context.AddBasicBlock(currentFunc, "else")
	mergeBlock := c.context.AddBasicBlock(currentFunc, "ifcont")

	// Create conditional branch
	c.builder.CreateCondBr(cond, thenBlock, elseBlock)

	// Emit then block
	c.builder.SetInsertPointAtEnd(thenBlock)
	// Generate code for the consequence block
	consequenceVal := c.genBlockStatement(expr.Consequence)
	// If the then block doesn't have a terminator, branch to merge
	if c.builder.GetInsertBlock().LastInstruction().IsNil() || c.builder.GetInsertBlock().LastInstruction().Opcode() != llvm.Ret {
		c.builder.CreateBr(mergeBlock)
	}

	// Emit else block
	c.builder.SetInsertPointAtEnd(elseBlock)
	var alternativeVal llvm.Value
	if expr.Alternative != nil {
		// Generate code for the alternative block
		alternativeVal = c.genBlockStatement(expr.Alternative)
	}
	// If the else block doesn't have a terminator, branch to merge
	if c.builder.GetInsertBlock().LastInstruction().IsNil() || c.builder.GetInsertBlock().LastInstruction().Opcode() != llvm.Ret {
		c.builder.CreateBr(mergeBlock)
	}

	// Emit merge block
	c.builder.SetInsertPointAtEnd(mergeBlock)

	// Create PHI node to merge results from then and else blocks
	phi := c.builder.CreatePHI(c.context.Int64Type(), "iftmp") // Assuming expressions return i64 for now
	phi.AddIncoming([]llvm.Value{consequenceVal, alternativeVal}, []llvm.BasicBlock{thenBlock, elseBlock})

	return phi
}

func (c *CodeGen) genBlockStatement(block *ast.BlockStatement) llvm.Value {
	var lastVal llvm.Value
	for _, stmt := range block.Statements {
		if exprStmt, ok := stmt.(*ast.ExpressionStatement); ok {
			lastVal = c.genExpression(exprStmt.Expression)
		} else {
			c.genStatement(stmt)
		}
	}
	return lastVal
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
