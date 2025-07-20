package compiler

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/value"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/types"
)

// compileIfExpression compiles an if-else expression
func (c *Compiler) compileIfExpression(expr *ast.IfExpression) (value.Value, error) {
	// Compile the condition
	condition, err := c.compileExpression(expr.Condition)
	if err != nil {
		return nil, err
	}

	// Generate unique block names
	blockId := c.blockCounter
	c.blockCounter++

	// Create the three basic blocks with unique names
	thenBlock := c.currentFunc.NewBlock(fmt.Sprintf("if.then.%d", blockId))
	elseBlock := c.currentFunc.NewBlock(fmt.Sprintf("if.else.%d", blockId))
	mergeBlock := c.currentFunc.NewBlock(fmt.Sprintf("if.merge.%d", blockId))

	// Branch based on condition
	c.currentBlock.NewCondBr(condition, thenBlock, elseBlock)

	// Compile the then branch
	c.currentBlock = thenBlock
	thenValue, err := c.compileBlockStatement(expr.Consequence)
	if err != nil {
		return nil, err
	}
	// Get the current block (may have changed due to nested control flow)
	thenEndBlock := c.currentBlock
	c.currentBlock.NewBr(mergeBlock)

	// Compile the else branch
	c.currentBlock = elseBlock
	var elseValue value.Value
	var elseEndBlock *ir.Block
	if expr.Alternative != nil {
		elseValue, err = c.compileBlockStatement(expr.Alternative)
		if err != nil {
			return nil, err
		}
		elseEndBlock = c.currentBlock
	} else {
		// No else branch, use a default value (none/0)
		elseValue = constant.NewInt(types.I64, 0)
		elseEndBlock = c.currentBlock
	}
	c.currentBlock.NewBr(mergeBlock)

	// Set up the merge block with PHI node
	c.currentBlock = mergeBlock

	// If the then and else values have different types, we need to handle this
	// For now, assume they have the same type
	if thenValue.Type() != elseValue.Type() {
		println(expr.String())
		return nil, fmt.Errorf("if branches return different types: %s vs %s, %+v vs %+v", thenValue.Type(), elseValue.Type(), thenValue, elseValue)
	}

	// Don't create PHI node for void types
	if thenValue.Type() == types.Void {
		return constant.NewInt(types.I64, 0), nil // Return a dummy value
	}

	// Create PHI node to merge the values
	phi := c.currentBlock.NewPhi(ir.NewIncoming(thenValue, thenEndBlock), ir.NewIncoming(elseValue, elseEndBlock))

	return phi, nil
}

// compileForExpression compiles a for loop expression
func (c *Compiler) compileForExpression(expr *ast.ForExpression) (value.Value, error) {
	// Generate unique block names
	blockId := c.blockCounter
	c.blockCounter++

	// Step 1: Create the three essential basic blocks
	condBlock := c.currentFunc.NewBlock(fmt.Sprintf("loop.cond.%d", blockId))
	bodyBlock := c.currentFunc.NewBlock(fmt.Sprintf("loop.body.%d", blockId))
	exitBlock := c.currentFunc.NewBlock(fmt.Sprintf("loop.exit.%d", blockId))

	// Enter the loop: jump from current block to condition block
	c.currentBlock.NewBr(condBlock)

	// Step 2: Compile the condition check
	c.currentBlock = condBlock
	condition, err := c.compileExpression(expr.Condition)
	if err != nil {
		return nil, err
	}

	// Condition must be boolean (i1)
	if condition.Type() != types.I1 {
		return nil, fmt.Errorf("for loop condition must be boolean, got %s", condition.Type())
	}

	// Create conditional branch: if true go to body, if false go to exit
	c.currentBlock.NewCondBr(condition, bodyBlock, exitBlock)

	// Step 3: Compile the loop body and create the loop
	c.currentBlock = bodyBlock
	_, err = c.compileBlockStatement(expr.Consequence)
	if err != nil {
		return nil, err
	}
	// Jump back to condition block to create the loop
	c.currentBlock.NewBr(condBlock)

	// Step 4: Continue execution after the loop
	c.currentBlock = exitBlock

	// For loops don't produce a meaningful value, return a default
	return constant.NewInt(types.I64, 0), nil
}

// compileMatchExpression compiles match expressions with switch/case logic
func (c *Compiler) compileMatchExpression(expr *ast.MatchExpression) (value.Value, error) {
	// Compile the condition
	conditionValue, err := c.compileExpression(expr.Condition)
	if err != nil {
		return nil, err
	}

	// Get the tag from the enum
	zero := constant.NewInt(types.I32, 0)
	tagIdx := constant.NewInt(types.I32, 0)
	tagPtr := c.currentBlock.NewGetElementPtr(conditionValue.Type().(*types.PointerType).ElemType, conditionValue, zero, tagIdx)
	tagValue := c.currentBlock.NewLoad(types.I32, tagPtr)

	// Create blocks for each arm and a merge block
	c.blockCounter++
	mergeBlock := c.currentFunc.NewBlock(fmt.Sprintf("match_merge_%d", c.blockCounter))

	// Create a switch instruction
	var defaultBlock *ir.Block

	armBlocks := make([]*ir.Block, len(expr.Arms))
	armValues := make([]value.Value, len(expr.Arms))

	// Create blocks for each arm
	for i := range expr.Arms {
		c.blockCounter++
		armBlocks[i] = c.currentFunc.NewBlock(fmt.Sprintf("match_arm_%d_%d", c.blockCounter, i))
	}

	// Create default block (should never be reached due to exhaustiveness checking)
	c.blockCounter++
	defaultBlock = c.currentFunc.NewBlock(fmt.Sprintf("match_default_%d", c.blockCounter))

	// Create switch instruction
	cases := make([]*ir.Case, len(expr.Arms))

	// Create cases for switch
	for i, arm := range expr.Arms {
		// Get the variant info to find the tag
		enumName := arm.Pattern.Enum.Segments[0].Value
		variantName := arm.Pattern.Variant.Value

		enumInfo, exists := c.enumRegistry[enumName]
		if !exists {
			return nil, fmt.Errorf("undefined enum type: %s", enumName)
		}

		variantInfo, exists := enumInfo.Variants[variantName]
		if !exists {
			return nil, fmt.Errorf("undefined variant: %s::%s", enumName, variantName)
		}

		tag := constant.NewInt(types.I32, int64(variantInfo.Tag))
		cases[i] = ir.NewCase(tag, armBlocks[i])
	}

	// Create switch instruction with cases
	c.currentBlock.NewSwitch(tagValue, defaultBlock, cases...)

	// Compile each arm
	for i, arm := range expr.Arms {
		c.currentBlock = armBlocks[i]

		// If the variant has a payload, extract it and bind to pattern variable
		enumName := arm.Pattern.Enum.Segments[0].Value
		variantName := arm.Pattern.Variant.Value

		enumInfo := c.enumRegistry[enumName]
		variantInfo := enumInfo.Variants[variantName]

		if variantInfo.PayloadType != nil && len(arm.Pattern.Arguments) > 0 {
			// Extract the payload
			dataIdx := constant.NewInt(types.I32, 1)
			dataPtr := c.currentBlock.NewGetElementPtr(enumInfo.LLVMType, conditionValue, zero, dataIdx)

			// Cast to the correct type
			payloadPtrType := types.NewPointer(variantInfo.PayloadType)
			castedDataPtr := c.currentBlock.NewBitCast(dataPtr, payloadPtrType)

			// Load the payload
			payloadValue := c.currentBlock.NewLoad(variantInfo.PayloadType, castedDataPtr)

			// Bind to pattern variable
			if ident, ok := arm.Pattern.Arguments[0].(*ast.Identifier); ok {
				// Allocate space for the pattern variable
				patternVar := c.createEntryAlloca(variantInfo.PayloadType)
				c.currentBlock.NewStore(payloadValue, patternVar)

				// Add to symbol table
				c.symbolTable[ident.Value] = patternVar
			}
		}

		// Compile the arm consequence
		armValue, err := c.compileExpression(arm.Consequence)
		if err != nil {
			return nil, err
		}

		armValues[i] = armValue

		// Jump to merge block
		c.currentBlock.NewBr(mergeBlock)
	}

	// Default block (unreachable)
	c.currentBlock = defaultBlock
	c.currentBlock.NewUnreachable()

	// Merge block
	c.currentBlock = mergeBlock

	// Create phi node to collect results
	if len(armValues) > 0 {
		incomings := make([]*ir.Incoming, len(armValues))
		for i := 0; i < len(armValues); i++ {
			incomings[i] = ir.NewIncoming(armValues[i], armBlocks[i])
		}
		phi := c.currentBlock.NewPhi(incomings...)
		return phi, nil
	}

	return constant.NewInt(types.I32, 0), nil
}

// compileBlockStatement compiles a block statement and returns the value of the last expression
func (c *Compiler) compileBlockStatement(block *ast.BlockStatement) (value.Value, error) {
	var lastVal value.Value = constant.NewInt(types.I64, 0) // Default return value for the block

	for i, stmt := range block.Statements {
		// If a previous statement (like a return) has already terminated this block, stop.
		if c.currentBlock.Term != nil {
			break
		}

		// Handle ReturnStatement explicitly as it terminates the block.
		if rs, ok := stmt.(*ast.ReturnStatement); ok {
			if rs.ReturnValue != nil {
				retVal, err := c.compileExpression(rs.ReturnValue)
				if err != nil {
					return nil, err
				}
				c.currentBlock.NewRet(retVal)
			} else {
				c.currentBlock.NewRet(nil)
			}
			// After a return, no more statements in this block can be compiled.
			return lastVal, nil
		}

		// Handle ExpressionStatement to prevent double compilation.
		if es, ok := stmt.(*ast.ExpressionStatement); ok {
			// Compile the expression ONCE.
			exprVal, err := c.compileExpression(es.Expression)
			if err != nil {
				return nil, err
			}

			// If it's the last statement and doesn't have a semicolon,
			// it's the block's implicit return value.
			isLastStatement := i == len(block.Statements)-1
			if isLastStatement && !es.HasSemicolon {
				lastVal = exprVal
			}
		} else {
			// For all other statement types (e.g., LetStatement), use the
			// existing implementation logic.
			if err := c.compileImplementation(stmt); err != nil {
				return nil, err
			}
		}
	}

	return lastVal, nil
}
