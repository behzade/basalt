package compiler

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/types"
)

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

	alloca := c.createEntryAlloca(targetType)

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

// compileExternStatement compiles an extern statement
func (c *Compiler) compileExternStatement(stmt *ast.ExternStatement) error {
	funcName := stmt.Function.Value

	// Check if the function is already in the function table to avoid redefinition
	if _, exists := c.functionTable[funcName]; exists {
		return nil // Already declared, so we can skip it.
	}

	// Create a slice to hold the LLVM parameter objects (*ir.Param).
	var llvmParams []*ir.Param

	// Loop through the parameters from the AST.
	for _, p := range stmt.Parameters {
		// For each parameter, get its name and LLVM type.
		paramName := p.Name.Value
		paramType := c.typeAnnotationToLLVMType(p.Type)

		// Create a single LLVM parameter with ir.NewParam and add it to the slice.
		llvmParams = append(llvmParams, ir.NewParam(paramName, paramType))
	}

	// Get the return type.
	returnType := c.typeAnnotationToLLVMType(stmt.ReturnType)

	// Declare the function using the correctly constructed slice of parameters.
	// The '...' unpacks the llvmParams slice for the variadic function call.
	fn := c.module.NewFunc(funcName, returnType, llvmParams...)

	// Set the Variadic property if this is a variadic function
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

// compileImportStatement compiles an import statement
func (c *Compiler) compileImportStatement(stmt *ast.ImportStatement) error {
	// Import statements are handled at the module resolution level
	// The actual module code is already included in the program
	// So we just need to create a module namespace entry

	// For now, we don't need to do anything special in the compiler
	// The module functions are already compiled as part of the program
	return nil
}

// compileUnsafeStatement compiles an unsafe statement
func (c *Compiler) compileUnsafeStatement(stmt *ast.UnsafeStatement) error {
	// Unsafe blocks are just regular blocks from the compiler's perspective
	// The type checker has already enforced the safety rules
	_, err := c.compileBlockStatement(stmt.Body)
	return err
}

// compileModuleStatement compiles a module statement
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

// compileLetAssignment compiles a let assignment for regular variables
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

	alloca := c.createEntryAlloca(targetType)
	c.currentBlock.NewStore(val, alloca)
	c.symbolTable[stmt.Name.Value] = alloca
	return nil
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
