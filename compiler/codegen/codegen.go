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

	for _, stmt := range program.Statements {
		c.genStatement(stmt)
	}

	// Ensure the main function always has a return instruction.
	// If the last instruction in the current block is not a terminator, add a default return 0.
	lastInst := c.builder.GetInsertBlock().LastInstruction()
	if lastInst.IsNil() || !(lastInst.Opcode() == llvm.Ret || lastInst.Opcode() == llvm.Br || lastInst.Opcode() == llvm.Switch || lastInst.Opcode() == llvm.IndirectBr || lastInst.Opcode() == llvm.Invoke || lastInst.Opcode() == llvm.Unreachable) {
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

// isTerminated checks if a basic block already has a terminator instruction.
func isTerminated(block llvm.BasicBlock) bool {
	lastInst := block.LastInstruction()
	if lastInst.IsNil() {
		return false
	}
	op := lastInst.Opcode()
	// An instruction is a terminator if it's one of Ret, Br, Switch, IndirectBr, Invoke, Unreachable
	return op == llvm.Ret || op == llvm.Br || op == llvm.Switch || op == llvm.IndirectBr || op == llvm.Invoke || op == llvm.Unreachable
}

func (c *CodeGen) genIfExpression(expr *ast.IfExpression) llvm.Value {
	cond := c.genExpression(expr.Condition)

	if cond.Type().IntTypeWidth() != 1 {
		cond = c.builder.CreateICmp(llvm.IntNE, cond, llvm.ConstInt(cond.Type(), 0, false), "condtmp")
	}

	currentFunc := c.builder.GetInsertBlock().Parent()

	// Create basic blocks
	thenBlock := c.context.AddBasicBlock(currentFunc, "then")
	elseBlock := c.context.AddBasicBlock(currentFunc, "else")
	mergeBlock := c.context.AddBasicBlock(currentFunc, "ifcont")

	c.builder.CreateCondBr(cond, thenBlock, elseBlock)

	// --- THEN Block ---
	c.builder.SetInsertPointAtEnd(thenBlock)
	consequenceVal := c.genBlockStatement(expr.Consequence)
	thenTerminated := isTerminated(c.builder.GetInsertBlock())
	if !thenTerminated {
		c.builder.CreateBr(mergeBlock)
	}
	thenFinalBlock := c.builder.GetInsertBlock()

	// --- ELSE Block ---
	var alternativeVal llvm.Value
	elseTerminated := false
	var elseFinalBlock llvm.BasicBlock

	if expr.Alternative != nil {
		c.builder.SetInsertPointAtEnd(elseBlock)
		alternativeVal = c.genBlockStatement(expr.Alternative)
		elseTerminated = isTerminated(c.builder.GetInsertBlock())
		if !elseTerminated {
			c.builder.CreateBr(mergeBlock)
		}
		elseFinalBlock = c.builder.GetInsertBlock()
	} else {
		// If no `else` is provided, the else block just jumps to the merge.
		c.builder.SetInsertPointAtEnd(elseBlock)
		c.builder.CreateBr(mergeBlock)
		elseFinalBlock = elseBlock
	}

	c.builder.SetInsertPointAtEnd(mergeBlock)

	// --- MERGE Block ---
	// If both branches were terminated, the merge block is unreachable.
	// Terminate it with `unreachable` to create valid IR.
	if thenTerminated && elseTerminated {
		c.builder.CreateUnreachable()
		return llvm.Value{}
	}

	// If the if-expression can return a value (i.e., has an `else` part)
	// and at least one branch continues to the merge block, create a PHI node.
	if expr.Alternative != nil {
		phi := c.builder.CreatePHI(c.context.Int64Type(), "iftmp")

		if !thenTerminated {
			phi.AddIncoming([]llvm.Value{consequenceVal}, []llvm.BasicBlock{thenFinalBlock})
		}
		if !elseTerminated {
			phi.AddIncoming([]llvm.Value{alternativeVal}, []llvm.BasicBlock{elseFinalBlock})
		}
		return phi
	}

	// If it's an `if` without an `else`, it produces no value.
	return llvm.Value{}
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
