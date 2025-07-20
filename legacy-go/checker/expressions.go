package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
)

// Expression checking methods

func (c *Checker) checkExpressionStatement(node *ast.ExpressionStatement) Type {
	exprType := c.Check(node.Expression)
	if node.HasSemicolon {
		return &NoneType{}
	}
	return exprType
}

func (c *Checker) checkIdentifier(node *ast.Identifier) Type {
	if typ, ok := c.env.Get(node.Value); ok {
		return typ
	}
	c.addErrorWithLocation(fmt.Sprintf("identifier not found: %s", node.Value), node)
	return &NoneType{}
}

func (c *Checker) checkIntegerLiteral(node *ast.IntegerLiteral) Type {
	return &IntegerType{}
}

func (c *Checker) checkFloatLiteral(node *ast.FloatLiteral) Type {
	return &FloatType{}
}

func (c *Checker) checkStringLiteral(node *ast.StringLiteral) Type {
	return &StringType{}
}

func (c *Checker) checkBoolean(node *ast.Boolean) Type {
	return &BooleanType{}
}

func (c *Checker) checkArrayLiteral(node *ast.ArrayLiteral) Type {
	if len(node.Elements) == 0 {
		return &ArrayType{ElementType: &NoneType{}} // An array of unknown type
	}
	elemType := c.Check(node.Elements[0])
	for i, elem := range node.Elements[1:] {
		t := c.Check(elem)
		if !elemType.Equals(t) {
			c.addError(fmt.Sprintf("array element %d has type %s, expected %s", i+2, t.String(), elemType.String()), node.Token)
		}
	}
	return &ArrayType{ElementType: elemType}
}

func (c *Checker) checkHashLiteral(node *ast.HashLiteral) Type {
	if len(node.Pairs) == 0 {
		// Empty hash literal {} - type can only be determined by explicit type annotation
		// Return a generic hash map type that will be resolved later
		return &HashMapType{KeyType: &NoneType{}, ValueType: &NoneType{}}
	}

	// Infer types from the first key-value pair
	firstPair := node.Pairs[0]
	keyType := c.Check(firstPair.Key)
	valueType := c.Check(firstPair.Value)

	// Validate all subsequent pairs match the inferred types
	for i, pair := range node.Pairs[1:] {
		pairKeyType := c.Check(pair.Key)
		pairValueType := c.Check(pair.Value)

		if !c.isAssignable(pairKeyType, keyType) {
			c.addError(fmt.Sprintf("hash key %d has type %s, expected %s", i+2, pairKeyType.String(), keyType.String()), node.Token)
		}

		if !c.isAssignable(pairValueType, valueType) {
			c.addError(fmt.Sprintf("hash value %d has type %s, expected %s", i+2, pairValueType.String(), valueType.String()), node.Token)
		}
	}

	return &HashMapType{KeyType: keyType, ValueType: valueType}
}

func (c *Checker) checkFunctionLiteral(node *ast.FunctionLiteral) Type {
	paramTypes := make([]Type, len(node.Parameters))
	for i, param := range node.Parameters {
		paramTypes[i] = c.parseTypeAnnotation(param.Type)
	}
	var returnType Type = &NoneType{}
	if node.ReturnType != nil {
		returnType = c.parseTypeAnnotation(node.ReturnType)
	}
	return &FunctionType{
		Parameters: paramTypes,
		ReturnType: returnType,
		IsVariadic: node.IsVariadic,
	}
}

func (c *Checker) checkCallExpression(node *ast.CallExpression) Type {
	funcTypeVal := c.Check(node.Function)
	fnType, ok := funcTypeVal.(*FunctionType)
	if !ok {
		c.addErrorWithLocation(fmt.Sprintf("cannot call non-function type: %s", funcTypeVal.String()), node)
		return &NoneType{}
	}

	if fnType.IsVariadic {
		if len(node.Arguments) < len(fnType.Parameters) {
			c.addErrorWithLocation(fmt.Sprintf("wrong number of arguments for variadic function: expected at least %d, got %d", len(fnType.Parameters), len(node.Arguments)), node)
		}
	} else {
		if len(node.Arguments) != len(fnType.Parameters) {
			c.addErrorWithLocation(fmt.Sprintf("wrong number of arguments: expected %d, got %d", len(fnType.Parameters), len(node.Arguments)), node)
		}
	}

	numFixedArgs := len(fnType.Parameters)
	if len(node.Arguments) < numFixedArgs {
		numFixedArgs = len(node.Arguments)
	}
	for i := 0; i < numFixedArgs; i++ {
		argType := c.Check(node.Arguments[i])
		paramType := fnType.Parameters[i]
		if !c.isAssignable(argType, paramType) {
			c.addErrorWithLocation(fmt.Sprintf("argument %d has type %s, expected %s", i+1, argType.String(), paramType.String()), node)
		}
	}

	for i := len(fnType.Parameters); i < len(node.Arguments); i++ {
		c.Check(node.Arguments[i])
	}

	return fnType.ReturnType
}

func (c *Checker) checkPrefixExpression(node *ast.PrefixExpression) Type {
	rightType := c.Check(node.Right)
	switch node.Operator {
	case "!":
		if !rightType.Equals(&BooleanType{}) {
			c.addError(fmt.Sprintf("cannot apply ! to %s", rightType.String()), node.Token)
		}
		return &BooleanType{}
	case "-":
		if !rightType.Equals(&IntegerType{}) && !rightType.Equals(&FloatType{}) {
			c.addError(fmt.Sprintf("cannot apply - to %s", rightType.String()), node.Token)
		}
		return rightType
	default:
		c.addError(fmt.Sprintf("unknown prefix operator: %s", node.Operator), node.Token)
		return &NoneType{}
	}
}

func (c *Checker) checkInfixExpression(node *ast.InfixExpression) Type {
	leftType := c.Check(node.Left)
	rightType := c.Check(node.Right)
	op := node.Operator

	isLeftNum := leftType.Equals(&IntegerType{}) || leftType.Equals(&FloatType{})
	isRightNum := rightType.Equals(&IntegerType{}) || rightType.Equals(&FloatType{})
	if isLeftNum && isRightNum {
		switch op {
		case "+", "-", "*", "/":
			if leftType.Equals(&FloatType{}) || rightType.Equals(&FloatType{}) {
				return &FloatType{}
			}
			return &IntegerType{}
		case "<", ">", "==", "!=":
			return &BooleanType{}
		}
	}

	isLeftString := leftType.Equals(&StringType{})
	isRightString := rightType.Equals(&StringType{})
	if isLeftString && isRightString {
		switch op {
		case "+":
			return &StringType{}
		case "==", "!=":
			return &BooleanType{}
		}
	}

	isLeftBool := leftType.Equals(&BooleanType{})
	isRightBool := rightType.Equals(&BooleanType{})
	if isLeftBool && isRightBool && (op == "==" || op == "!=") {
		return &BooleanType{}
	}

	// Handle pointer and rawptr operations in unsafe context
	if c.isInUnsafeContext {
		isLeftRaw := leftType.Equals(&RawPointerType{})
		isRightRaw := rightType.Equals(&RawPointerType{})
		_, isLeftPtr := leftType.(*PointerType)
		_, isRightPtr := rightType.(*PointerType)

		// Pointer/rawptr arithmetic: ptr + int or int + ptr
		if op == "+" || op == "-" {
			if (isLeftRaw || isLeftPtr) && isRightNum {
				return leftType // Return the pointer type
			}
			if isLeftNum && (isRightRaw || isRightPtr) && op == "+" {
				return rightType // Return the pointer type
			}
		}

		// Pointer/rawptr comparison with null (0)
		if op == "==" || op == "!=" {
			if (isLeftRaw || isLeftPtr) && rightType.Equals(&IntegerType{}) {
				return &BooleanType{}
			}
			if leftType.Equals(&IntegerType{}) && (isRightRaw || isRightPtr) {
				return &BooleanType{}
			}
		}
	}

	if op == "=" {
		// Special handling for hash map assignments: map["key"] = value
		if indexExpr, ok := node.Left.(*ast.IndexExpression); ok {
			mapType := c.Check(indexExpr.Left)
			if hashMapType, ok := mapType.(*HashMapType); ok {
				keyType := c.Check(indexExpr.Start)
				if !c.isAssignable(keyType, hashMapType.KeyType) {
					c.addError(fmt.Sprintf("hash map key must be %s, got %s", hashMapType.KeyType.String(), keyType.String()), node.Token)
				}
				if !c.isAssignable(rightType, hashMapType.ValueType) {
					c.addError(fmt.Sprintf("cannot assign %s to hash map value of type %s", rightType.String(), hashMapType.ValueType.String()), node.Token)
				}
				return rightType
			}
		}

		if !c.isAssignable(rightType, leftType) {
			c.addError(fmt.Sprintf("cannot assign %s to %s", rightType.String(), leftType.String()), node.Token)
		}
		return rightType
	}

	c.addError(fmt.Sprintf("unknown infix operator: %s for types %s and %s", op, leftType.String(), rightType.String()), node.Token)
	return &NoneType{}
}

func (c *Checker) checkIndexExpression(node *ast.IndexExpression) Type {
	leftType := c.Check(node.Left)

	if arrayType, ok := leftType.(*ArrayType); ok {
		indexType := c.Check(node.Start)
		if !indexType.Equals(&IntegerType{}) {
			c.addError(fmt.Sprintf("array index must be integer, got %s", indexType.String()), node.Token)
		}
		return arrayType.ElementType
	}

	if hashMapType, ok := leftType.(*HashMapType); ok {
		indexType := c.Check(node.Start)
		if !c.isAssignable(indexType, hashMapType.KeyType) {
			c.addError(fmt.Sprintf("hash map key must be %s, got %s", hashMapType.KeyType.String(), indexType.String()), node.Token)
		}
		return hashMapType.ValueType
	}

	c.addError(fmt.Sprintf("cannot index into %s", leftType.String()), node.Token)
	return &NoneType{}
}

func (c *Checker) checkMemberAccessExpression(node *ast.MemberAccessExpression) Type {
	leftType := c.Check(node.Left)

	if structType, ok := leftType.(*StructType); ok {
		memberName := node.Right.Value
		fieldType, exists := structType.Fields[memberName]
		if !exists {
			c.addError(fmt.Sprintf("field '%s' not found in struct %s", memberName, structType.Name), node.Token)
			return &NoneType{}
		}
		return fieldType
	}

	if ptrType, ok := leftType.(*PointerType); ok {
		if structType, ok := ptrType.InnerType.(*StructType); ok {
			memberName := node.Right.Value
			fieldType, exists := structType.Fields[memberName]
			if !exists {
				c.addError(fmt.Sprintf("field '%s' not found in struct %s", memberName, structType.Name), node.Token)
				return &NoneType{}
			}
			return fieldType
		} else {
			c.addError(fmt.Sprintf("cannot access field '%s' on pointer to non-struct type %s", node.Right.Value, ptrType.InnerType.String()), node.Token)
			return &NoneType{}
		}
	}

	if moduleType, ok := leftType.(*ModuleType); ok {
		memberName := node.Right.Value
		memberType, exists := moduleType.Members.Get(memberName)
		if !exists {
			c.addError(fmt.Sprintf("member '%s' not found in module %s", memberName, moduleType.Name), node.Token)
			return &NoneType{}
		}
		return memberType
	}

	c.addError(fmt.Sprintf("member access not supported on type %s", leftType.String()), node.Token)
	return &NoneType{}
}
