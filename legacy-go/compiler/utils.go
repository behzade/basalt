package compiler

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir"
	"github.com/llir/llvm/ir/types"
)

// createEntryAlloca creates an alloca instruction in the entry block
func (c *Compiler) createEntryAlloca(typ types.Type) *ir.InstAlloca {
	// The entry block is always the first block of a function.
	entryBlock := c.currentFunc.Blocks[0]

	// We need to insert the alloca before the first non-alloca instruction.
	// A simple and effective way is to insert it before the block's terminator.
	terminator := entryBlock.Term
	entryBlock.Term = nil // Temporarily remove the terminator

	// Create the alloca in the entry block.
	alloca := entryBlock.NewAlloca(typ)

	entryBlock.Term = terminator // Re-add the terminator

	return alloca
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
	if _, exists := c.typeRegistry[typeAnnotation.Value]; exists && !typeAnnotation.IsPointer {
		return types.NewPointer(baseType)
	}
	if _, exists := c.enumRegistry[typeAnnotation.Value]; exists && !typeAnnotation.IsPointer {
		return types.NewPointer(baseType)
	}

	return baseType
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

// compileImplementation compiles implementation statements (for two-pass compilation)
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

// collectDeclarations collects declarations for two-pass compilation
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
