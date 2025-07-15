package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
)

// Statement checking methods

func (c *Checker) checkProgram(program *ast.Program) Type {
	return c.checkStatements(program.Statements)
}

func (c *Checker) checkStatements(statements []ast.Statement) Type {
	// PASS 1: Collect all top-level definitions in the current environment.
	for _, stmt := range statements {
		c.collectTopLevelDeclarations(stmt)
	}

	// PASS 2: Type-check the statements in the current environment.
	var result Type = &NoneType{}
	for _, stmt := range statements {
		result = c.Check(stmt)
	}
	return result
}

func (c *Checker) collectTopLevelDeclarations(stmt ast.Statement) {
	switch s := stmt.(type) {
	case *ast.ModuleStatement:
		// Module declarations are handled in the main check loop, not pre-collected
	case *ast.LetStatement:
		// Handle struct definitions
		if structLit, ok := s.Value.(*ast.StructLiteral); ok {
			fields := make(map[string]Type)
			for _, field := range structLit.Fields {
				fieldName := field.Name.Value
				fieldType := c.parseTypeAnnotation(field.Type)
				fields[fieldName] = fieldType
			}
			structType := &StructType{Name: s.Name.Value, Fields: fields}
			c.env.Set(s.Name.Value, structType)
		}

		// Handle enum definitions
		if enumLit, ok := s.Value.(*ast.EnumLiteral); ok {
			enumType := c.checkEnumLiteral(enumLit)
			if enumTypeVal, ok := enumType.(*EnumType); ok {
				enumTypeVal.Name = s.Name.Value
				c.env.Set(s.Name.Value, enumTypeVal)
			}
		}

		// Handle function definitions
		if funcLit, ok := s.Value.(*ast.FunctionLiteral); ok {
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
			c.env.Set(s.Name.Value, funcType)
		}

	case *ast.ExternStatement:
		// Handle extern function declarations
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
		c.env.Set(s.Function.Value, funcType)
	}
}

func (c *Checker) checkModuleStatement(node *ast.ModuleStatement) Type {
	// Create a new, isolated environment for the module's contents.
	// It can see the outer scope (e.g., for global types if you had them) but only defines in its own scope.
	moduleEnv := NewEnclosedTypeEnvironment(c.env)
	moduleType := &ModuleType{
		Name:    node.Name.Value,
		Members: moduleEnv, // The module's type points to its own environment
	}

	// Temporarily switch the checker's context to this new environment.
	savedEnv := c.env
	c.env = moduleEnv

	// Recursively check the module's program using the isolated environment.
	c.checkStatements(node.Module.Statements)

	// Restore the original environment.
	c.env = savedEnv

	// Add the fully-checked module type to the current scope.
	c.env.Set(node.Name.Value, moduleType)

	return &NoneType{}
}

func (c *Checker) checkLetStatement(node *ast.LetStatement) Type {
	// Structs and enums were fully defined in pass 1. We can skip them here.
	if _, ok := node.Value.(*ast.StructLiteral); ok {
		return &NoneType{}
	}
	if _, ok := node.Value.(*ast.EnumLiteral); ok {
		return &NoneType{}
	}

	// For functions, the signature is already in the environment.
	// Now, we check the body.
	if funcLit, ok := node.Value.(*ast.FunctionLiteral); ok {
		funcTypeVal, ok := c.env.Get(node.Name.Value)
		if !ok {
			// This should not happen if pass 1 worked correctly
			c.addError(fmt.Sprintf("internal error: function '%s' not found in pre-check pass", node.Name.Value), node.Token)
			return &NoneType{}
		}
		funcType := funcTypeVal.(*FunctionType)

		// Check function body in a new scope
		funcEnv := NewEnclosedTypeEnvironment(c.env)
		for i, param := range funcLit.Parameters {
			funcEnv.Set(param.Name.Value, funcType.Parameters[i])
		}
		savedEnv := c.env
		c.env = funcEnv
		bodyType := c.Check(funcLit.Body)
		c.env = savedEnv

		// If return type is explicit, check against inferred body type
		if funcLit.ReturnType != nil {
			if !c.isAssignable(bodyType, funcType.ReturnType) && bodyType.String() != "none" {
				c.addError(fmt.Sprintf("function body returns %s but expected %s", bodyType.String(), funcType.ReturnType.String()), node.Token)
			}
		} else { // Infer return type
			funcType.ReturnType = bodyType
		}

		return &NoneType{}
	}

	// Regular variable assignment
	valueType := c.Check(node.Value)

	// Check for illegal rawptr declaration
	if valueType.Equals(&RawPointerType{}) && !c.isInUnsafeContext {
		c.addError("variables of type rawptr can only be declared inside an unsafe block", node.Token)
	}

	if node.Type != nil {
		expectedType := c.parseTypeAnnotation(node.Type)
		if expectedType.Equals(&RawPointerType{}) && !c.isInUnsafeContext {
			c.addError("variables of type rawptr can only be declared inside an unsafe block", node.Token)
		}
		if !c.isAssignable(valueType, expectedType) {
			c.addError(fmt.Sprintf("type mismatch: cannot assign %s to variable of type %s", valueType.String(), expectedType.String()), node.Token)
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
