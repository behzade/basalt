package compiler

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/checker"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/enum"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

// StructInfo holds information about a struct type
type StructInfo struct {
	LLVMType   *types.StructType
	FieldNames []string              // Ordered list of field names
	FieldTypes map[string]types.Type // Maps field name to LLVM type
	FieldIndex map[string]int        // Maps field name to index in struct
}

// EnumVariantInfo holds information about an enum variant
type EnumVariantInfo struct {
	Tag         int32      // The tag value for this variant
	PayloadType types.Type // LLVM type for the payload (nil if no payload)
}

// EnumInfo holds information about an enum type
type EnumInfo struct {
	LLVMType    *types.StructType           // The LLVM struct type for the enum
	Variants    map[string]*EnumVariantInfo // Maps variant name to variant info
	TagToName   map[int32]string            // Maps tag values to variant names
	MaxDataSize int                         // Size of the largest variant payload
}

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

// compileStatement compiles a statement
func (c *Compiler) compileStatement(stmt ast.Statement) error {
	switch s := stmt.(type) {
	case *ast.ModuleStatement:
		return c.compileModuleStatement(s)
	case *ast.LetStatement:
		return c.compileLetStatement(s)
	case *ast.ExpressionStatement:
		_, err := c.compileExpression(s.Expression)
		return err
	case *ast.ReturnStatement:
		return c.compileReturnStatement(s)
	case *ast.ExternStatement:
		return c.compileExternStatement(s)
	case *ast.ImportStatement:
		return c.compileImportStatement(s)
	case *ast.UnsafeStatement:
		return c.compileUnsafeStatement(s)
	// EnumLiteral is now handled as an expression in let statements
	default:
		return fmt.Errorf("unsupported statement type: %T", stmt)
	}
}

// compileLetStatement compiles a let statement
func (c *Compiler) compileLetStatement(stmt *ast.LetStatement) error {
	// Special handling for function literals
	if funcLit, ok := stmt.Value.(*ast.FunctionLiteral); ok {
		return c.compileFunctionDefinition(stmt.Name.Value, funcLit)
	}

	// Special handling for struct definitions
	if structLit, ok := stmt.Value.(*ast.StructLiteral); ok {
		return c.compileStructDefinition(stmt.Name.Value, structLit)
	}

	// Special handling for enum definitions
	if enumLit, ok := stmt.Value.(*ast.EnumLiteral); ok {
		return c.compileEnumDefinition(stmt.Name.Value, enumLit)
	}

	// Regular variable assignment
	// Compile the right-hand side expression first to get its type
	value, err := c.compileExpression(stmt.Value)
	if err != nil {
		return err
	}

	// Special handling for the underscore identifier - discard the value
	if stmt.Name.Value == "_" {
		// Just evaluate the expression for side effects, don't store it
		return nil
	}

	// Determine the target type
	var targetType types.Type
	if stmt.Type != nil {
		// Use the declared type annotation
		targetType = c.typeAnnotationToLLVMType(stmt.Type)

		// The checker has already approved this assignment. If types don't match,
		// it must be a valid cast (e.g., rawptr to *MyStruct).
		if !value.Type().Equal(targetType) {
			value = c.currentBlock.NewBitCast(value, targetType)
		}
	} else {
		// Use the actual type of the compiled value
		targetType = value.Type()
	}

	// Allocate stack space for the variable
	alloca := c.currentBlock.NewAlloca(targetType)

	// Store the value in the allocated space
	c.currentBlock.NewStore(value, alloca)

	// Add to symbol table
	c.symbolTable[stmt.Name.Value] = alloca

	// Track ARC-managed variables in current scope
	if c.isARCManagedValue(value) && len(c.scopeStack) > 0 {
		currentScopeIndex := len(c.scopeStack) - 1
		c.scopeStack[currentScopeIndex] = append(c.scopeStack[currentScopeIndex], value)
	}

	return nil
}

// compileReturnStatement compiles a return statement
func (c *Compiler) compileReturnStatement(stmt *ast.ReturnStatement) error {
	if stmt.ReturnValue == nil {
		c.currentBlock.NewRet(constant.NewInt(types.I32, 0))
		return nil
	}

	value, err := c.compileExpression(stmt.ReturnValue)
	if err != nil {
		return err
	}

	// If returning an ARC-managed value, remove it from current scope's release list
	// to prevent it from being freed before the caller receives it
	// This is only needed when not in nogc context
	if !c.isNoGCContext && c.isARCManagedValue(value) && len(c.scopeStack) > 0 {
		currentScopeIndex := len(c.scopeStack) - 1
		currentScope := c.scopeStack[currentScopeIndex]

		// Remove the returned value from the scope's release list
		for i, scopeValue := range currentScope {
			if scopeValue == value {
				// Remove from slice
				c.scopeStack[currentScopeIndex] = append(currentScope[:i], currentScope[i+1:]...)
				break
			}
		}
	}

	// For now, assume main returns int
	c.currentBlock.NewRet(value)
	return nil
}

// compileExpression compiles an expression and returns its value
func (c *Compiler) compileExpression(expr ast.Expression) (value.Value, error) {
	switch e := expr.(type) {
	case *ast.IntegerLiteral:
		return constant.NewInt(types.I64, e.Value), nil
	case *ast.Boolean:
		if e.Value {
			return constant.NewBool(true), nil
		}
		return constant.NewBool(false), nil
	case *ast.FloatLiteral:
		return constant.NewFloat(types.Double, e.Value), nil
	case *ast.StringLiteral:
		return c.compileStringLiteral(e)
	case *ast.Identifier:
		return c.compileIdentifier(e)
	case *ast.PrefixExpression:
		return c.compilePrefixExpression(e)
	case *ast.InfixExpression:
		return c.compileInfixExpression(e)
	case *ast.CallExpression:
		return c.compileCallExpression(e)
	case *ast.IfExpression:
		return c.compileIfExpression(e)
	case *ast.ForExpression:
		return c.compileForExpression(e)
	case *ast.FunctionLiteral:
		return c.compileFunctionLiteral(e)
	case *ast.ArrayLiteral:
		return c.compileArrayLiteral(e)
	case *ast.IndexExpression:
		return c.compileIndexExpression(e)
	case *ast.MemberAccessExpression:
		return c.compileMemberAccessExpression(e)
	case *ast.StructLiteral:
		return c.compileStructLiteral(e)
	case *ast.StructInstanceExpression:
		return c.compileStructInstanceExpression(e)
	case *ast.EnumInstantiationExpression:
		return c.compileEnumInstantiationExpression(e)
	case *ast.MatchExpression:
		return c.compileMatchExpression(e)
	case *ast.EnumLiteral:
		return c.compileEnumLiteral(e)
	case *ast.HashLiteral:
		return c.compileHashLiteral(e)
	default:
		return nil, fmt.Errorf("unsupported expression type: %T", expr)
	}
}

// compilePrefixExpression compiles a prefix expression
func (c *Compiler) compilePrefixExpression(expr *ast.PrefixExpression) (value.Value, error) {
	right, err := c.compileExpression(expr.Right)
	if err != nil {
		return nil, err
	}

	switch expr.Operator {
	case "-":
		// Negate the value
		if right.Type() == types.I64 {
			zero := constant.NewInt(types.I64, 0)
			return c.currentBlock.NewSub(zero, right), nil
		} else if right.Type() == types.Double {
			zero := constant.NewFloat(types.Double, 0.0)
			return c.currentBlock.NewFSub(zero, right), nil
		}
		return nil, fmt.Errorf("unsupported type for negation: %s", right.Type())
	case "!":
		// Logical NOT
		if right.Type() == types.I1 {
			return c.currentBlock.NewXor(right, constant.NewBool(true)), nil
		}
		return nil, fmt.Errorf("unsupported type for logical NOT: %s", right.Type())
	default:
		return nil, fmt.Errorf("unsupported prefix operator: %s", expr.Operator)
	}
}

// compileFunctionLiteral compiles a function literal
func (c *Compiler) compileFunctionLiteral(expr *ast.FunctionLiteral) (value.Value, error) {
	// Generate unique function name
	funcName := fmt.Sprintf("func_%d", c.blockCounter)
	c.blockCounter++

	// Determine parameter types
	var paramTypes []types.Type
	for _, param := range expr.Parameters {
		if param.Type == nil {
			return nil, fmt.Errorf("function parameter %s must have type annotation", param.Name.Value)
		}
		paramTypes = append(paramTypes, c.typeAnnotationToLLVMType(param.Type))
	}

	// Determine return type
	var returnType types.Type = types.I64 // Default to i64
	if expr.ReturnType != nil {
		returnType = c.typeAnnotationToLLVMType(expr.ReturnType)
	}

	// Create function parameters
	var params []*ir.Param
	for i, param := range expr.Parameters {
		params = append(params, ir.NewParam(param.Name.Value, paramTypes[i]))
	}

	// Create the function
	fn := c.module.NewFunc(funcName, returnType, params...)

	// Save current compilation state
	savedFunc := c.currentFunc
	savedBlock := c.currentBlock
	savedSymbolTable := c.symbolTable

	// Set up new compilation context for the function
	c.currentFunc = fn
	c.symbolTable = make(map[string]value.Value)

	// Create entry block
	entryBlock := fn.NewBlock("entry")
	c.currentBlock = entryBlock

	// Allocate stack space for parameters and store their values
	for i, param := range expr.Parameters {
		alloca := c.currentBlock.NewAlloca(paramTypes[i])
		c.currentBlock.NewStore(fn.Params[i], alloca)
		c.symbolTable[param.Name.Value] = alloca
	}

	// Compile function body
	bodyValue, err := c.compileBlockStatement(expr.Body)
	if err != nil {
		return nil, err
	}

	// Add return statement
	if returnType == types.Void {
		c.currentBlock.NewRet(nil)
	} else {
		c.currentBlock.NewRet(bodyValue)
	}

	// Restore previous compilation state
	c.currentFunc = savedFunc
	c.currentBlock = savedBlock
	c.symbolTable = savedSymbolTable

	// Return a placeholder value (function literals don't have a direct value representation)
	// The actual function will be stored in the function table when assigned to a variable
	return constant.NewInt(types.I64, 0), nil
}

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

// compileBlockStatement compiles a block statement and returns the value of the last expression
func (c *Compiler) compileBlockStatement(block *ast.BlockStatement) (value.Value, error) {
	var lastVal value.Value = constant.NewInt(types.I64, 0)
	for i, stmt := range block.Statements {
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
			return lastVal, nil // Return after terminator
		}

		// compileImplementation handles the logic of what to compile now.
		if err := c.compileImplementation(stmt); err != nil {
			return nil, err
		}

		// If it was an expression statement, capture its value only if it's the last statement
		// and doesn't have a semicolon (for implicit return)
		if es, ok := stmt.(*ast.ExpressionStatement); ok {
			isLastStatement := i == len(block.Statements)-1
			if isLastStatement && !es.HasSemicolon {
				// This is the last statement and it doesn't have a semicolon,
				// so its value should be used for implicit return
				var err error
				lastVal, err = c.compileExpression(es.Expression)
				if err != nil {
					return nil, err
				}
			} else {
				// Either not the last statement or has a semicolon,
				// so just compile it but don't use its value
				_, err := c.compileExpression(es.Expression)
				if err != nil {
					return nil, err
				}
			}
		}
	}
	return lastVal, nil
}

// isARCManagedValue checks if a value is ARC-managed (allocated with arc_alloc_internal)
func (c *Compiler) isARCManagedValue(val value.Value) bool {
	// For now, we consider string pointers as ARC-managed
	// This is a simplified check - in a full implementation, we'd need to track
	// which values were allocated with arc_alloc_internal
	return c.isStringType(val.Type())
}

// compileIdentifier compiles an identifier (variable lookup)
func (c *Compiler) compileIdentifier(ident *ast.Identifier) (value.Value, error) {
	ptr, exists := c.symbolTable[ident.Value]
	if !exists {
		return nil, fmt.Errorf("undefined variable: %s", ident.Value)
	}

	// Get the type of the allocated value by examining the pointer type
	ptrType, ok := ptr.Type().(*types.PointerType)
	if !ok {
		return nil, fmt.Errorf("invalid pointer type for variable %s", ident.Value)
	}

	// Load the value from the pointer
	return c.currentBlock.NewLoad(ptrType.ElemType, ptr), nil
}

// compileInfixExpression compiles an infix expression
func (c *Compiler) compileInfixExpression(expr *ast.InfixExpression) (value.Value, error) {
	left, err := c.compileExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	right, err := c.compileExpression(expr.Right)
	if err != nil {
		return nil, err
	}

	switch expr.Operator {
	case "+":
		return c.compileAddition(left, right)
	case "-":
		return c.compileSubtraction(left, right)
	case "*":
		return c.compileMultiplication(left, right)
	case "/":
		return c.compileDivision(left, right)
	case "==":
		return c.compileEquality(left, right)
	case "!=":
		return c.compileInequality(left, right)
	case "<":
		return c.compileLessThan(left, right)
	case ">":
		return c.compileGreaterThan(left, right)
	case "=":
		// Assignment operator - left should be an identifier or member access
		if ident, ok := expr.Left.(*ast.Identifier); ok {
			// Look up the variable in symbol table
			ptr, exists := c.symbolTable[ident.Value]
			if !exists {
				return nil, fmt.Errorf("undefined variable: %s", ident.Value)
			}

			// Handle ARC retain/release for assignment only if not in nogc context
			if !c.isNoGCContext {
				// If assigning an ARC-managed value, retain it
				if c.isARCManagedValue(right) {
					arcRetainFunc, ok := c.functionTable["arc_retain"]
					if ok {
						c.currentBlock.NewCall(arcRetainFunc, right)
					}
				}

				// If the variable previously held an ARC-managed value, release it
				// This is simplified - in a full implementation, we'd need to track
				// what the variable previously contained
			}

			// Store the right-hand side value into the variable
			c.currentBlock.NewStore(right, ptr)
			// Return the assigned value
			return right, nil
		} else if memberAccess, ok := expr.Left.(*ast.MemberAccessExpression); ok {
			// Handle struct field assignment
			return c.compileStructFieldAssignment(memberAccess, right)
		} else if indexExpr, ok := expr.Left.(*ast.IndexExpression); ok {
			// Handle HashMap or array element assignment
			return c.compileIndexAssignment(indexExpr, right)
		}
		return nil, fmt.Errorf("assignment target must be an identifier, struct field, or index expression")
	default:
		return nil, fmt.Errorf("unsupported operator: %s", expr.Operator)
	}
}

// Helper methods for binary operations with type handling

// compileAddition handles addition with type conversions and string concatenation
func (c *Compiler) compileAddition(left, right value.Value) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// String concatenation
	if c.isStringType(leftType) && c.isStringType(rightType) {
		return c.compileStringConcatenation(left, right)
	}

	// Pointer arithmetic: ptr + int or int + ptr
	if c.isPointerType(leftType) && c.isIntegerType(rightType) {
		return c.compilePointerArithmetic(left, right, "add")
	}
	if c.isIntegerType(leftType) && c.isPointerType(rightType) {
		return c.compilePointerArithmetic(right, left, "add")
	}

	// Numeric addition with type promotion
	return c.compileNumericOperation(left, right, "add")
}

// compileSubtraction handles subtraction with type conversions
func (c *Compiler) compileSubtraction(left, right value.Value) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// Pointer arithmetic: ptr - int
	if c.isPointerType(leftType) && c.isIntegerType(rightType) {
		return c.compilePointerArithmetic(left, right, "sub")
	}

	return c.compileNumericOperation(left, right, "sub")
}

// compileMultiplication handles multiplication with type conversions
func (c *Compiler) compileMultiplication(left, right value.Value) (value.Value, error) {
	return c.compileNumericOperation(left, right, "mul")
}

// compileDivision handles division with type conversions
func (c *Compiler) compileDivision(left, right value.Value) (value.Value, error) {
	return c.compileNumericOperation(left, right, "div")
}

// compileEquality handles equality comparison with string support
func (c *Compiler) compileEquality(left, right value.Value) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// String comparison
	if c.isStringType(leftType) && c.isStringType(rightType) {
		return c.compileStringComparison(left, right, true)
	}

	// Pointer comparison with null (0)
	if c.isPointerType(leftType) && c.isIntegerType(rightType) {
		return c.compilePointerComparison(left, right, "eq")
	}
	if c.isIntegerType(leftType) && c.isPointerType(rightType) {
		return c.compilePointerComparison(right, left, "eq")
	}

	// Numeric comparison
	return c.compileNumericComparison(left, right, "eq")
}

// compileInequality handles inequality comparison with string support
func (c *Compiler) compileInequality(left, right value.Value) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// String comparison
	if c.isStringType(leftType) && c.isStringType(rightType) {
		return c.compileStringComparison(left, right, false)
	}

	// Pointer comparison with null (0)
	if c.isPointerType(leftType) && c.isIntegerType(rightType) {
		return c.compilePointerComparison(left, right, "ne")
	}
	if c.isIntegerType(leftType) && c.isPointerType(rightType) {
		return c.compilePointerComparison(right, left, "ne")
	}

	// Numeric comparison
	return c.compileNumericComparison(left, right, "ne")
}

// compileLessThan handles less than comparison
func (c *Compiler) compileLessThan(left, right value.Value) (value.Value, error) {
	return c.compileNumericComparison(left, right, "lt")
}

// compileGreaterThan handles greater than comparison
func (c *Compiler) compileGreaterThan(left, right value.Value) (value.Value, error) {
	return c.compileNumericComparison(left, right, "gt")
}

// compileNumericOperation handles arithmetic operations with type promotion
func (c *Compiler) compileNumericOperation(left, right value.Value, op string) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// Promote types if necessary
	if leftType.Equal(types.I64) && rightType.Equal(types.Double) {
		// Convert int to float
		left = c.currentBlock.NewSIToFP(left, types.Double)
		leftType = types.Double
	} else if leftType.Equal(types.Double) && rightType.Equal(types.I64) {
		// Convert int to float
		right = c.currentBlock.NewSIToFP(right, types.Double)
		rightType = types.Double
	}

	// Perform operation based on type
	if leftType.Equal(types.Double) && rightType.Equal(types.Double) {
		switch op {
		case "add":
			return c.currentBlock.NewFAdd(left, right), nil
		case "sub":
			return c.currentBlock.NewFSub(left, right), nil
		case "mul":
			return c.currentBlock.NewFMul(left, right), nil
		case "div":
			return c.currentBlock.NewFDiv(left, right), nil
		}
	} else if leftType.Equal(types.I64) && rightType.Equal(types.I64) {
		switch op {
		case "add":
			return c.currentBlock.NewAdd(left, right), nil
		case "sub":
			return c.currentBlock.NewSub(left, right), nil
		case "mul":
			return c.currentBlock.NewMul(left, right), nil
		case "div":
			return c.currentBlock.NewSDiv(left, right), nil
		}
	}

	return nil, fmt.Errorf("unsupported types for %s operation: %s %s %s", op, leftType, op, rightType)
}

// compileNumericComparison handles numeric comparisons with type promotion
func (c *Compiler) compileNumericComparison(left, right value.Value, op string) (value.Value, error) {
	leftType := left.Type()
	rightType := right.Type()

	// Promote types if necessary
	if leftType.Equal(types.I64) && rightType.Equal(types.Double) {
		// Convert int to float
		left = c.currentBlock.NewSIToFP(left, types.Double)
		leftType = types.Double
	} else if leftType.Equal(types.Double) && rightType.Equal(types.I64) {
		// Convert int to float
		right = c.currentBlock.NewSIToFP(right, types.Double)
		rightType = types.Double
	}

	// Perform comparison based on type
	if leftType.Equal(types.Double) && rightType.Equal(types.Double) {
		switch op {
		case "eq":
			return c.currentBlock.NewFCmp(enum.FPredOEQ, left, right), nil
		case "ne":
			return c.currentBlock.NewFCmp(enum.FPredONE, left, right), nil
		case "lt":
			return c.currentBlock.NewFCmp(enum.FPredOLT, left, right), nil
		case "gt":
			return c.currentBlock.NewFCmp(enum.FPredOGT, left, right), nil
		}
	} else if leftType.Equal(types.I64) && rightType.Equal(types.I64) {
		switch op {
		case "eq":
			return c.currentBlock.NewICmp(enum.IPredEQ, left, right), nil
		case "ne":
			return c.currentBlock.NewICmp(enum.IPredNE, left, right), nil
		case "lt":
			return c.currentBlock.NewICmp(enum.IPredSLT, left, right), nil
		case "gt":
			return c.currentBlock.NewICmp(enum.IPredSGT, left, right), nil
		}
	}

	return nil, fmt.Errorf("unsupported types for %s comparison: %s %s %s", op, leftType, op, rightType)
}

// compileStringConcatenation implements string concatenation using C library functions
func (c *Compiler) compileStringConcatenation(left, right value.Value) (value.Value, error) {
	// Get strlen function
	strlenFunc, ok := c.functionTable["strlen"]
	if !ok {
		return nil, fmt.Errorf("strlen function not found - ensure libc functions are declared")
	}

	// Get malloc function
	mallocFunc, ok := c.functionTable["malloc"]
	if !ok {
		return nil, fmt.Errorf("malloc function not found - ensure libc functions are declared")
	}

	// Get strcpy function
	strcpyFunc, ok := c.functionTable["strcpy"]
	if !ok {
		return nil, fmt.Errorf("strcpy function not found - ensure libc functions are declared")
	}

	// Get strcat function
	strcatFunc, ok := c.functionTable["strcat"]
	if !ok {
		return nil, fmt.Errorf("strcat function not found - ensure libc functions are declared")
	}

	// Get lengths of both strings
	leftLen := c.currentBlock.NewCall(strlenFunc, left)
	rightLen := c.currentBlock.NewCall(strlenFunc, right)

	// Calculate total length + 1 for null terminator
	totalLen := c.currentBlock.NewAdd(leftLen, rightLen)
	one := constant.NewInt(types.I64, 1)
	totalLenPlusOne := c.currentBlock.NewAdd(totalLen, one)

	// Allocate memory for result
	var result value.Value
	if c.isNoGCContext {
		// In nogc context, use regular malloc
		result = c.currentBlock.NewCall(mallocFunc, totalLenPlusOne)
	} else {
		// In regular context, use ARC allocation
		arcAllocFunc, ok := c.functionTable["arc_alloc_internal"]
		if !ok {
			return nil, fmt.Errorf("arc_alloc_internal function not found")
		}
		result = c.currentBlock.NewCall(arcAllocFunc, totalLenPlusOne)
	}

	// Copy left string to result
	c.currentBlock.NewCall(strcpyFunc, result, left)

	// Concatenate right string to result
	c.currentBlock.NewCall(strcatFunc, result, right)

	return result, nil
}

// compileStringComparison implements string comparison using strcmp
func (c *Compiler) compileStringComparison(left, right value.Value, isEqual bool) (value.Value, error) {
	// Get strcmp function
	strcmpFunc, ok := c.functionTable["strcmp"]
	if !ok {
		return nil, fmt.Errorf("strcmp function not found - ensure libc functions are declared")
	}

	// Call strcmp
	result := c.currentBlock.NewCall(strcmpFunc, left, right)

	// Compare result with 0
	zero := constant.NewInt(types.I32, 0)
	if isEqual {
		return c.currentBlock.NewICmp(enum.IPredEQ, result, zero), nil
	} else {
		return c.currentBlock.NewICmp(enum.IPredNE, result, zero), nil
	}
}

// isStringType checks if a type is a string (pointer to i8)
func (c *Compiler) isStringType(t types.Type) bool {
	if ptrType, ok := t.(*types.PointerType); ok {
		return ptrType.ElemType == types.I8
	}
	return false
}

// isPointerType checks if a type is a pointer type (including rawptr)
func (c *Compiler) isPointerType(t types.Type) bool {
	_, ok := t.(*types.PointerType)
	return ok
}

// isIntegerType checks if a type is an integer type
func (c *Compiler) isIntegerType(t types.Type) bool {
	_, ok := t.(*types.IntType)
	return ok
}

// compilePointerArithmetic handles pointer arithmetic operations (ptr + int, ptr - int)
func (c *Compiler) compilePointerArithmetic(ptr, offset value.Value, op string) (value.Value, error) {
	// For pointer arithmetic, we use getelementptr instruction
	// This is the safe way to do pointer arithmetic in LLVM

	// Convert offset to proper type if needed
	offsetVal := offset
	if offsetVal.Type() != types.I64 {
		offsetVal = c.currentBlock.NewSExt(offsetVal, types.I64)
	}

	// For subtraction, negate the offset
	if op == "sub" {
		offsetVal = c.currentBlock.NewSub(constant.NewInt(types.I64, 0), offsetVal)
	}

	// Use getelementptr for pointer arithmetic
	// Since rawptr is i8*, we can use byte-level arithmetic
	return c.currentBlock.NewGetElementPtr(types.I8, ptr, offsetVal), nil
}

// compilePointerComparison handles pointer comparison with null (0)
func (c *Compiler) compilePointerComparison(ptr, nullVal value.Value, op string) (value.Value, error) {
	// Convert null value to pointer type
	nullPtr := c.currentBlock.NewIntToPtr(nullVal, ptr.Type())

	// Compare pointers
	switch op {
	case "eq":
		return c.currentBlock.NewICmp(enum.IPredEQ, ptr, nullPtr), nil
	case "ne":
		return c.currentBlock.NewICmp(enum.IPredNE, ptr, nullPtr), nil
	default:
		return nil, fmt.Errorf("unsupported pointer comparison operator: %s", op)
	}
}

func (c *Compiler) compileCallExpression(expr *ast.CallExpression) (value.Value, error) {
	var fn *ir.Func
	var exists bool

	if ident, ok := expr.Function.(*ast.Identifier); ok {
		funcName := ident.Value
		if c.currentModulePrefix != "" {
			mangledName := fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
			fn, exists = c.functionTable[mangledName]
		}
		if !exists {
			fn, exists = c.functionTable[funcName]
		}
	}

	if memberAccess, ok := expr.Function.(*ast.MemberAccessExpression); ok {
		if moduleIdent, ok := memberAccess.Left.(*ast.Identifier); ok {
			moduleAlias := moduleIdent.Value
			funcName := memberAccess.Right.Value

			// Look up the full module path from the alias
			modulePrefix, aliasExists := c.moduleAliasMap[moduleAlias]
			if !aliasExists {
				// Fallback to the old behavior for backward compatibility
				modulePrefix = strings.ReplaceAll(moduleAlias, "::", "_")
			}

			mangledName := fmt.Sprintf("%s_%s", modulePrefix, funcName)
			fn, exists = c.functionTable[mangledName]
		}
	}

	if !exists {
		return nil, fmt.Errorf("undefined function: %s", expr.Function.String())
	}

	var args []value.Value
	for _, arg := range expr.Arguments {
		argValue, err := c.compileExpression(arg)
		if err != nil {
			return nil, err
		}
		args = append(args, argValue)
	}

	if !fn.Sig.Variadic && len(args) != len(fn.Params) {
		return nil, fmt.Errorf("function %s expects %d arguments, but %d were provided", fn.Name(), len(fn.Params), len(args))
	}

	return c.currentBlock.NewCall(fn, args...), nil
}

// compileFunctionCall compiles a call to any function (user-defined or extern)
func (c *Compiler) compileFunctionCall(expr *ast.CallExpression, fn value.Value) (value.Value, error) {
	// Compile arguments
	var args []value.Value
	for _, arg := range expr.Arguments {
		argValue, err := c.compileExpression(arg)
		if err != nil {
			return nil, err
		}

		// Retain ARC-managed arguments only if not in nogc context
		if !c.isNoGCContext && c.isARCManagedValue(argValue) {
			arcRetainFunc, ok := c.functionTable["arc_retain"]
			if ok {
				c.currentBlock.NewCall(arcRetainFunc, argValue)
			}
		}

		args = append(args, argValue)
	}

	// Cast fn to *ir.Func
	irFunc, ok := fn.(*ir.Func)
	if !ok {
		return nil, fmt.Errorf("invalid function type")
	}

	// Check if this is a variadic function call
	if irFunc.Sig.Variadic {
		// For variadic functions, we need to ensure we have at least the required number of arguments
		requiredArgs := len(irFunc.Params)
		if len(args) < requiredArgs {
			return nil, fmt.Errorf("variadic function %s requires at least %d arguments, got %d",
				irFunc.Name(), requiredArgs, len(args))
		}
	} else {
		// For non-variadic functions, check exact argument count
		if len(args) != len(irFunc.Params) {
			return nil, fmt.Errorf("function %s expects %d arguments, got %d",
				irFunc.Name(), len(irFunc.Params), len(args))
		}
	}

	// Call the function
	return c.currentBlock.NewCall(irFunc, args...), nil
}

// compilePrintCall compiles a print function call
func (c *Compiler) compilePrintCall(expr *ast.CallExpression) (value.Value, error) {
	if len(expr.Arguments) != 1 {
		return nil, fmt.Errorf("print expects exactly 1 argument, got %d", len(expr.Arguments))
	}

	arg, err := c.compileExpression(expr.Arguments[0])
	if err != nil {
		return nil, err
	}

	// Determine which print function to call based on argument type
	var funcName string
	argType := arg.Type()

	switch {
	case argType.Equal(types.I64):
		funcName = "basalt_print_int"
	case argType.Equal(types.I1):
		funcName = "basalt_print_bool"
	case argType.Equal(types.Double):
		funcName = "basalt_print_float"
	case c.isStringType(argType):
		funcName = "basalt_print_string"
	default:
		return nil, fmt.Errorf("unsupported type for print: %s", argType)
	}

	printFunc, exists := c.functionTable[funcName]
	if !exists {
		return nil, fmt.Errorf("runtime function %s not found", funcName)
	}

	// Call the print function
	c.currentBlock.NewCall(printFunc, arg)

	// Print returns void, but we need to return something for expression context
	return constant.NewInt(types.I32, 0), nil
}

// compileEnumLiteral compiles enum literal expressions (shouldn't be called directly)
func (c *Compiler) compileEnumLiteral(expr *ast.EnumLiteral) (value.Value, error) {
	// Enum literals should not be compiled directly as expressions
	// They are handled in let statements via compileEnumDefinition
	return nil, fmt.Errorf("enum literals must be assigned to a variable")
}

// compileHashLiteral compiles hash map literals like {"key": value, 42: "answer"}
func (c *Compiler) compileHashLiteral(expr *ast.HashLiteral) (value.Value, error) {
	// For now, we'll create a simple HashMap for string->int64 or int64->string mappings
	// In a full implementation, we'd need to determine the key and value types from the pairs

	// Create a new HashMap by calling the Basalt HashMap::new function
	// We need to determine the key and value types from the pairs
	if len(expr.Pairs) == 0 {
		// Empty hash map - we'll need type annotation to determine the types
		// For now, return a placeholder pointer
		return constant.NewIntToPtr(constant.NewInt(types.I64, 0), types.I8Ptr), nil
	}

	// Analyze the first pair to determine types
	firstPair := expr.Pairs[0]
	keyValue, err := c.compileExpression(firstPair.Key)
	if err != nil {
		return nil, err
	}
	valueValue, err := c.compileExpression(firstPair.Value)
	if err != nil {
		return nil, err
	}

	// Determine the HashMap type based on key and value types
	keyType := keyValue.Type()
	valType := valueValue.Type()

	// For now, we'll support string keys and int64 values, or int64 keys and string values
	var hashmapNewFunc *ir.Func
	var hashmapSetFunc *ir.Func
	var ok bool

	if c.isStringType(keyType) && valType.Equal(types.I64) {
		// HashMap<string, int64>
		hashmapNewFunc, ok = c.functionTable["std_collections_new"]
		if !ok {
			return nil, fmt.Errorf("Collections::new function not found")
		}
		hashmapSetFunc, ok = c.functionTable["std_collections_set"]
		if !ok {
			return nil, fmt.Errorf("Collections::set function not found")
		}
	} else if keyType.Equal(types.I64) && c.isStringType(valType) {
		// HashMap<int64, string>
		hashmapNewFunc, ok = c.functionTable["std_collections_new"]
		if !ok {
			return nil, fmt.Errorf("Collections::new function not found")
		}
		hashmapSetFunc, ok = c.functionTable["std_collections_set"]
		if !ok {
			return nil, fmt.Errorf("Collections::set function not found")
		}
	} else {
		return nil, fmt.Errorf("unsupported HashMap key/value types: %s -> %s", keyType, valType)
	}

	// Create a new HashMap
	hashMapPtr := c.currentBlock.NewCall(hashmapNewFunc)

	// Set the first pair
	c.currentBlock.NewCall(hashmapSetFunc, hashMapPtr, keyValue, valueValue)

	// Set the remaining pairs
	for i := 1; i < len(expr.Pairs); i++ {
		pair := expr.Pairs[i]

		keyVal, err := c.compileExpression(pair.Key)
		if err != nil {
			return nil, err
		}
		valVal, err := c.compileExpression(pair.Value)
		if err != nil {
			return nil, err
		}

		// Type check - ensure all keys and values have consistent types
		if !keyVal.Type().Equal(keyType) {
			return nil, fmt.Errorf("inconsistent key types in hash map literal: expected %s, got %s", keyType, keyVal.Type())
		}
		if !valVal.Type().Equal(valType) {
			return nil, fmt.Errorf("inconsistent value types in hash map literal: expected %s, got %s", valType, valVal.Type())
		}

		// Set the key-value pair
		c.currentBlock.NewCall(hashmapSetFunc, hashMapPtr, keyVal, valVal)
	}

	return hashMapPtr, nil
}

func unescapeString(raw string) (string, error) {
	var sb strings.Builder
	// The raw string from the parser includes the surrounding quotes,
	// so we can iterate from the second character to the second-to-last.
	// If your parser provides the string WITHOUT quotes, use `for i := 0; i < len(raw); i++`
	for i := 0; i < len(raw); i++ {
		char := raw[i]
		if char == '\\' {
			// Make sure there is a character after the backslash
			if i+1 >= len(raw) {
				return "", fmt.Errorf("invalid escape sequence at end of string")
			}
			i++ // Move to the character after '\'
			switch raw[i] {
			case 'n':
				sb.WriteRune('\n')
			case 't':
				sb.WriteRune('\t')
			case '\\':
				sb.WriteRune('\\')
			case '"':
				sb.WriteRune('"')
			// Add other escapes as needed (e.g., \r)
			default:
				// Optional: return an error for unknown escape sequences
				return "", fmt.Errorf("unknown escape sequence: \\%c", raw[i])
			}
		} else {
			sb.WriteRune(rune(char))
		}
	}
	return sb.String(), nil
}

// compileStringLiteral compiles a string literal to a global string constant
func (c *Compiler) compileStringLiteral(expr *ast.StringLiteral) (value.Value, error) {
	// 1. Un-escape the raw string value to handle sequences like \n
	processedValue, err := unescapeString(expr.Value)
	if err != nil {
		return nil, err // Propagate error if the escape sequence is invalid
	}

	// Create a global string constant
	// LLVM string constants are arrays of i8 with null terminator
	stringValue := processedValue + "\x00" // Add null terminator

	// Create character array type
	charArrayType := types.NewArray(uint64(len(stringValue)), types.I8)

	// Create the global string constant
	globalName := fmt.Sprintf("str_%d", c.blockCounter)
	c.blockCounter++

	// Create constant character array
	chars := make([]constant.Constant, len(stringValue))
	for i, char := range stringValue {
		chars[i] = constant.NewInt(types.I8, int64(char))
	}
	charArray := constant.NewArray(charArrayType, chars...)

	// Create global variable for the string
	global := c.module.NewGlobalDef(globalName, charArray)
	global.Linkage = enum.LinkagePrivate
	global.UnnamedAddr = enum.UnnamedAddrUnnamedAddr

	// Return a pointer to the first character (i8*)
	// Use GetElementPtr to get pointer to first element
	zero := constant.NewInt(types.I64, 0)
	return constant.NewGetElementPtr(charArrayType, global, zero, zero), nil
}

// basaltTypeToLLVMType converts a Basalt type to an LLVM type
func (c *Compiler) basaltTypeToLLVMType(expr ast.Expression) types.Type {
	switch expr.(type) {
	case *ast.IntegerLiteral:
		return types.I64
	case *ast.Boolean:
		return types.I1
	case *ast.FloatLiteral:
		return types.Double
	default:
		return types.I64 // Default to int64
	}
}

// typeAnnotationToLLVMType converts a type annotation to LLVM type
func (c *Compiler) typeAnnotationToLLVMType(typeAnnotation *ast.TypeAnnotation) types.Type {
	var baseType types.Type

	switch typeAnnotation.Value {
	case "int64":
		baseType = types.I64
	case "int32":
		baseType = types.I32
	case "int16":
		baseType = types.I16
	case "int8":
		baseType = types.I8
	case "bool":
		baseType = types.I1
	case "float64":
		baseType = types.Double
	case "float32":
		baseType = types.Float
	case "string":
		baseType = types.I8Ptr // String is represented as i8* (pointer to i8)
	case "rawptr":
		baseType = types.I8Ptr // rawptr is also represented as a generic i8*
	case "none":
		baseType = types.Void
	default:
		// Check if it's a struct type
		if structInfo, exists := c.typeRegistry[typeAnnotation.Value]; exists {
			baseType = structInfo.LLVMType
		} else if enumInfo, exists := c.enumRegistry[typeAnnotation.Value]; exists {
			// Check if it's an enum type
			baseType = enumInfo.LLVMType
		} else {
			// Default to i64 for unknown types
			baseType = types.I64
		}
	}

	// If it's a pointer type, wrap it in a pointer
	if typeAnnotation.IsPointer {
		return types.NewPointer(baseType)
	}

	// For struct and enum types, we always return a pointer (unless it's already a pointer type)
	if structInfo, exists := c.typeRegistry[typeAnnotation.Value]; exists && !typeAnnotation.IsPointer {
		return types.NewPointer(structInfo.LLVMType)
	}
	if enumInfo, exists := c.enumRegistry[typeAnnotation.Value]; exists && !typeAnnotation.IsPointer {
		return types.NewPointer(enumInfo.LLVMType)
	}

	return baseType
}

// CompileToExecutable compiles the program and creates an executable
func (c *Compiler) CompileToExecutable(program *ast.Program, outputPath string) error {
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

	// Link object file with runtime.c to executable using clang
	clangCmd := exec.Command("clang", objFile, "-o", outputPath, "-fsanitize=address")
	var clangStdErr bytes.Buffer
	var clangStdOut bytes.Buffer
	clangCmd.Stdout = &clangStdOut
	clangCmd.Stderr = &clangStdErr
	if err := clangCmd.Run(); err != nil {
		return fmt.Errorf("clang linking failed: %ww\n%v\n%v", err, clangStdOut.String(), clangStdErr.String())
	}

	return nil
}

// compileModuleStatement is MODIFIED for the two-pass approach
func (c *Compiler) compileModuleStatement(stmt *ast.ModuleStatement) error {
	pathSegments := make([]string, len(stmt.FullPath.Segments))
	for i, segment := range stmt.FullPath.Segments {
		pathSegments[i] = segment.Value
	}
	prefix := strings.Join(pathSegments, "_")

	// Register the module alias mapping
	moduleAlias := stmt.Name.Value
	c.moduleAliasMap[moduleAlias] = prefix

	savedPrefix := c.currentModulePrefix
	c.currentModulePrefix = prefix

	// PASS 1: Collect all declarations within the module.
	for _, modStmt := range stmt.Module.Statements {
		if err := c.collectDeclarations(modStmt); err != nil {
			return err
		}
	}

	// PASS 2: Compile all implementations within the module.
	for _, modStmt := range stmt.Module.Statements {
		if err := c.compileImplementation(modStmt); err != nil {
			return err
		}
	}

	c.currentModulePrefix = savedPrefix
	return nil
}

func (c *Compiler) compileImplementation(stmt ast.Statement) error {
	switch s := stmt.(type) {
	case *ast.ModuleStatement:
		// Modules are self-contained; their implementation was handled in collectDeclarations.
		return nil

	case *ast.LetStatement:
		// If it's a function, now we compile its body.
		if funcLit, ok := s.Value.(*ast.FunctionLiteral); ok {
			return c.compileFunctionBody(s.Name.Value, funcLit)
		}
		// Struct/Enum definitions have no "body" to compile, so we skip them.
		if _, ok := s.Value.(*ast.StructLiteral); ok {
			return nil
		}
		if _, ok := s.Value.(*ast.EnumLiteral); ok {
			return nil
		}
		// Handle regular variable assignments.
		return c.compileLetAssignment(s)

	case *ast.ExpressionStatement:
		_, err := c.compileExpression(s.Expression)
		return err

	case *ast.ReturnStatement:
		// Return statements only appear inside function bodies, which are handled
		// by compileFunctionBody, so we shouldn't see them at the top level.
		return nil

	case *ast.ExternStatement:
		// Already handled in Pass 1.
		return nil
	case *ast.UnsafeStatement:
		return c.compileUnsafeStatement(s)
	default:
		return fmt.Errorf("unsupported implementation statement type: %T", stmt)
	}
}

func (c *Compiler) collectDeclarations(stmt ast.Statement) error {
	switch s := stmt.(type) {
	case *ast.ModuleStatement:
		// Recursively collect declarations from sub-modules
		return c.compileModuleStatement(s)

	case *ast.LetStatement:
		// Find function, struct, and enum declarations
		if funcLit, ok := s.Value.(*ast.FunctionLiteral); ok {
			return c.compileFunctionDeclaration(s.Name.Value, funcLit)
		}
		if structLit, ok := s.Value.(*ast.StructLiteral); ok {
			return c.compileStructDefinition(s.Name.Value, structLit)
		}
		if enumLit, ok := s.Value.(*ast.EnumLiteral); ok {
			return c.compileEnumDefinition(s.Name.Value, enumLit)
		}
		// Other let statements are implementations, ignore in this pass.
		return nil

	case *ast.ExternStatement:
		// Externs are pure declarations.
		return c.compileExternStatement(s)

	default:
		// All other statements are implementations, ignore in this pass.
		return nil
	}
}

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
	return nil
}

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
		alloca := c.currentBlock.NewAlloca(fn.Params[i].Typ)
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

func (c *Compiler) compileLetAssignment(stmt *ast.LetStatement) error {
	val, err := c.compileExpression(stmt.Value)
	if err != nil {
		return err
	}

	// Special handling for the underscore identifier - discard the value
	if stmt.Name.Value == "_" {
		// Just evaluate the expression for side effects, don't store it
		return nil
	}

	var targetType types.Type
	if stmt.Type != nil {
		targetType = c.typeAnnotationToLLVMType(stmt.Type)
		// The checker has already approved this assignment. If types don't match,
		// it must be a valid cast (e.g., int64 to rawptr).
		if !val.Type().Equal(targetType) {
			if val.Type().Equal(types.I64) && targetType.Equal(types.I8Ptr) {
				// Convert int64 to rawptr using inttoptr
				val = c.currentBlock.NewIntToPtr(val, targetType)
			} else {
				val = c.currentBlock.NewBitCast(val, targetType)
			}
		}
	} else {
		targetType = val.Type()
	}

	alloca := c.currentBlock.NewAlloca(targetType)
	c.currentBlock.NewStore(val, alloca)
	c.symbolTable[stmt.Name.Value] = alloca
	return nil
}

// compileFunctionDefinition compiles a function definition (let funcName = fn(...) {...})
func (c *Compiler) compileFunctionDefinition(funcName string, expr *ast.FunctionLiteral) error {
	llvmFuncName := funcName
	// Mangle the function name if inside a module
	if c.currentModulePrefix != "" {
		llvmFuncName = fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
	}

	var paramTypes []types.Type
	for _, param := range expr.Parameters {
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
		alloca := c.currentBlock.NewAlloca(paramTypes[i])
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

// compileArrayLiteral compiles an array literal [1, 2, 3]
func (c *Compiler) compileArrayLiteral(expr *ast.ArrayLiteral) (value.Value, error) {
	// Create a new array with initial capacity equal to the number of elements
	// For empty arrays, use a default capacity of 0
	initialCapacity := constant.NewInt(types.I64, int64(len(expr.Elements)))
	arrayNewFunc, ok := c.functionTable["basalt_array_new"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_new not found")
	}
	arrayPtr := c.currentBlock.NewCall(arrayNewFunc, initialCapacity)

	// Push each element to the array
	arrayPushFunc, ok := c.functionTable["basalt_array_push"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_push not found")
	}
	for _, element := range expr.Elements {
		elementValue, err := c.compileExpression(element)
		if err != nil {
			return nil, err
		}

		// For now, only support integer elements
		if elementValue.Type() != types.I64 {
			return nil, fmt.Errorf("array elements must be integers, got %s", elementValue.Type())
		}

		c.currentBlock.NewCall(arrayPushFunc, arrayPtr, elementValue)
	}

	return arrayPtr, nil
}

// compileIndexExpression compiles array indexing arr[index]
func (c *Compiler) compileIndexExpression(expr *ast.IndexExpression) (value.Value, error) {
	// Compile the left expression (array or hashmap)
	leftValue, err := c.compileExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	// For now, only support simple indexing (not slicing)
	if expr.IsSlice {
		return nil, fmt.Errorf("array slicing not yet implemented")
	}

	// Compile the index expression
	indexValue, err := c.compileExpression(expr.Start)
	if err != nil {
		return nil, err
	}

	// Check if this is a HashMap access by looking at the left expression type
	// For now, we'll assume any pointer type that's not an array is a HashMap
	// In a full implementation, we'd need better type information
	leftType := leftValue.Type()

	// If the left side is a pointer and the index is not an integer, it might be a HashMap
	if c.isPointerType(leftType) && !indexValue.Type().Equal(types.I64) {
		// This is likely a HashMap with non-integer keys
		hashmapGetFunc, ok := c.functionTable["std_collections_get"]
		if !ok {
			return nil, fmt.Errorf("Collections::get function not found")
		}
		return c.currentBlock.NewCall(hashmapGetFunc, leftValue, indexValue), nil
	}

	// If the left side is a pointer and the index is an integer, it could be either
	// We'll try HashMap first, then fall back to array
	if c.isPointerType(leftType) && indexValue.Type().Equal(types.I64) {
		// Try HashMap first
		hashmapGetFunc, ok := c.functionTable["std_collections_get"]
		if ok {
			// Check if this looks like a HashMap by trying to access it
			// For now, we'll assume it's a HashMap if the function exists
			return c.currentBlock.NewCall(hashmapGetFunc, leftValue, indexValue), nil
		}

		// Fall back to array access
		arrayGetFunc, ok := c.functionTable["basalt_array_get"]
		if ok {
			return c.currentBlock.NewCall(arrayGetFunc, leftValue, indexValue), nil
		}

		return nil, fmt.Errorf("neither Collections::get nor basalt_array_get functions found")
	}

	// For non-pointer types, this should be an array access
	if indexValue.Type() != types.I64 {
		return nil, fmt.Errorf("array index must be integer, got %s", indexValue.Type())
	}

	// Call basalt_array_get
	arrayGetFunc, ok := c.functionTable["basalt_array_get"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_get not found")
	}
	return c.currentBlock.NewCall(arrayGetFunc, leftValue, indexValue), nil
}

// compileMemberAccessExpression compiles member access like arr.len or struct.field
func (c *Compiler) compileMemberAccessExpression(expr *ast.MemberAccessExpression) (value.Value, error) {
	// Compile the object being accessed
	objectValue, err := c.compileExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	memberName := expr.Right.Value

	// Check if this is array.len - call basalt_array_len directly
	if memberName == "len" {
		// Check if objectValue is an array (array_ptr type)
		if objectValue.Type().Equal(types.I8Ptr) {
			// Call basalt_array_len
			arrayLenFunc, ok := c.functionTable["basalt_array_len"]
			if !ok {
				return nil, fmt.Errorf("runtime function basalt_array_len not found")
			}
			return c.currentBlock.NewCall(arrayLenFunc, objectValue), nil
		}
		// If it's a string, we could also support string.len here
		if c.isStringType(objectValue.Type()) {
			// Call strlen
			strlenFunc, ok := c.functionTable["strlen"]
			if !ok {
				return nil, fmt.Errorf("strlen function not found")
			}
			return c.currentBlock.NewCall(strlenFunc, objectValue), nil
		}
	}

	// Check if this is struct field access
	if structPtr, ok := objectValue.Type().(*types.PointerType); ok {
		if structType, ok := structPtr.ElemType.(*types.StructType); ok {
			// Find the struct info for this type
			for _, structInfo := range c.typeRegistry {
				if structInfo.LLVMType == structType {
					// Check if the field exists
					fieldIndex, exists := structInfo.FieldIndex[memberName]
					if !exists {
						return nil, fmt.Errorf("field '%s' not found in struct", memberName)
					}

					// Get pointer to the field using GEP
					zero := constant.NewInt(types.I32, 0)
					fieldIdx := constant.NewInt(types.I32, int64(fieldIndex))
					fieldPtr := c.currentBlock.NewGetElementPtr(structType, objectValue, zero, fieldIdx)

					// Load the value from the field
					return c.currentBlock.NewLoad(structInfo.FieldTypes[memberName], fieldPtr), nil
				}
			}
		}
	}

	return nil, fmt.Errorf("unsupported member access: %s", memberName)
}

// compileStructDefinition compiles a struct definition (let Point = struct { x: int64, y: int64 })
func (c *Compiler) compileStructDefinition(structName string, expr *ast.StructLiteral) error {
	// Create ordered list of field names and types
	fieldNames := make([]string, len(expr.Fields))
	fieldTypes := make(map[string]types.Type)
	fieldIndex := make(map[string]int)
	llvmFieldTypes := make([]types.Type, len(expr.Fields))

	for i, field := range expr.Fields {
		fieldName := field.Name.Value
		fieldTypeName := field.Type.Value

		// Convert Basalt type to LLVM type
		var llvmType types.Type
		switch fieldTypeName {
		case "int64":
			llvmType = types.I64
		case "bool":
			llvmType = types.I1
		case "float64":
			llvmType = types.Double
		case "string":
			llvmType = types.I8Ptr
		default:
			return fmt.Errorf("unsupported field type: %s", fieldTypeName)
		}

		fieldNames[i] = fieldName
		fieldTypes[fieldName] = llvmType
		fieldIndex[fieldName] = i
		llvmFieldTypes[i] = llvmType
	}

	// Create struct type
	structType := types.NewStruct(llvmFieldTypes...)

	// Create named struct type
	c.module.NewTypeDef(structName, structType)

	// Store struct information in type registry
	c.typeRegistry[structName] = &StructInfo{
		LLVMType:   structType,
		FieldNames: fieldNames,
		FieldTypes: fieldTypes,
		FieldIndex: fieldIndex,
	}

	return nil
}

// compileStructLiteral compiles a struct literal (for standalone struct definitions)
func (c *Compiler) compileStructLiteral(expr *ast.StructLiteral) (value.Value, error) {
	// This shouldn't be called directly as struct literals are handled in let statements
	return nil, fmt.Errorf("struct literals must be assigned to a variable")
}

// compileStructInstanceExpression compiles struct instantiation (Point { x: 10, y: 20 })
func (c *Compiler) compileStructInstanceExpression(expr *ast.StructInstanceExpression) (value.Value, error) {
	// Get the struct type name from the left side (should be an identifier)
	structIdent, ok := expr.StructExpr.(*ast.Identifier)
	if !ok {
		return nil, fmt.Errorf("struct instantiation requires a struct type name")
	}

	structName := structIdent.Value

	// Look up the struct info
	structInfo, exists := c.typeRegistry[structName]
	if !exists {
		return nil, fmt.Errorf("undefined struct type: %s", structName)
	}

	// Allocate stack memory for the struct
	structPtr := c.currentBlock.NewAlloca(structInfo.LLVMType)

	// Store field values
	for fieldName, fieldExpr := range expr.Fields {
		// Check if field exists
		fieldIndex, exists := structInfo.FieldIndex[fieldName]
		if !exists {
			return nil, fmt.Errorf("field '%s' not found in struct %s", fieldName, structName)
		}

		// Compile the field value
		fieldValue, err := c.compileExpression(fieldExpr)
		if err != nil {
			return nil, err
		}

		// Type check
		expectedType := structInfo.FieldTypes[fieldName]
		if !fieldValue.Type().Equal(expectedType) {
			return nil, fmt.Errorf("field '%s' expected type %s, got %s", fieldName, expectedType, fieldValue.Type())
		}

		// Get pointer to the field using GEP
		zero := constant.NewInt(types.I32, 0)
		fieldIdx := constant.NewInt(types.I32, int64(fieldIndex))
		fieldPtr := c.currentBlock.NewGetElementPtr(structInfo.LLVMType, structPtr, zero, fieldIdx)

		// Store the value
		c.currentBlock.NewStore(fieldValue, fieldPtr)
	}

	// Check that all fields are provided
	if len(expr.Fields) != len(structInfo.FieldNames) {
		return nil, fmt.Errorf("struct instantiation missing fields")
	}

	return structPtr, nil
}

// compileStructFieldAssignment compiles assignment to a struct field (e.g., obj.field = value)
func (c *Compiler) compileStructFieldAssignment(memberAccess *ast.MemberAccessExpression, value value.Value) (value.Value, error) {
	// Compile the object being accessed
	objectValue, err := c.compileExpression(memberAccess.Left)
	if err != nil {
		return nil, err
	}

	memberName := memberAccess.Right.Value

	// Check if this is struct field access
	if structPtr, ok := objectValue.Type().(*types.PointerType); ok {
		if structType, ok := structPtr.ElemType.(*types.StructType); ok {
			// Find the struct info for this type
			for _, structInfo := range c.typeRegistry {
				if structInfo.LLVMType == structType {
					// Check if the field exists
					fieldIndex, exists := structInfo.FieldIndex[memberName]
					if !exists {
						return nil, fmt.Errorf("field '%s' not found in struct", memberName)
					}

					// Get pointer to the field using GEP
					zero := constant.NewInt(types.I32, 0)
					fieldIdx := constant.NewInt(types.I32, int64(fieldIndex))
					fieldPtr := c.currentBlock.NewGetElementPtr(structType, objectValue, zero, fieldIdx)

					// Type check - ensure the value type matches the field type
					expectedType := structInfo.FieldTypes[memberName]
					if !value.Type().Equal(expectedType) {
						return nil, fmt.Errorf("cannot assign %s to field '%s' of type %s", value.Type(), memberName, expectedType)
					}

					// Store the value into the field
					c.currentBlock.NewStore(value, fieldPtr)

					// Return the assigned value
					return value, nil
				}
			}
		}
	}

	return nil, fmt.Errorf("cannot assign to field '%s' on non-struct type", memberName)
}

// compileIndexAssignment compiles assignment to an index expression (e.g., map[key] = value or arr[index] = value)
func (c *Compiler) compileIndexAssignment(indexExpr *ast.IndexExpression, value value.Value) (value.Value, error) {
	// Compile the left expression (map or array)
	leftValue, err := c.compileExpression(indexExpr.Left)
	if err != nil {
		return nil, err
	}

	// Compile the index/key expression
	keyValue, err := c.compileExpression(indexExpr.Start)
	if err != nil {
		return nil, err
	}

	// Check if this is a HashMap assignment
	leftType := leftValue.Type()

	// For HashMap assignment, we need to call Collections::set
	if c.isPointerType(leftType) {
		// Try HashMap set first
		hashmapSetFunc, ok := c.functionTable["std_collections_set"]
		if ok {
			// Call Collections::set(map, key, value)
			c.currentBlock.NewCall(hashmapSetFunc, leftValue, keyValue, value)
			return value, nil
		}

		// If Collections::set is not available, this might be array assignment
		// For now, we don't support array element assignment
		return nil, fmt.Errorf("array element assignment not yet implemented")
	}

	return nil, fmt.Errorf("unsupported index assignment target type: %s", leftType)
}

func (c *Compiler) compileImportStatement(stmt *ast.ImportStatement) error {
	// Import statements are handled at the module resolution level
	// The actual module code is already included in the program
	// So we just need to create a module namespace entry

	// For now, we don't need to do anything special in the compiler
	// The module functions are already compiled as part of the program
	return nil
}

func (c *Compiler) compileUnsafeStatement(stmt *ast.UnsafeStatement) error {
	// Unsafe blocks are just regular blocks from the compiler's perspective
	// The type checker has already enforced the safety rules
	_, err := c.compileBlockStatement(stmt.Body)
	return err
}

func (c *Compiler) compileExternStatement(stmt *ast.ExternStatement) error {
	funcName := stmt.Function.Value

	// Check if the function is already in the function table to avoid redefinition
	if _, exists := c.functionTable[funcName]; exists {
		return nil // Already declared, so we can skip it.
	}

	// 1. Create a slice to hold the LLVM parameter objects (*ir.Param).
	var llvmParams []*ir.Param

	// 2. Loop through the parameters from the AST.
	for _, p := range stmt.Parameters {
		// For each parameter, get its name and LLVM type.
		paramName := p.Name.Value
		paramType := c.typeAnnotationToLLVMType(p.Type)

		// 3. Create a single LLVM parameter with ir.NewParam and add it to the slice.
		llvmParams = append(llvmParams, ir.NewParam(paramName, paramType))
	}

	// 4. Get the return type.
	returnType := c.typeAnnotationToLLVMType(stmt.ReturnType)

	// 5. Declare the function using the correctly constructed slice of parameters.
	// The '...' unpacks the llvmParams slice for the variadic function call.
	fn := c.module.NewFunc(funcName, returnType, llvmParams...)

	// 6. Set the Variadic property if this is a variadic function
	if stmt.IsVariadic {
		fn.Sig.Variadic = true
	}

	c.functionTable[funcName] = fn // Register it for calls

	// Also register with module prefix if we're inside a module
	// This allows the function to be called as Module.function_name
	if c.currentModulePrefix != "" {
		mangledName := fmt.Sprintf("%s_%s", c.currentModulePrefix, funcName)
		c.functionTable[mangledName] = fn
	}

	return nil
}

// compileEnumDefinition compiles enum definitions and registers them in the enum registry
func (c *Compiler) compileEnumDefinition(enumName string, enumLit *ast.EnumLiteral) error {
	// Calculate the maximum data size needed for any variant
	maxDataSize := 0
	variants := make(map[string]*EnumVariantInfo)
	tagToName := make(map[int32]string)

	for i, variant := range enumLit.Variants {
		tag := int32(i)
		variantName := variant.Name.Value

		variantInfo := &EnumVariantInfo{
			Tag:         tag,
			PayloadType: nil,
		}

		if variant.Payload != nil {
			payloadType := c.typeAnnotationToLLVMType(variant.Payload)
			variantInfo.PayloadType = payloadType

			// Calculate size (simplified - just use 8 bytes for all types for now)
			size := 8
			if size > maxDataSize {
				maxDataSize = size
			}
		}

		variants[variantName] = variantInfo
		tagToName[tag] = variantName
	}

	// Create the enum struct type: { i32 tag, [N x i8] data }
	dataArrayType := types.NewArray(uint64(maxDataSize), types.I8)
	enumStructType := types.NewStruct(types.I32, dataArrayType)

	// Create named type
	c.module.NewTypeDef(enumName, enumStructType)

	// Register in enum registry
	c.enumRegistry[enumName] = &EnumInfo{
		LLVMType:    enumStructType,
		Variants:    variants,
		TagToName:   tagToName,
		MaxDataSize: maxDataSize,
	}

	return nil
}

// compileEnumInstantiationExpression compiles enum instantiation (Option::Some(42))
func (c *Compiler) compileEnumInstantiationExpression(expr *ast.EnumInstantiationExpression) (value.Value, error) {
	enumName := expr.Enum.Segments[0].Value
	variantName := expr.Variant.Value

	// Look up the enum info
	enumInfo, exists := c.enumRegistry[enumName]
	if !exists {
		return nil, fmt.Errorf("undefined enum type: %s", enumName)
	}

	// Look up the variant info
	variantInfo, exists := enumInfo.Variants[variantName]
	if !exists {
		return nil, fmt.Errorf("undefined variant: %s::%s", enumName, variantName)
	}

	// Allocate stack memory for the enum
	enumPtr := c.currentBlock.NewAlloca(enumInfo.LLVMType)

	// Store the tag
	zero := constant.NewInt(types.I32, 0)
	tagIdx := constant.NewInt(types.I32, 0)
	tagPtr := c.currentBlock.NewGetElementPtr(enumInfo.LLVMType, enumPtr, zero, tagIdx)
	tag := constant.NewInt(types.I32, int64(variantInfo.Tag))
	c.currentBlock.NewStore(tag, tagPtr)

	// Store the payload if present
	if variantInfo.PayloadType != nil && len(expr.Arguments) > 0 {
		// Compile the argument
		argValue, err := c.compileExpression(expr.Arguments[0])
		if err != nil {
			return nil, err
		}

		// Get pointer to the data field
		dataIdx := constant.NewInt(types.I32, 1)
		dataPtr := c.currentBlock.NewGetElementPtr(enumInfo.LLVMType, enumPtr, zero, dataIdx)

		// Cast the data pointer to the correct type
		payloadPtrType := types.NewPointer(variantInfo.PayloadType)
		castedDataPtr := c.currentBlock.NewBitCast(dataPtr, payloadPtrType)

		// Store the payload
		c.currentBlock.NewStore(argValue, castedDataPtr)
	}

	return enumPtr, nil
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
				patternVar := c.currentBlock.NewAlloca(variantInfo.PayloadType)
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
