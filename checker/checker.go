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
type (
	IntegerType  struct{}
	FloatType    struct{}
	BooleanType  struct{}
	StringType   struct{}
	NoneType     struct{}
	ArrayPtrType struct{} // Represents a generic pointer, used for interop
)

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

func (t *ArrayPtrType) String() string         { return "array_ptr" }
func (t *ArrayPtrType) Equals(other Type) bool { _, ok := other.(*ArrayPtrType); return ok }

// Array type
type ArrayType struct {
	ElementType Type
}

func (t *ArrayType) String() string {
	return fmt.Sprintf("[%s]", t.ElementType.String())
}

func (t *ArrayType) Equals(other Type) bool {
	if otherArray, ok := other.(*ArrayType); ok {
		if t.ElementType.Equals(&NoneType{}) || otherArray.ElementType.Equals(&NoneType{}) {
			return true // Allow comparison with untyped empty array
		}
		return t.ElementType.Equals(otherArray.ElementType)
	}
	return false
}

// Function type
type FunctionType struct {
	Parameters []Type
	ReturnType Type
	IsVariadic bool
}

func (t *FunctionType) String() string {
	params := make([]string, len(t.Parameters))
	for i, param := range t.Parameters {
		params[i] = param.String()
	}
	paramStr := strings.Join(params, ", ")
	if t.IsVariadic {
		if len(params) > 0 {
			paramStr += ", "
		}
		paramStr += "..."
	}
	return fmt.Sprintf("fn(%s) -> %s", paramStr, t.ReturnType.String())
}

func (t *FunctionType) Equals(other Type) bool {
	if otherFunc, ok := other.(*FunctionType); ok {
		if len(t.Parameters) != len(otherFunc.Parameters) || t.IsVariadic != otherFunc.IsVariadic {
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
	Name   string
	Fields map[string]Type
}

func (t *StructType) String() string {
	return t.Name
}

func (t *StructType) Equals(other Type) bool {
	if otherStruct, ok := other.(*StructType); ok {
		return t.Name == otherStruct.Name
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
	checker.setupBuiltins()
	return checker
}

func (c *Checker) setupBuiltins() {
	// No built-ins needed anymore, as they are provided by the stdlib
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
		return &NoneType{} // Imports are handled by file loading for now
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
	case *ast.StructLiteral:
		return c.checkStructLiteral(node)
	case *ast.StructInstanceExpression:
		return c.checkStructInstanceExpression(node)
	case *ast.ExternStatement:
		return c.checkExternStatement(node)
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

func (c *Checker) checkLetStatement(node *ast.LetStatement) Type {
	// Special handling for struct definitions
	if structLit, ok := node.Value.(*ast.StructLiteral); ok {
		return c.checkStructDefinition(node.Name.Value, structLit)
	}

	// Special handling for function definitions to support recursion
	if funcLit, ok := node.Value.(*ast.FunctionLiteral); ok {
		paramTypes := make([]Type, len(funcLit.Parameters))
		for i, param := range funcLit.Parameters {
			paramTypes[i] = c.parseTypeAnnotation(param.Type)
		}
		var returnType Type = &NoneType{}
		if funcLit.ReturnType != nil {
			returnType = c.parseTypeAnnotation(funcLit.ReturnType)
		}
		funcType := &FunctionType{
			Parameters: paramTypes,
			ReturnType: returnType,
			IsVariadic: funcLit.IsVariadic,
		}
		c.env.Set(node.Name.Value, funcType)

		// Check function body in a new scope
		funcEnv := NewEnclosedTypeEnvironment(c.env)
		for i, param := range funcLit.Parameters {
			funcEnv.Set(param.Name.Value, paramTypes[i])
		}
		savedEnv := c.env
		c.env = funcEnv
		bodyType := c.Check(funcLit.Body)
		c.env = savedEnv

		// If return type is explicit, check against inferred body type
		if funcLit.ReturnType != nil {
			// A block's type is its last expression. If it ends in a semicolon, it's NoneType.
			// Allow functions to implicitly return.
			if !c.isAssignable(bodyType, returnType) && bodyType.String() != "none" {
				c.addError(fmt.Sprintf("function body returns %s but expected %s", bodyType.String(), returnType.String()))
			}
		} else { // Infer return type
			funcType.ReturnType = bodyType
		}

		return &NoneType{}
	}

	// Regular variable assignment
	valueType := c.Check(node.Value)
	if node.Type != nil {
		expectedType := c.parseTypeAnnotation(node.Type)
		if !c.isAssignable(valueType, expectedType) {
			c.addError(fmt.Sprintf("type mismatch: cannot assign %s to variable of type %s", valueType.String(), expectedType.String()))
		}
		c.env.Set(node.Name.Value, expectedType)
	} else {
		c.env.Set(node.Name.Value, valueType)
	}
	return &NoneType{}
}

func (c *Checker) checkReturnStatement(node *ast.ReturnStatement) Type {
	if node.ReturnValue != nil {
		return c.Check(node.ReturnValue)
	}
	return &NoneType{}
}

func (c *Checker) checkExpressionStatement(node *ast.ExpressionStatement) Type {
	exprType := c.Check(node.Expression)
	if node.HasSemicolon {
		return &NoneType{}
	}
	return exprType
}

func (c *Checker) checkBlockStatement(node *ast.BlockStatement) Type {
	blockEnv := NewEnclosedTypeEnvironment(c.env)
	savedEnv := c.env
	c.env = blockEnv
	defer func() { c.env = savedEnv }()

	var result Type = &NoneType{}
	for _, stmt := range node.Statements {
		result = c.Check(stmt)
	}
	return result
}

func (c *Checker) checkIdentifier(node *ast.Identifier) Type {
	if typ, ok := c.env.Get(node.Value); ok {
		return typ
	}
	c.addError(fmt.Sprintf("identifier not found: %s", node.Value))
	return &NoneType{}
}

func (c *Checker) checkArrayLiteral(node *ast.ArrayLiteral) Type {
	if len(node.Elements) == 0 {
		return &ArrayType{ElementType: &NoneType{}} // An array of unknown type
	}
	elemType := c.Check(node.Elements[0])
	for i, elem := range node.Elements[1:] {
		t := c.Check(elem)
		if !elemType.Equals(t) {
			c.addError(fmt.Sprintf("array element %d has type %s, expected %s", i+2, t.String(), elemType.String()))
		}
	}
	return &ArrayType{ElementType: elemType}
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
		c.addError(fmt.Sprintf("cannot call non-function type: %s", funcTypeVal.String()))
		return &NoneType{}
	}

	if fnType.IsVariadic {
		if len(node.Arguments) < len(fnType.Parameters) {
			c.addError(fmt.Sprintf("wrong number of arguments for variadic function: expected at least %d, got %d", len(fnType.Parameters), len(node.Arguments)))
		}
	} else {
		if len(node.Arguments) != len(fnType.Parameters) {
			c.addError(fmt.Sprintf("wrong number of arguments: expected %d, got %d", len(fnType.Parameters), len(node.Arguments)))
		}
	}

	// Type-check the non-variadic arguments
	numFixedArgs := len(fnType.Parameters)
	if len(node.Arguments) < numFixedArgs {
		numFixedArgs = len(node.Arguments)
	}
	for i := 0; i < numFixedArgs; i++ {
		argType := c.Check(node.Arguments[i])
		paramType := fnType.Parameters[i]
		if !c.isAssignable(argType, paramType) {
			c.addError(fmt.Sprintf("argument %d has type %s, expected %s", i+1, argType.String(), paramType.String()))
		}
	}

	// Type-check variadic arguments (for now, we just check them without a specific target type)
	for i := len(fnType.Parameters); i < len(node.Arguments); i++ {
		c.Check(node.Arguments[i])
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
			// This might be okay if one branch returns a value and the other doesn't,
			// making the whole expression have type NoneType.
			// For now, let's be strict.
			c.addError(fmt.Sprintf("if branches have different types: %s vs %s", consequenceType.String(), alternativeType.String()))
		}
		return consequenceType
	}
	return &NoneType{}
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
	op := node.Operator

	// Handle numeric types
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

	// Handle string types
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

	// Handle boolean types
	isLeftBool := leftType.Equals(&BooleanType{})
	isRightBool := rightType.Equals(&BooleanType{})
	if isLeftBool && isRightBool && (op == "==" || op == "!=") {
		return &BooleanType{}
	}

	// Handle assignment
	if op == "=" {
		if !c.isAssignable(rightType, leftType) {
			c.addError(fmt.Sprintf("cannot assign %s to %s", rightType.String(), leftType.String()))
		}
		return rightType
	}

	c.addError(fmt.Sprintf("unknown infix operator: %s for types %s and %s", op, leftType.String(), rightType.String()))
	return &NoneType{}
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

func (c *Checker) isAssignable(from, to Type) bool {
	if from.Equals(to) {
		return true
	}
	// Allow assigning int to float
	if from.Equals(&IntegerType{}) && to.Equals(&FloatType{}) {
		return true
	}
	// Allow assigning untyped array literal to any array type
	if from.Equals(&ArrayType{ElementType: &NoneType{}}) {
		if _, ok := to.(*ArrayType); ok {
			return true
		}
	}
	return false
}

func (c *Checker) checkStructDefinition(structName string, node *ast.StructLiteral) Type {
	fields := make(map[string]Type)
	for _, field := range node.Fields {
		fieldName := field.Name.Value
		fieldType := c.parseTypeAnnotation(field.Type)
		fields[fieldName] = fieldType
	}
	structType := &StructType{Name: structName, Fields: fields}
	c.env.Set(structName, structType)
	return &NoneType{}
}

func (c *Checker) checkStructLiteral(node *ast.StructLiteral) Type {
	c.addError("struct literals must be assigned to a variable")
	return &NoneType{}
}

func (c *Checker) checkStructInstanceExpression(node *ast.StructInstanceExpression) Type {
	structTypeVal := c.Check(node.StructExpr)
	structDef, ok := structTypeVal.(*StructType)
	if !ok {
		c.addError(fmt.Sprintf("cannot instantiate non-struct type: %s", structTypeVal.String()))
		return &NoneType{}
	}
	// Check field existence and types
	for name, expr := range node.Fields {
		expectedType, ok := structDef.Fields[name]
		if !ok {
			c.addError(fmt.Sprintf("field '%s' not found in struct %s", name, structDef.Name))
			continue
		}
		actualType := c.Check(expr)
		if !c.isAssignable(actualType, expectedType) {
			c.addError(fmt.Sprintf("field '%s' expects type %s, got %s", name, expectedType.String(), actualType.String()))
		}
	}
	// Check for missing fields
	for name := range structDef.Fields {
		if _, ok := node.Fields[name]; !ok {
			c.addError(fmt.Sprintf("missing field '%s' in instantiation of struct %s", name, structDef.Name))
		}
	}
	return structDef
}

func (c *Checker) checkExternStatement(node *ast.ExternStatement) Type {
	paramTypes := make([]Type, len(node.Parameters))
	for i, p := range node.Parameters {
		paramTypes[i] = c.parseTypeAnnotation(p.Type)
	}
	var returnType Type = &NoneType{}
	if node.ReturnType != nil {
		returnType = c.parseTypeAnnotation(node.ReturnType)
	}
	funcType := &FunctionType{
		Parameters: paramTypes,
		ReturnType: returnType,
		IsVariadic: node.IsVariadic,
	}
	c.env.Set(node.Function.Value, funcType)
	return &NoneType{}
}

func (c *Checker) parseTypeAnnotation(typeAnnotation ast.Node) Type {
	var typeName string
	switch ta := typeAnnotation.(type) {
	case *ast.TypeAnnotation:
		typeName = ta.Value
	case *ast.Identifier:
		typeName = ta.Value
	default:
		c.addError(fmt.Sprintf("invalid type annotation node: %T", typeAnnotation))
		return &NoneType{}
	}

	switch typeName {
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
	case "array_ptr":
		return &ArrayPtrType{}
	default:
		if typ, ok := c.env.Get(typeName); ok {
			return typ
		}
		c.addError(fmt.Sprintf("unknown type: %s", typeName))
		return &NoneType{}
	}
}

func (c *Checker) checkMemberAccessExpression(node *ast.MemberAccessExpression) Type {
	leftType := c.Check(node.Left)

	if structType, ok := leftType.(*StructType); ok {
		memberName := node.Right.Value
		fieldType, exists := structType.Fields[memberName]
		if !exists {
			c.addError(fmt.Sprintf("field '%s' not found in struct %s", memberName, structType.Name))
			return &NoneType{}
		}
		return fieldType
	}

	c.addError(fmt.Sprintf("member access not supported on type %s", leftType.String()))
	return &NoneType{}
}
