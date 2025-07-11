package compiler

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"

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

// Compiler holds the LLVM IR module and compilation state
type Compiler struct {
	module        *ir.Module
	currentFunc   *ir.Func
	currentBlock  *ir.Block
	symbolTable   map[string]value.Value // Maps variable names to their allocated stack pointers
	functionTable map[string]*ir.Func    // Maps function names to their IR functions
	typeRegistry  map[string]*StructInfo // Maps struct names to their type information
	env           *checker.TypeEnvironment
	externalFuncs map[string]*ir.Func // Cache for external function declarations
	blockCounter  int                 // Counter for generating unique block names
}

// New creates a new compiler instance
func New() *Compiler {
	return &Compiler{
		module:        ir.NewModule(),
		symbolTable:   make(map[string]value.Value),
		functionTable: make(map[string]*ir.Func),
		typeRegistry:  make(map[string]*StructInfo),
		env:           checker.NewTypeEnvironment(),
		externalFuncs: make(map[string]*ir.Func),
		blockCounter:  0,
	}
}

// declareExternalFunction declares an external C function
func (c *Compiler) declareExternalFunction(name string, returnType types.Type, paramTypes ...types.Type) *ir.Func {
	if fn, exists := c.externalFuncs[name]; exists {
		return fn
	}

	// Create parameters
	var params []*ir.Param
	for i, paramType := range paramTypes {
		params = append(params, ir.NewParam(fmt.Sprintf("arg%d", i), paramType))
	}

	fn := c.module.NewFunc(name, returnType, params...)
	c.externalFuncs[name] = fn
	return fn
}

// Compile compiles the AST program to LLVM IR
func (c *Compiler) Compile(program *ast.Program) (*ir.Module, error) {
	// Declare external functions
	c.declareExternalFunction("basalt_print_int", types.Void, types.I64)
	c.declareExternalFunction("basalt_print_bool", types.Void, types.I1)
	c.declareExternalFunction("basalt_print_float", types.Void, types.Double)
	c.declareExternalFunction("basalt_print_string", types.Void, types.I8Ptr)
	c.declareExternalFunction("basalt_string_concat", types.I8Ptr, types.I8Ptr, types.I8Ptr)
	c.declareExternalFunction("basalt_string_equals", types.I32, types.I8Ptr, types.I8Ptr)

	// Declare array runtime functions
	arrayPtrType := types.NewPointer(types.I8) // BasaltArray* represented as i8*
	c.declareExternalFunction("basalt_array_new", arrayPtrType, types.I64)
	c.declareExternalFunction("basalt_array_push", types.Void, arrayPtrType, types.I64)
	c.declareExternalFunction("basalt_array_get", types.I64, arrayPtrType, types.I64)
	c.declareExternalFunction("basalt_array_len", types.I64, arrayPtrType)

	// Create main function
	mainFunc := c.module.NewFunc("main", types.I32)
	c.currentFunc = mainFunc

	// Create entry block
	entryBlock := mainFunc.NewBlock("")
	c.currentBlock = entryBlock

	// Compile all statements in the program
	for _, stmt := range program.Statements {
		err := c.compileStatement(stmt)
		if err != nil {
			return nil, err
		}
	}

	// Add return 0 at the end of main if no explicit return
	c.currentBlock.NewRet(constant.NewInt(types.I32, 0))

	return c.module, nil
}

// compileStatement compiles a statement
func (c *Compiler) compileStatement(stmt ast.Statement) error {
	switch s := stmt.(type) {
	case *ast.LetStatement:
		return c.compileLetStatement(s)
	case *ast.ExpressionStatement:
		_, err := c.compileExpression(s.Expression)
		return err
	case *ast.ReturnStatement:
		return c.compileReturnStatement(s)
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

	// Regular variable assignment
	// Compile the right-hand side expression first to get its type
	value, err := c.compileExpression(stmt.Value)
	if err != nil {
		return err
	}

	// Use the actual type of the compiled value
	llvmType := value.Type()

	// Allocate stack space for the variable
	alloca := c.currentBlock.NewAlloca(llvmType)

	// Store the value in the allocated space
	c.currentBlock.NewStore(value, alloca)

	// Add to symbol table
	c.symbolTable[stmt.Name.Value] = alloca

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
		return nil, fmt.Errorf("if branches return different types: %s vs %s", thenValue.Type(), elseValue.Type())
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
	var lastValue value.Value = constant.NewInt(types.I64, 0) // Default value

	for _, stmt := range block.Statements {
		switch s := stmt.(type) {
		case *ast.ExpressionStatement:
			val, err := c.compileExpression(s.Expression)
			if err != nil {
				return nil, err
			}
			lastValue = val
		case *ast.LetStatement:
			err := c.compileLetStatement(s)
			if err != nil {
				return nil, err
			}
			// Let statements don't produce values
		case *ast.ReturnStatement:
			return nil, c.compileReturnStatement(s)
		default:
			return nil, fmt.Errorf("unsupported statement type in block: %T", stmt)
		}
	}

	return lastValue, nil
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
		// Check if both operands are strings (pointers to i8)
		leftType := left.Type()
		rightType := right.Type()

		if c.isStringType(leftType) && c.isStringType(rightType) {
			// String concatenation
			concatFunc := c.externalFuncs["basalt_string_concat"]
			return c.currentBlock.NewCall(concatFunc, left, right), nil
		} else {
			// Regular arithmetic addition
			return c.currentBlock.NewAdd(left, right), nil
		}
	case "-":
		return c.currentBlock.NewSub(left, right), nil
	case "*":
		return c.currentBlock.NewMul(left, right), nil
	case "/":
		return c.currentBlock.NewSDiv(left, right), nil
	case "==":
		// Check if both operands are strings
		leftType := left.Type()
		rightType := right.Type()

		if c.isStringType(leftType) && c.isStringType(rightType) {
			// String comparison
			equalsFunc := c.externalFuncs["basalt_string_equals"]
			result := c.currentBlock.NewCall(equalsFunc, left, right)
			// Convert i32 result to i1 (boolean)
			zero := constant.NewInt(types.I32, 0)
			return c.currentBlock.NewICmp(enum.IPredNE, result, zero), nil
		} else {
			// Regular integer comparison
			return c.currentBlock.NewICmp(enum.IPredEQ, left, right), nil
		}
	case "!=":
		// Check if both operands are strings
		leftType := left.Type()
		rightType := right.Type()

		if c.isStringType(leftType) && c.isStringType(rightType) {
			// String comparison
			equalsFunc := c.externalFuncs["basalt_string_equals"]
			result := c.currentBlock.NewCall(equalsFunc, left, right)
			// Convert i32 result to i1 (boolean) and negate
			zero := constant.NewInt(types.I32, 0)
			return c.currentBlock.NewICmp(enum.IPredEQ, result, zero), nil
		} else {
			// Regular integer comparison
			return c.currentBlock.NewICmp(enum.IPredNE, left, right), nil
		}
	case "<":
		return c.currentBlock.NewICmp(enum.IPredSLT, left, right), nil
	case ">":
		return c.currentBlock.NewICmp(enum.IPredSGT, left, right), nil
	case "=":
		// Assignment operator - left should be an identifier or member access
		if ident, ok := expr.Left.(*ast.Identifier); ok {
			// Look up the variable in symbol table
			ptr, exists := c.symbolTable[ident.Value]
			if !exists {
				return nil, fmt.Errorf("undefined variable: %s", ident.Value)
			}
			// Store the right-hand side value into the variable
			c.currentBlock.NewStore(right, ptr)
			// Return the assigned value
			return right, nil
		} else if memberAccess, ok := expr.Left.(*ast.MemberAccessExpression); ok {
			// Handle struct field assignment
			return c.compileStructFieldAssignment(memberAccess, right)
		}
		return nil, fmt.Errorf("assignment target must be an identifier or struct field")
	default:
		return nil, fmt.Errorf("unsupported operator: %s", expr.Operator)
	}
}

// isStringType checks if a type is a string (pointer to i8)
func (c *Compiler) isStringType(t types.Type) bool {
	if ptrType, ok := t.(*types.PointerType); ok {
		return ptrType.ElemType == types.I8
	}
	return false
}

// compileCallExpression compiles a function call
func (c *Compiler) compileCallExpression(expr *ast.CallExpression) (value.Value, error) {
	// Handle built-in print function
	if ident, ok := expr.Function.(*ast.Identifier); ok {
		if ident.Value == "print" {
			return c.compilePrintCall(expr)
		}

		// Handle user-defined function calls
		if fn, exists := c.functionTable[ident.Value]; exists {
			return c.compileUserFunctionCall(expr, fn)
		}
	}

	// Handle member access expressions like arr.len()
	if memberAccess, ok := expr.Function.(*ast.MemberAccessExpression); ok {
		if memberAccess.Right.Value == "len" {
			// This is a .len() call
			arrayValue, err := c.compileExpression(memberAccess.Left)
			if err != nil {
				return nil, err
			}

			// Verify no arguments are passed to len()
			if len(expr.Arguments) != 0 {
				return nil, fmt.Errorf("len() expects no arguments, got %d", len(expr.Arguments))
			}

			// Call basalt_array_len
			arrayLenFunc := c.externalFuncs["basalt_array_len"]
			return c.currentBlock.NewCall(arrayLenFunc, arrayValue), nil
		}
	}

	return nil, fmt.Errorf("undefined function: %v", expr.Function)
}

// compileUserFunctionCall compiles a call to a user-defined function
func (c *Compiler) compileUserFunctionCall(expr *ast.CallExpression, fn value.Value) (value.Value, error) {
	// Compile arguments
	var args []value.Value
	for _, arg := range expr.Arguments {
		argValue, err := c.compileExpression(arg)
		if err != nil {
			return nil, err
		}
		args = append(args, argValue)
	}

	// Cast fn to *ir.Func
	irFunc, ok := fn.(*ir.Func)
	if !ok {
		return nil, fmt.Errorf("invalid function type")
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
	var printFunc *ir.Func
	argType := arg.Type()

	switch argType {
	case types.I64:
		printFunc = c.externalFuncs["basalt_print_int"]
	case types.I1:
		printFunc = c.externalFuncs["basalt_print_bool"]
	case types.Double:
		printFunc = c.externalFuncs["basalt_print_float"]
	default:
		// Check if it's a pointer to i8 (string)
		if ptrType, ok := argType.(*types.PointerType); ok {
			if ptrType.ElemType == types.I8 {
				printFunc = c.externalFuncs["basalt_print_string"]
			} else {
				return nil, fmt.Errorf("unsupported pointer type for print: %s", argType)
			}
		} else {
			return nil, fmt.Errorf("unsupported type for print: %s", argType)
		}
	}

	// Call the print function
	c.currentBlock.NewCall(printFunc, arg)

	// Print returns void, but we need to return something for expression context
	return constant.NewInt(types.I32, 0), nil
}

// compileStringLiteral compiles a string literal to a global string constant
func (c *Compiler) compileStringLiteral(expr *ast.StringLiteral) (value.Value, error) {
	// Create a global string constant
	// LLVM string constants are arrays of i8 with null terminator
	stringValue := expr.Value + "\x00" // Add null terminator

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
	switch typeAnnotation.Value {
	case "int64":
		return types.I64
	case "bool":
		return types.I1
	case "float64":
		return types.Double
	case "string":
		return types.I8Ptr // String is represented as i8* (pointer to i8)
	case "none":
		return types.Void
	default:
		return types.I64 // Default to i64
	}
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
	if err := os.WriteFile(irFile, []byte(irContent), 0644); err != nil {
		return fmt.Errorf("failed to write IR file: %w", err)
	}

	// Also write IR to debug.ll for inspection
	debugFile := "debug.ll"
	if err := os.WriteFile(debugFile, []byte(irContent), 0644); err != nil {
		// Don't fail if we can't write debug file
		fmt.Printf("Warning: couldn't write debug file: %v\n", err)
	}

	// Compile IR to object file using llc
	objFile := filepath.Join(tempDir, "output.o")
	llcCmd := exec.Command("llc", "-filetype=obj", irFile, "-o", objFile)
	if err := llcCmd.Run(); err != nil {
		return fmt.Errorf("llc compilation failed: %w", err)
	}

	// Link object file with runtime.c to executable using clang
	clangCmd := exec.Command("clang", objFile, "runtime.c", "-o", outputPath)
	if err := clangCmd.Run(); err != nil {
		return fmt.Errorf("clang linking failed: %w", err)
	}

	return nil
}

// compileFunctionDefinition compiles a function definition (let funcName = fn(...) {...})
func (c *Compiler) compileFunctionDefinition(funcName string, expr *ast.FunctionLiteral) error {
	// Generate LLVM function name
	llvmFuncName := fmt.Sprintf("func_%s_%d", funcName, c.blockCounter)
	c.blockCounter++

	// Determine parameter types
	var paramTypes []types.Type
	for _, param := range expr.Parameters {
		if param.Type == nil {
			return fmt.Errorf("function parameter %s must have type annotation", param.Name.Value)
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
	fn := c.module.NewFunc(llvmFuncName, returnType, params...)

	// Add to function table
	c.functionTable[funcName] = fn

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
		return err
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

	return nil
}

// compileArrayLiteral compiles an array literal [1, 2, 3]
func (c *Compiler) compileArrayLiteral(expr *ast.ArrayLiteral) (value.Value, error) {
	// Create a new array with initial capacity equal to the number of elements
	// For empty arrays, use a default capacity of 0
	initialCapacity := constant.NewInt(types.I64, int64(len(expr.Elements)))
	arrayNewFunc := c.externalFuncs["basalt_array_new"]
	arrayPtr := c.currentBlock.NewCall(arrayNewFunc, initialCapacity)

	// Push each element to the array
	arrayPushFunc := c.externalFuncs["basalt_array_push"]
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
	// Compile the array expression
	arrayValue, err := c.compileExpression(expr.Left)
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

	// Index must be an integer
	if indexValue.Type() != types.I64 {
		return nil, fmt.Errorf("array index must be integer, got %s", indexValue.Type())
	}

	// Call basalt_array_get
	arrayGetFunc := c.externalFuncs["basalt_array_get"]
	return c.currentBlock.NewCall(arrayGetFunc, arrayValue, indexValue), nil
}

// compileMemberAccessExpression compiles member access like arr.len()
func (c *Compiler) compileMemberAccessExpression(expr *ast.MemberAccessExpression) (value.Value, error) {
	// Compile the object being accessed
	objectValue, err := c.compileExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	memberName := expr.Right.Value

	// Check if this is array.len() - we need to handle this specially since len() is a method call
	// For now, we'll treat arr.len as a special case that returns a function-like value
	// that can be called with no arguments
	if memberName == "len" {
		// We need to return something that can be called
		// For now, let's return a special marker that the call expression handler can recognize
		// This is a bit of a hack, but it works for our current needs
		return objectValue, nil // Return the array itself, CallExpression will handle len() specially
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
