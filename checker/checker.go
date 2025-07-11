package checker

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
)

// Type represents a type in the Basalt type system
type Type interface {
	String() string
	Equals(other Type) bool
}

// Basic types
type IntegerType struct{}
type FloatType struct{}
type BooleanType struct{}
type StringType struct{}
type NoneType struct{}

func (t *IntegerType) String() string         { return "int64" }
func (t *IntegerType) Equals(other Type) bool { _, ok := other.(*IntegerType); return ok }

func (t *FloatType) String() string         { return "float64" }
func (t *FloatType) Equals(other Type) bool { _, ok := other.(*FloatType); return ok }

func (t *BooleanType) String() string         { return "bool" }
func (t *BooleanType) Equals(other Type) bool { _, ok := other.(*BooleanType); return ok }

func (t *StringType) String() string         { return "string" }
func (t *StringType) Equals(other Type) bool { _, ok := other.(*StringType); return ok }

func (t *NoneType) String() string         { return "none" }
func (t *NoneType) Equals(other Type) bool { _, ok := other.(*NoneType); return ok }

// Array type
type ArrayType struct {
	ElementType Type
}

func (t *ArrayType) String() string {
	return fmt.Sprintf("[%s]", t.ElementType.String())
}

func (t *ArrayType) Equals(other Type) bool {
	if otherArray, ok := other.(*ArrayType); ok {
		return t.ElementType.Equals(otherArray.ElementType)
	}
	return false
}

// Function type
type FunctionType struct {
	Parameters []Type
	ReturnType Type
}

func (t *FunctionType) String() string {
	params := make([]string, len(t.Parameters))
	for i, param := range t.Parameters {
		params[i] = param.String()
	}
	return fmt.Sprintf("fn(%s): %s", strings.Join(params, ", "), t.ReturnType.String())
}

func (t *FunctionType) Equals(other Type) bool {
	if otherFunc, ok := other.(*FunctionType); ok {
		if len(t.Parameters) != len(otherFunc.Parameters) {
			return false
		}
		for i, param := range t.Parameters {
			if !param.Equals(otherFunc.Parameters[i]) {
				return false
			}
		}
		return t.ReturnType.Equals(otherFunc.ReturnType)
	}
	return false
}

// Struct type
type StructType struct {
	Fields map[string]Type
}

func (t *StructType) String() string {
	fields := make([]string, 0, len(t.Fields))
	for name, fieldType := range t.Fields {
		fields = append(fields, fmt.Sprintf("%s: %s", name, fieldType.String()))
	}
	return fmt.Sprintf("struct { %s }", strings.Join(fields, ", "))
}

func (t *StructType) Equals(other Type) bool {
	if otherStruct, ok := other.(*StructType); ok {
		if len(t.Fields) != len(otherStruct.Fields) {
			return false
		}
		for name, fieldType := range t.Fields {
			otherFieldType, exists := otherStruct.Fields[name]
			if !exists || !fieldType.Equals(otherFieldType) {
				return false
			}
		}
		return true
	}
	return false
}

// Module type represents an imported module
type ModuleType struct {
	Name    string
	Members map[string]Type
}

func (t *ModuleType) String() string {
	return fmt.Sprintf("module %s", t.Name)
}

func (t *ModuleType) Equals(other Type) bool {
	if otherModule, ok := other.(*ModuleType); ok {
		return t.Name == otherModule.Name
	}
	return false
}

// TypeEnvironment represents a scope-aware symbol table for types
type TypeEnvironment struct {
	store map[string]Type
	outer *TypeEnvironment
}

func NewTypeEnvironment() *TypeEnvironment {
	return &TypeEnvironment{
		store: make(map[string]Type),
		outer: nil,
	}
}

func NewEnclosedTypeEnvironment(outer *TypeEnvironment) *TypeEnvironment {
	env := NewTypeEnvironment()
	env.outer = outer
	return env
}

func (e *TypeEnvironment) Get(name string) (Type, bool) {
	typ, ok := e.store[name]
	if !ok && e.outer != nil {
		return e.outer.Get(name)
	}
	return typ, ok
}

func (e *TypeEnvironment) Set(name string, typ Type) {
	e.store[name] = typ
}

// TypeError represents a type checking error
type TypeError struct {
	Message string
}

func (e *TypeError) Error() string {
	return e.Message
}

// Checker performs static type checking
type Checker struct {
	env    *TypeEnvironment
	errors []error
}

func New() *Checker {
	checker := &Checker{
		env:    NewTypeEnvironment(),
		errors: []error{},
	}

	// Add builtin functions
	checker.setupBuiltins()

	return checker
}

func (c *Checker) setupBuiltins() {
	// Add print function - variadic function that accepts any type and returns none
	printType := &FunctionType{
		Parameters: []Type{}, // Variadic, handled specially
		ReturnType: &NoneType{},
	}
	c.env.Set("print", printType)
}

func (c *Checker) Errors() []error {
	return c.errors
}

func (c *Checker) addError(message string) {
	c.errors = append(c.errors, &TypeError{Message: message})
}

// Check performs type checking on the given AST
func (c *Checker) Check(node ast.Node) Type {
	switch node := node.(type) {
	case *ast.Program:
		return c.checkProgram(node)
	case *ast.ImportStatement:
		return c.checkImportStatement(node)
	case *ast.LetStatement:
		return c.checkLetStatement(node)
	case *ast.ReturnStatement:
		return c.checkReturnStatement(node)
	case *ast.ExpressionStatement:
		return c.checkExpressionStatement(node)
	case *ast.BlockStatement:
		return c.checkBlockStatement(node)
	case *ast.Identifier:
		return c.checkIdentifier(node)
	case *ast.IntegerLiteral:
		return &IntegerType{}
	case *ast.FloatLiteral:
		return &FloatType{}
	case *ast.StringLiteral:
		return &StringType{}
	case *ast.Boolean:
		return &BooleanType{}
	case *ast.ArrayLiteral:
		return c.checkArrayLiteral(node)
	case *ast.FunctionLiteral:
		return c.checkFunctionLiteral(node)
	case *ast.CallExpression:
		return c.checkCallExpression(node)
	case *ast.MemberAccessExpression:
		return c.checkMemberAccessExpression(node)
	case *ast.IfExpression:
		return c.checkIfExpression(node)
	case *ast.ForExpression:
		return c.checkForExpression(node)
	case *ast.PrefixExpression:
		return c.checkPrefixExpression(node)
	case *ast.InfixExpression:
		return c.checkInfixExpression(node)
	case *ast.IndexExpression:
		return c.checkIndexExpression(node)
	default:
		c.addError(fmt.Sprintf("unknown node type: %T", node))
		return &NoneType{}
	}
}

func (c *Checker) checkProgram(program *ast.Program) Type {
	var result Type = &NoneType{}

	for _, stmt := range program.Statements {
		result = c.Check(stmt)
	}

	return result
}

func (c *Checker) checkImportStatement(node *ast.ImportStatement) Type {
	// For now, we'll create a simple module type for standard library modules
	modulePath := node.Path.String()

	// Determine the variable name for the import
	var variableName string
	if node.Alias != nil {
		variableName = node.Alias.Value
	} else {
		// Use the last part of the module path (e.g., "io" from "std::io")
		pathSegments := node.Path.Segments
		if len(pathSegments) > 0 {
			variableName = pathSegments[len(pathSegments)-1].Value
		} else {
			c.addError("invalid module path")
			return &NoneType{}
		}
	}

	// Create a module type with known members
	moduleType := &ModuleType{
		Name:    modulePath,
		Members: make(map[string]Type),
	}

	// Add known standard library functions
	switch modulePath {
	case "std::io":
		// io.print is a variadic function that accepts any number of arguments
		moduleType.Members["print"] = &FunctionType{
			Parameters: []Type{}, // Variadic, so we'll handle specially
			ReturnType: &NoneType{},
		}
		// io.write_file(filename: string, content: string) -> none
		moduleType.Members["write_file"] = &FunctionType{
			Parameters: []Type{&StringType{}, &StringType{}},
			ReturnType: &NoneType{},
		}
		// io.read_file(filename: string) -> string
		moduleType.Members["read_file"] = &FunctionType{
			Parameters: []Type{&StringType{}},
			ReturnType: &StringType{},
		}
	case "std::strings":
		// strings.contains(text: string, substring: string) -> bool
		moduleType.Members["contains"] = &FunctionType{
			Parameters: []Type{&StringType{}, &StringType{}},
			ReturnType: &BooleanType{},
		}
		// strings.split(text: string, separator: string) -> [string]
		moduleType.Members["split"] = &FunctionType{
			Parameters: []Type{&StringType{}, &StringType{}},
			ReturnType: &ArrayType{ElementType: &StringType{}},
		}
		// strings.join(parts: [string], separator: string) -> string
		moduleType.Members["join"] = &FunctionType{
			Parameters: []Type{&ArrayType{ElementType: &StringType{}}, &StringType{}},
			ReturnType: &StringType{},
		}
	}

	c.env.Set(variableName, moduleType)
	return &NoneType{}
}

func (c *Checker) checkMemberAccessExpression(node *ast.MemberAccessExpression) Type {
	leftType := c.Check(node.Left)

	if moduleType, ok := leftType.(*ModuleType); ok {
		memberName := node.Right.Value
		memberType, exists := moduleType.Members[memberName]
		if !exists {
			c.addError(fmt.Sprintf("undefined member '%s' on module %s", memberName, moduleType.Name))
			return &NoneType{}
		}
		return memberType
	}

	// Handle array member access
	if _, ok := leftType.(*ArrayType); ok {
		memberName := node.Right.Value
		if memberName == "len" {
			// arr.len is a function that returns the length of the array
			return &FunctionType{
				Parameters: []Type{}, // No parameters
				ReturnType: &IntegerType{},
			}
		}
		c.addError(fmt.Sprintf("undefined member '%s' on array", memberName))
		return &NoneType{}
	}

	c.addError(fmt.Sprintf("member access not supported on type %s", leftType.String()))
	return &NoneType{}
}

func (c *Checker) checkLetStatement(node *ast.LetStatement) Type {
	// Special handling for function definitions to support recursion
	if funcLit, ok := node.Value.(*ast.FunctionLiteral); ok {
		// First, determine the function type signature
		paramTypes := make([]Type, len(funcLit.Parameters))
		for i, param := range funcLit.Parameters {
			if param.Type == nil {
				c.addError(fmt.Sprintf("parameter %s must have a type annotation", param.Name.Value))
				paramTypes[i] = &NoneType{}
			} else {
				paramTypes[i] = c.parseTypeAnnotation(param.Type)
			}
		}

		var returnType Type
		if funcLit.ReturnType != nil {
			returnType = c.parseTypeAnnotation(funcLit.ReturnType)
		} else {
			returnType = &NoneType{}
		}

		// Create the function type
		funcType := &FunctionType{
			Parameters: paramTypes,
			ReturnType: returnType,
		}

		// Register the function name in the current scope BEFORE checking the body
		// This allows recursive calls to work
		c.env.Set(node.Name.Value, funcType)

		// Now check the function body with the function name available
		bodyType := c.checkFunctionLiteralBody(funcLit, funcType)

		// If return type was specified, check compatibility
		if funcLit.ReturnType != nil {
			if !c.isAssignable(bodyType, returnType) {
				c.addError(fmt.Sprintf("function body returns %s, expected %s", bodyType.String(), returnType.String()))
			}
		} else {
			// Update return type based on body
			funcType.ReturnType = bodyType
			c.env.Set(node.Name.Value, funcType) // Update with correct return type
		}

		// If variable type annotation is provided, check compatibility
		if node.Type != nil {
			expectedType := c.parseTypeAnnotation(node.Type)
			if !c.isAssignable(funcType, expectedType) {
				c.addError(fmt.Sprintf("type mismatch: cannot assign %s to %s", funcType.String(), expectedType.String()))
			}
		}

		return &NoneType{}
	}

	// Regular variable assignment
	valueType := c.Check(node.Value)

	// If type annotation is provided, check compatibility
	if node.Type != nil {
		expectedType := c.parseTypeAnnotation(node.Type)
		if !c.isAssignable(valueType, expectedType) {
			c.addError(fmt.Sprintf("type mismatch: cannot assign %s to %s", valueType.String(), expectedType.String()))
		}
		c.env.Set(node.Name.Value, expectedType)
	} else {
		// Type inference: use the type of the value
		c.env.Set(node.Name.Value, valueType)
	}

	return &NoneType{}
}

func (c *Checker) checkFunctionLiteralBody(node *ast.FunctionLiteral, funcType *FunctionType) Type {
	// Create new scope for function parameters
	prevEnv := c.env
	c.env = NewEnclosedTypeEnvironment(prevEnv)

	// Add parameters to scope
	for _, param := range node.Parameters {
		if param.Type != nil {
			paramType := c.parseTypeAnnotation(param.Type)
			c.env.Set(param.Name.Value, paramType)
		}
	}

	// Check function body
	bodyType := c.Check(node.Body)

	// Restore previous environment
	c.env = prevEnv

	return bodyType
}

func (c *Checker) checkReturnStatement(node *ast.ReturnStatement) Type {
	if node.ReturnValue != nil {
		return c.Check(node.ReturnValue)
	}
	return &NoneType{}
}

func (c *Checker) checkExpressionStatement(node *ast.ExpressionStatement) Type {
	return c.Check(node.Expression)
}

func (c *Checker) checkBlockStatement(node *ast.BlockStatement) Type {
	var result Type = &NoneType{}

	for _, stmt := range node.Statements {
		result = c.Check(stmt)
	}

	return result
}

func (c *Checker) checkIdentifier(node *ast.Identifier) Type {
	typ, ok := c.env.Get(node.Value)
	if !ok {
		c.addError(fmt.Sprintf("identifier not found: %s", node.Value))
		return &NoneType{}
	}
	return typ
}

func (c *Checker) checkArrayLiteral(node *ast.ArrayLiteral) Type {
	if len(node.Elements) == 0 {
		// For empty arrays, we'll assume they're integer arrays for now
		// In a more sophisticated implementation, we'd need type inference or annotations
		return &ArrayType{ElementType: &IntegerType{}}
	}

	elementType := c.Check(node.Elements[0])

	for i, elem := range node.Elements[1:] {
		elemType := c.Check(elem)
		if !elementType.Equals(elemType) {
			c.addError(fmt.Sprintf("array element %d has type %s, expected %s", i+1, elemType.String(), elementType.String()))
		}
	}

	return &ArrayType{ElementType: elementType}
}

func (c *Checker) checkFunctionLiteral(node *ast.FunctionLiteral) Type {
	// Create new scope for function
	prevEnv := c.env
	c.env = NewEnclosedTypeEnvironment(prevEnv)

	// Add parameters to scope
	paramTypes := make([]Type, len(node.Parameters))
	for i, param := range node.Parameters {
		if param.Type == nil {
			c.addError(fmt.Sprintf("parameter %s must have a type annotation", param.Name.Value))
			paramTypes[i] = &NoneType{}
			c.env.Set(param.Name.Value, &NoneType{})
		} else {
			paramType := c.parseTypeAnnotation(param.Type)
			paramTypes[i] = paramType
			c.env.Set(param.Name.Value, paramType)
		}
	}

	// Determine return type first
	var returnType Type
	if node.ReturnType != nil {
		returnType = c.parseTypeAnnotation(node.ReturnType)
	} else {
		returnType = &NoneType{} // Default return type
	}

	// Create the function type early so it can be used for recursive calls
	funcType := &FunctionType{
		Parameters: paramTypes,
		ReturnType: returnType,
	}

	// Check function body
	bodyType := c.Check(node.Body)

	// If return type was specified, check compatibility
	if node.ReturnType != nil {
		if !c.isAssignable(bodyType, returnType) {
			c.addError(fmt.Sprintf("function body returns %s, expected %s", bodyType.String(), returnType.String()))
		}
	} else {
		// Update return type based on body
		funcType.ReturnType = bodyType
	}

	// Restore previous environment
	c.env = prevEnv

	return funcType
}

func (c *Checker) checkCallExpression(node *ast.CallExpression) Type {
	funcType := c.Check(node.Function)

	fnType, ok := funcType.(*FunctionType)
	if !ok {
		c.addError(fmt.Sprintf("cannot call non-function type: %s", funcType.String()))
		return &NoneType{}
	}

	// Special handling for print function (variadic)
	if ident, ok := node.Function.(*ast.Identifier); ok && ident.Value == "print" {
		// Print accepts any single argument and returns none
		if len(node.Arguments) != 1 {
			c.addError(fmt.Sprintf("print expects exactly 1 argument, got %d", len(node.Arguments)))
		} else {
			// Type check the argument but allow any type
			c.Check(node.Arguments[0])
		}
		return &NoneType{}
	}

	// Regular function call handling
	if len(node.Arguments) != len(fnType.Parameters) {
		c.addError(fmt.Sprintf("wrong number of arguments: expected %d, got %d", len(fnType.Parameters), len(node.Arguments)))
		return fnType.ReturnType
	}

	for i, arg := range node.Arguments {
		argType := c.Check(arg)
		expectedType := fnType.Parameters[i]
		if !c.isAssignable(argType, expectedType) {
			c.addError(fmt.Sprintf("argument %d has type %s, expected %s", i, argType.String(), expectedType.String()))
		}
	}

	return fnType.ReturnType
}

func (c *Checker) checkIfExpression(node *ast.IfExpression) Type {
	condType := c.Check(node.Condition)
	if !condType.Equals(&BooleanType{}) {
		c.addError(fmt.Sprintf("if condition must be boolean, got %s", condType.String()))
	}

	consequenceType := c.Check(node.Consequence)

	if node.Alternative != nil {
		alternativeType := c.Check(node.Alternative)
		if !consequenceType.Equals(alternativeType) {
			c.addError(fmt.Sprintf("if branches have different types: %s vs %s", consequenceType.String(), alternativeType.String()))
		}
	}

	return consequenceType
}

func (c *Checker) checkForExpression(node *ast.ForExpression) Type {
	condType := c.Check(node.Condition)
	if !condType.Equals(&BooleanType{}) {
		c.addError(fmt.Sprintf("for condition must be boolean, got %s", condType.String()))
	}

	c.Check(node.Consequence)
	return &NoneType{}
}

func (c *Checker) checkPrefixExpression(node *ast.PrefixExpression) Type {
	rightType := c.Check(node.Right)

	switch node.Operator {
	case "!":
		if !rightType.Equals(&BooleanType{}) {
			c.addError(fmt.Sprintf("cannot apply ! to %s", rightType.String()))
		}
		return &BooleanType{}
	case "-":
		if !rightType.Equals(&IntegerType{}) && !rightType.Equals(&FloatType{}) {
			c.addError(fmt.Sprintf("cannot apply - to %s", rightType.String()))
		}
		return rightType
	default:
		c.addError(fmt.Sprintf("unknown prefix operator: %s", node.Operator))
		return &NoneType{}
	}
}

func (c *Checker) checkInfixExpression(node *ast.InfixExpression) Type {
	leftType := c.Check(node.Left)
	rightType := c.Check(node.Right)

	switch node.Operator {
	case "+", "-", "*", "/":
		return c.checkArithmeticOperation(node.Operator, leftType, rightType)
	case "==", "!=":
		if !leftType.Equals(rightType) {
			c.addError(fmt.Sprintf("cannot compare %s with %s", leftType.String(), rightType.String()))
		}
		return &BooleanType{}
	case "<", ">":
		if !c.isComparable(leftType, rightType) {
			c.addError(fmt.Sprintf("cannot compare %s with %s", leftType.String(), rightType.String()))
		}
		return &BooleanType{}
	case "=":
		// Assignment
		if !c.isAssignable(rightType, leftType) {
			c.addError(fmt.Sprintf("cannot assign %s to %s", rightType.String(), leftType.String()))
		}
		return rightType
	default:
		c.addError(fmt.Sprintf("unknown infix operator: %s", node.Operator))
		return &NoneType{}
	}
}

func (c *Checker) checkIndexExpression(node *ast.IndexExpression) Type {
	leftType := c.Check(node.Left)

	if arrayType, ok := leftType.(*ArrayType); ok {
		indexType := c.Check(node.Start)
		if !indexType.Equals(&IntegerType{}) {
			c.addError(fmt.Sprintf("array index must be integer, got %s", indexType.String()))
		}
		return arrayType.ElementType
	}

	c.addError(fmt.Sprintf("cannot index into %s", leftType.String()))
	return &NoneType{}
}

func (c *Checker) checkArithmeticOperation(operator string, leftType, rightType Type) Type {
	// String concatenation
	if operator == "+" && leftType.Equals(&StringType{}) && rightType.Equals(&StringType{}) {
		return &StringType{}
	}

	// Numeric operations
	if leftType.Equals(&IntegerType{}) && rightType.Equals(&IntegerType{}) {
		return &IntegerType{}
	}

	if leftType.Equals(&FloatType{}) && rightType.Equals(&FloatType{}) {
		return &FloatType{}
	}

	if (leftType.Equals(&IntegerType{}) && rightType.Equals(&FloatType{})) ||
		(leftType.Equals(&FloatType{}) && rightType.Equals(&IntegerType{})) {
		return &FloatType{}
	}

	c.addError(fmt.Sprintf("cannot apply %s to %s and %s", operator, leftType.String(), rightType.String()))
	return &NoneType{}
}

func (c *Checker) isAssignable(from, to Type) bool {
	return from.Equals(to)
}

func (c *Checker) isComparable(left, right Type) bool {
	return (left.Equals(&IntegerType{}) || left.Equals(&FloatType{})) &&
		(right.Equals(&IntegerType{}) || right.Equals(&FloatType{}))
}

func (c *Checker) parseTypeAnnotation(typeAnnotation *ast.TypeAnnotation) Type {
	switch typeAnnotation.Value {
	case "int64":
		return &IntegerType{}
	case "float64":
		return &FloatType{}
	case "bool":
		return &BooleanType{}
	case "string":
		return &StringType{}
	case "none":
		return &NoneType{}
	default:
		// Check if it's a function type annotation like "fn(int64): int64"
		if strings.HasPrefix(typeAnnotation.Value, "fn(") {
			return c.parseFunctionTypeAnnotation(typeAnnotation.Value)
		}
		c.addError(fmt.Sprintf("unknown type: %s", typeAnnotation.Value))
		return &NoneType{}
	}
}

func (c *Checker) parseFunctionTypeAnnotation(typeStr string) Type {
	// For now, we'll do a simple parsing of function type annotations
	// This is a simplified parser for function types like "fn(int64): int64"

	// Remove "fn(" prefix
	if !strings.HasPrefix(typeStr, "fn(") {
		c.addError(fmt.Sprintf("invalid function type: %s", typeStr))
		return &NoneType{}
	}

	// Find the matching closing parenthesis
	parenCount := 0
	paramEnd := -1
	for i, char := range typeStr {
		if char == '(' {
			parenCount++
		} else if char == ')' {
			parenCount--
			if parenCount == 0 {
				paramEnd = i
				break
			}
		}
	}

	if paramEnd == -1 {
		c.addError(fmt.Sprintf("invalid function type: %s", typeStr))
		return &NoneType{}
	}

	// Extract parameter types
	paramStr := typeStr[3:paramEnd] // Remove "fn("
	var paramTypes []Type

	if paramStr != "" {
		paramParts := strings.Split(paramStr, ",")
		for _, part := range paramParts {
			part = strings.TrimSpace(part)
			paramTypes = append(paramTypes, c.parseSimpleType(part))
		}
	}

	// Extract return type
	returnTypeStr := ""
	if len(typeStr) > paramEnd+1 && typeStr[paramEnd+1] == ':' {
		returnTypeStr = strings.TrimSpace(typeStr[paramEnd+2:])
	}

	var returnType Type = &NoneType{}
	if returnTypeStr != "" {
		returnType = c.parseSimpleType(returnTypeStr)
	}

	return &FunctionType{
		Parameters: paramTypes,
		ReturnType: returnType,
	}
}

func (c *Checker) parseSimpleType(typeStr string) Type {
	switch strings.TrimSpace(typeStr) {
	case "int64":
		return &IntegerType{}
	case "float64":
		return &FloatType{}
	case "bool":
		return &BooleanType{}
	case "string":
		return &StringType{}
	case "none":
		return &NoneType{}
	default:
		c.addError(fmt.Sprintf("unknown type: %s", typeStr))
		return &NoneType{}
	}
}
