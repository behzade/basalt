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

// EnumVariantType represents a single variant in an enum
type EnumVariantType struct {
	Name        string
	PayloadType Type // nil if no payload
}

func (t *EnumVariantType) String() string {
	if t.PayloadType != nil {
		return fmt.Sprintf("%s(%s)", t.Name, t.PayloadType.String())
	}
	return t.Name
}

func (t *EnumVariantType) Equals(other Type) bool {
	if otherVariant, ok := other.(*EnumVariantType); ok {
		if t.Name != otherVariant.Name {
			return false
		}
		if t.PayloadType == nil && otherVariant.PayloadType == nil {
			return true
		}
		if t.PayloadType != nil && otherVariant.PayloadType != nil {
			return t.PayloadType.Equals(otherVariant.PayloadType)
		}
		return false
	}
	return false
}

// EnumType represents an enum type
type EnumType struct {
	Name     string
	Variants map[string]*EnumVariantType
}

func (t *EnumType) String() string {
	return t.Name
}

func (t *EnumType) Equals(other Type) bool {
	if otherEnum, ok := other.(*EnumType); ok {
		return t.Name == otherEnum.Name
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

func (c *Checker) addErrorWithLocation(message string, node ast.Node) {
	var location string
	if node != nil {
		// Get token information from the node
		switch n := node.(type) {
		case *ast.Identifier:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.LetStatement:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.CallExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.InfixExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.PrefixExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IntegerLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.FloatLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StringLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.Boolean:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ArrayLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IndexExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.MemberAccessExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IfExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ForExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.FunctionLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StructLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StructInstanceExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ExternStatement:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		}
	}
	c.errors = append(c.errors, &TypeError{Message: location + message})
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
	case *ast.StructLiteral:
		return c.checkStructLiteral(node)
	case *ast.StructInstanceExpression:
		return c.checkStructInstanceExpression(node)
	case *ast.ExternStatement:
		return c.checkExternStatement(node)
	case *ast.EnumLiteral:
		return c.checkEnumLiteral(node)
	case *ast.EnumInstantiationExpression:
		return c.checkEnumInstantiationExpression(node)
	case *ast.MatchExpression:
		return c.checkMatchExpression(node)
	default:
		c.addError(fmt.Sprintf("unknown node type: %T", node))
		return &NoneType{}
	}
}

func (c *Checker) checkProgram(program *ast.Program) Type {
	var result Type = &NoneType{}

	// First pass: collect all function definitions and associate them with modules
	c.collectModuleFunctions(program)

	// Second pass: type check everything
	for _, stmt := range program.Statements {
		result = c.Check(stmt)
	}
	return result
}

func (c *Checker) collectModuleFunctions(program *ast.Program) {
	var currentModule *ModuleType

	for _, stmt := range program.Statements {
		switch s := stmt.(type) {
		case *ast.ImportStatement:
			// Create or get the module
			moduleName := s.Path.Segments[len(s.Path.Segments)-1].Value
			if s.Alias != nil {
				moduleName = s.Alias.Value
			}

			moduleType := &ModuleType{
				Name:    moduleName,
				Members: make(map[string]Type),
			}
			c.env.Set(moduleName, moduleType)
			currentModule = moduleType

		case *ast.LetStatement:
			// If we're in a module context and this is a function, add it to the module
			if currentModule != nil {
				if funcLit, ok := s.Value.(*ast.FunctionLiteral); ok {
					// Create function type
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

					// Add to module
					currentModule.Members[s.Name.Value] = funcType
				}
			} else {
				// This is a regular let statement in the main program, reset module context
				currentModule = nil
			}

		case *ast.ExternStatement:
			// If we're in a module context, add extern function to the module
			if currentModule != nil {
				paramTypes := make([]Type, len(s.Parameters))
				for i, p := range s.Parameters {
					paramTypes[i] = c.parseTypeAnnotation(p.Type)
				}
				var returnType Type = &NoneType{}
				if s.ReturnType != nil {
					returnType = c.parseTypeAnnotation(s.ReturnType)
				}
				funcType := &FunctionType{
					Parameters: paramTypes,
					ReturnType: returnType,
					IsVariadic: s.IsVariadic,
				}

				// Add to module
				currentModule.Members[s.Function.Value] = funcType
			}

		case *ast.ExpressionStatement:
			// Expression statements from main program, reset module context
			currentModule = nil
		}
	}
}

func (c *Checker) checkImportStatement(node *ast.ImportStatement) Type {
	// Create module name from path
	moduleName := node.Path.Segments[len(node.Path.Segments)-1].Value

	// Use alias if provided, otherwise use the last segment of the path
	if node.Alias != nil {
		moduleName = node.Alias.Value
	}

	// Create a module type with empty members for now
	// The members will be populated as functions are defined in the module
	moduleType := &ModuleType{
		Name:    moduleName,
		Members: make(map[string]Type),
	}

	// Add the module to the environment
	c.env.Set(moduleName, moduleType)

	return &NoneType{}
}

func (c *Checker) checkLetStatement(node *ast.LetStatement) Type {
	// Special handling for struct definitions
	if structLit, ok := node.Value.(*ast.StructLiteral); ok {
		return c.checkStructDefinition(node.Name.Value, structLit)
	}

	// Special handling for enum definitions
	if enumLit, ok := node.Value.(*ast.EnumLiteral); ok {
		enumType := c.checkEnumLiteral(enumLit)
		if enumTypeVal, ok := enumType.(*EnumType); ok {
			enumTypeVal.Name = node.Name.Value
			c.env.Set(node.Name.Value, enumTypeVal)
		}
		return &NoneType{}
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
	c.addErrorWithLocation(fmt.Sprintf("identifier not found: %s", node.Value), node)
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

	// Type-check the non-variadic arguments
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

	if moduleType, ok := leftType.(*ModuleType); ok {
		memberName := node.Right.Value
		memberType, exists := moduleType.Members[memberName]
		if !exists {
			// Check if it's a function that might be defined later in the module
			// For now, we'll look it up in the global environment
			if typ, found := c.env.Get(memberName); found {
				// Add the member to the module for future reference
				moduleType.Members[memberName] = typ
				return typ
			}
			c.addError(fmt.Sprintf("member '%s' not found in module %s", memberName, moduleType.Name))
			return &NoneType{}
		}
		return memberType
	}

	c.addError(fmt.Sprintf("member access not supported on type %s", leftType.String()))
	return &NoneType{}
}

// checkEnumLiteral checks enum literal expressions and returns the enum type
func (c *Checker) checkEnumLiteral(node *ast.EnumLiteral) Type {
	variants := make(map[string]*EnumVariantType)

	for _, variant := range node.Variants {
		variantType := &EnumVariantType{
			Name:        variant.Name.Value,
			PayloadType: nil,
		}

		if variant.Payload != nil {
			variantType.PayloadType = c.parseTypeAnnotation(variant.Payload)
		}

		variants[variant.Name.Value] = variantType
	}

	// For enum literals, we create an anonymous enum type
	// The name will be set when it's assigned to a variable
	enumType := &EnumType{
		Name:     "", // Will be set during assignment
		Variants: variants,
	}

	return enumType
}

// checkEnumInstantiationExpression checks enum instantiation expressions
func (c *Checker) checkEnumInstantiationExpression(node *ast.EnumInstantiationExpression) Type {
	// Get the enum type from the environment
	enumName := node.Enum.Segments[0].Value
	enumTypeVal, ok := c.env.Get(enumName)
	if !ok {
		c.addError(fmt.Sprintf("unknown enum type: %s", enumName))
		return &NoneType{}
	}

	enumType, ok := enumTypeVal.(*EnumType)
	if !ok {
		c.addError(fmt.Sprintf("%s is not an enum type", enumName))
		return &NoneType{}
	}

	// Check if the variant exists
	variantName := node.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		c.addError(fmt.Sprintf("variant %s not found in enum %s", variantName, enumName))
		return &NoneType{}
	}

	// Check arguments match the variant's payload
	if variant.PayloadType == nil {
		if len(node.Arguments) > 0 {
			c.addError(fmt.Sprintf("variant %s::%s expects no arguments, got %d", enumName, variantName, len(node.Arguments)))
		}
	} else {
		if len(node.Arguments) != 1 {
			c.addError(fmt.Sprintf("variant %s::%s expects 1 argument, got %d", enumName, variantName, len(node.Arguments)))
		} else {
			argType := c.Check(node.Arguments[0])
			if !c.isAssignable(argType, variant.PayloadType) {
				c.addError(fmt.Sprintf("variant %s::%s expects argument of type %s, got %s", enumName, variantName, variant.PayloadType.String(), argType.String()))
			}
		}
	}

	return enumType
}

// checkMatchExpression checks match expressions for exhaustiveness and type consistency
func (c *Checker) checkMatchExpression(node *ast.MatchExpression) Type {
	// Check the condition type
	conditionType := c.Check(node.Condition)
	enumType, ok := conditionType.(*EnumType)
	if !ok {
		c.addError(fmt.Sprintf("match expression can only be used with enum types, got %s", conditionType.String()))
		return &NoneType{}
	}

	// Track which variants are covered
	coveredVariants := make(map[string]bool)
	var armTypes []Type

	for _, arm := range node.Arms {
		// Check the pattern
		patternType := c.checkMatchPattern(arm.Pattern, enumType)
		if patternType == nil {
			continue
		}

		// Mark variant as covered
		variantName := arm.Pattern.Variant.Value
		coveredVariants[variantName] = true

		// Check the consequence in a new scope with pattern variables
		consequenceEnv := NewEnclosedTypeEnvironment(c.env)
		c.addPatternVariables(arm.Pattern, enumType, consequenceEnv)

		savedEnv := c.env
		c.env = consequenceEnv
		armType := c.Check(arm.Consequence)
		c.env = savedEnv

		armTypes = append(armTypes, armType)
	}

	// Check exhaustiveness
	for variantName := range enumType.Variants {
		if !coveredVariants[variantName] {
			c.addError(fmt.Sprintf("match expression is not exhaustive: missing variant %s::%s", enumType.Name, variantName))
		}
	}

	// Check that all arms return the same type
	if len(armTypes) == 0 {
		return &NoneType{}
	}

	firstType := armTypes[0]
	for i, armType := range armTypes {
		if !armType.Equals(firstType) {
			c.addError(fmt.Sprintf("match arm %d returns type %s, expected %s", i+1, armType.String(), firstType.String()))
		}
	}

	return firstType
}

// checkMatchPattern checks a match pattern and returns the variant type
func (c *Checker) checkMatchPattern(pattern *ast.EnumInstantiationExpression, enumType *EnumType) *EnumVariantType {
	// Check if the pattern refers to the correct enum
	patternEnumName := pattern.Enum.Segments[0].Value
	if patternEnumName != enumType.Name {
		c.addError(fmt.Sprintf("pattern uses enum %s, but matching against %s", patternEnumName, enumType.Name))
		return nil
	}

	// Check if the variant exists
	variantName := pattern.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		c.addError(fmt.Sprintf("variant %s not found in enum %s", variantName, enumType.Name))
		return nil
	}

	// Check pattern arguments
	if variant.PayloadType == nil {
		if len(pattern.Arguments) > 0 {
			c.addError(fmt.Sprintf("variant %s::%s has no payload, but pattern has %d arguments", enumType.Name, variantName, len(pattern.Arguments)))
		}
	} else {
		if len(pattern.Arguments) != 1 {
			c.addError(fmt.Sprintf("variant %s::%s expects 1 pattern argument, got %d", enumType.Name, variantName, len(pattern.Arguments)))
		} else {
			// Pattern arguments should be identifiers (pattern variables)
			if _, ok := pattern.Arguments[0].(*ast.Identifier); !ok {
				c.addError(fmt.Sprintf("pattern argument must be an identifier, got %T", pattern.Arguments[0]))
			}
		}
	}

	return variant
}

// addPatternVariables adds pattern variables to the environment
func (c *Checker) addPatternVariables(pattern *ast.EnumInstantiationExpression, enumType *EnumType, env *TypeEnvironment) {
	variantName := pattern.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		return
	}

	if variant.PayloadType != nil && len(pattern.Arguments) == 1 {
		if ident, ok := pattern.Arguments[0].(*ast.Identifier); ok {
			env.Set(ident.Value, variant.PayloadType)
		}
	}
}
