package compiler

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

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

// compileEnumLiteral compiles enum literal expressions (shouldn't be called directly)
func (c *Compiler) compileEnumLiteral(expr *ast.EnumLiteral) (value.Value, error) {
	// Enum literals should not be compiled directly as expressions
	// They are handled in let statements via compileEnumDefinition
	return nil, fmt.Errorf("enum literals must be assigned to a variable")
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

	// Get the GC memory allocator function.
	gcMalloc, ok := c.functionTable["GC_malloc"]
	if !ok {
		return nil, fmt.Errorf("GC_malloc function not found. Ensure it's declared via an extern statement")
	}

	// Calculate the size of the enum struct.
	enumSize := c.sizeOf(enumInfo.LLVMType)

	// Allocate memory on the heap using the garbage collector.
	mem := c.currentBlock.NewCall(gcMalloc, enumSize)

	// Cast the returned i8* to a pointer of the correct enum type.
	enumPtr := c.currentBlock.NewBitCast(mem, types.NewPointer(enumInfo.LLVMType))

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
