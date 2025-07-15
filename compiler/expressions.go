package compiler

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/enum"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

// compileExpression compiles an expression and returns its value
func (c *Compiler) compileExpression(expr ast.Expression) (value.Value, error) {
	switch e := expr.(type) {
	case *ast.IntegerLiteral:
		return c.compileIntegerLiteral(e)
	case *ast.Boolean:
		return c.compileBoolean(e)
	case *ast.FloatLiteral:
		return c.compileFloatLiteral(e)
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

// compileCallExpression compiles a function call
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
		alloca := c.createEntryAlloca(paramTypes[i])
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

	// Get GC_malloc function
	mallocFunc, ok := c.functionTable["GC_malloc"]
	if !ok {
		return nil, fmt.Errorf("GC_malloc function not found. Ensure it is declared via extern")
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

	// Allocate memory for the new string using the garbage collector.
	result := c.currentBlock.NewCall(mallocFunc, totalLenPlusOne)

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

func (c *Compiler) compileStructLiteral(expr *ast.StructLiteral) (value.Value, error) {
	// This shouldn't be called directly as struct literals are handled in let statements
	return nil, fmt.Errorf("struct literals must be assigned to a variable")
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
