package compiler

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
	"github.com/llir/llvm/ir"
)

// compileArrayLiteral compiles an array literal [1, 2, 3]
func (c *Compiler) compileArrayLiteral(expr *ast.ArrayLiteral) (value.Value, error) {
	// Create a new array with initial capacity equal to the number of elements
	// For empty arrays, use a default capacity of 0
	initialCapacity := constant.NewInt(types.I64, int64(len(expr.Elements)))
	arrayNewFunc, ok := c.functionTable["basalt_array_new"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_new not found")
	}
	arrayPtr := c.currentBlock.NewCall(arrayNewFunc, initialCapacity)

	// Push each element to the array
	arrayPushFunc, ok := c.functionTable["basalt_array_push"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_push not found")
	}
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
	// Compile the left expression (array or hashmap)
	leftValue, err := c.compileExpression(expr.Left)
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

	// Check if this is a HashMap access by looking at the left expression type
	// For now, we'll assume any pointer type that's not an array is a HashMap
	// In a full implementation, we'd need better type information
	leftType := leftValue.Type()

	// If the left side is a pointer and the index is not an integer, it might be a HashMap
	if c.isPointerType(leftType) && !indexValue.Type().Equal(types.I64) {
		// This is likely a HashMap with non-integer keys
		hashmapGetFunc, ok := c.functionTable["std_collections_get"]
		if !ok {
			return nil, fmt.Errorf("Collections::get function not found")
		}
		return c.currentBlock.NewCall(hashmapGetFunc, leftValue, indexValue), nil
	}

	// If the left side is a pointer and the index is an integer, it could be either
	// We'll try HashMap first, then fall back to array
	if c.isPointerType(leftType) && indexValue.Type().Equal(types.I64) {
		// Try HashMap first
		hashmapGetFunc, ok := c.functionTable["std_collections_get"]
		if ok {
			// Check if this looks like a HashMap by trying to access it
			// For now, we'll assume it's a HashMap if the function exists
			return c.currentBlock.NewCall(hashmapGetFunc, leftValue, indexValue), nil
		}

		// Fall back to array access
		arrayGetFunc, ok := c.functionTable["basalt_array_get"]
		if ok {
			return c.currentBlock.NewCall(arrayGetFunc, leftValue, indexValue), nil
		}

		return nil, fmt.Errorf("neither Collections::get nor basalt_array_get functions found")
	}

	// For non-pointer types, this should be an array access
	if indexValue.Type() != types.I64 {
		return nil, fmt.Errorf("array index must be integer, got %s", indexValue.Type())
	}

	// Call basalt_array_get
	arrayGetFunc, ok := c.functionTable["basalt_array_get"]
	if !ok {
		return nil, fmt.Errorf("runtime function basalt_array_get not found")
	}
	return c.currentBlock.NewCall(arrayGetFunc, leftValue, indexValue), nil
}

// compileHashLiteral compiles hash map literals like {"key": value, 42: "answer"}
func (c *Compiler) compileHashLiteral(expr *ast.HashLiteral) (value.Value, error) {
	// For now, we'll create a simple HashMap for string->int64 or int64->string mappings
	// In a full implementation, we'd need to determine the key and value types from the pairs

	// Create a new HashMap by calling the Basalt HashMap::new function
	// We need to determine the key and value types from the pairs
	if len(expr.Pairs) == 0 {
		// Empty hash map - we'll need type annotation to determine the types
		// For now, return a placeholder pointer
		return constant.NewIntToPtr(constant.NewInt(types.I64, 0), types.I8Ptr), nil
	}

	// Analyze the first pair to determine types
	firstPair := expr.Pairs[0]
	keyValue, err := c.compileExpression(firstPair.Key)
	if err != nil {
		return nil, err
	}
	valueValue, err := c.compileExpression(firstPair.Value)
	if err != nil {
		return nil, err
	}

	// Determine the HashMap type based on key and value types
	keyType := keyValue.Type()
	valType := valueValue.Type()

	// For now, we'll support string keys and int64 values, or int64 keys and string values
	var hashmapNewFunc *ir.Func
	var hashmapSetFunc *ir.Func
	var ok bool

	if c.isStringType(keyType) && valType.Equal(types.I64) {
		// HashMap<string, int64>
		hashmapNewFunc, ok = c.functionTable["std_collections_new"]
		if !ok {
			return nil, fmt.Errorf("Collections::new function not found")
		}
		hashmapSetFunc, ok = c.functionTable["std_collections_set"]
		if !ok {
			return nil, fmt.Errorf("Collections::set function not found")
		}
	} else if keyType.Equal(types.I64) && c.isStringType(valType) {
		// HashMap<int64, string>
		hashmapNewFunc, ok = c.functionTable["std_collections_new"]
		if !ok {
			return nil, fmt.Errorf("Collections::new function not found")
		}
		hashmapSetFunc, ok = c.functionTable["std_collections_set"]
		if !ok {
			return nil, fmt.Errorf("Collections::set function not found")
		}
	} else {
		return nil, fmt.Errorf("unsupported HashMap key/value types: %s -> %s", keyType, valType)
	}

	// Create a new HashMap
	hashMapPtr := c.currentBlock.NewCall(hashmapNewFunc)

	// Set the first pair
	c.currentBlock.NewCall(hashmapSetFunc, hashMapPtr, keyValue, valueValue)

	// Set the remaining pairs
	for i := 1; i < len(expr.Pairs); i++ {
		pair := expr.Pairs[i]

		keyVal, err := c.compileExpression(pair.Key)
		if err != nil {
			return nil, err
		}
		valVal, err := c.compileExpression(pair.Value)
		if err != nil {
			return nil, err
		}

		// Type check - ensure all keys and values have consistent types
		if !keyVal.Type().Equal(keyType) {
			return nil, fmt.Errorf("inconsistent key types in hash map literal: expected %s, got %s", keyType, keyVal.Type())
		}
		if !valVal.Type().Equal(valType) {
			return nil, fmt.Errorf("inconsistent value types in hash map literal: expected %s, got %s", valType, valVal.Type())
		}

		// Set the key-value pair
		c.currentBlock.NewCall(hashmapSetFunc, hashMapPtr, keyVal, valVal)
	}

	return hashMapPtr, nil
}

// compileIndexAssignment compiles assignment to an index expression (e.g., map[key] = value or arr[index] = value)
func (c *Compiler) compileIndexAssignment(indexExpr *ast.IndexExpression, value value.Value) (value.Value, error) {
	// Compile the left expression (map or array)
	leftValue, err := c.compileExpression(indexExpr.Left)
	if err != nil {
		return nil, err
	}

	// Compile the index/key expression
	keyValue, err := c.compileExpression(indexExpr.Start)
	if err != nil {
		return nil, err
	}

	// Check if this is a HashMap assignment
	leftType := leftValue.Type()

	// For HashMap assignment, we need to call Collections::set
	if c.isPointerType(leftType) {
		// Try HashMap set first
		hashmapSetFunc, ok := c.functionTable["std_collections_set"]
		if ok {
			// Call Collections::set(map, key, value)
			c.currentBlock.NewCall(hashmapSetFunc, leftValue, keyValue, value)
			return value, nil
		}

		// If Collections::set is not available, this might be array assignment
		// For now, we don't support array element assignment
		return nil, fmt.Errorf("array element assignment not yet implemented")
	}

	return nil, fmt.Errorf("unsupported index assignment target type: %s", leftType)
}

// sizeOf returns the size of a given type in bytes as an i64 value.
func (c *Compiler) sizeOf(typ types.Type) value.Value {
	// To get the size of a type, we use a getelementptr instruction.
	// GEP on a null pointer of type T* with an index of 1 gives a pointer
	// to an address that is sizeof(T) bytes away from NULL.
	// We then convert this pointer to an integer to get the size.
	nullPtr := constant.NewNull(types.NewPointer(typ))
	gep := c.currentBlock.NewGetElementPtr(typ, nullPtr, constant.NewInt(types.I64, 1))
	return c.currentBlock.NewPtrToInt(gep, types.I64)
}

func (c *Compiler) compileMemberAccessExpression(expr *ast.MemberAccessExpression) (value.Value, error) {
	// Compile the object being accessed
	objectValue, err := c.compileExpression(expr.Left)
	if err != nil {
		return nil, err
	}

	memberName := expr.Right.Value

	// Check if this is array.len - call basalt_array_len directly
	if memberName == "len" {
		// Check if objectValue is an array (array_ptr type)
		if objectValue.Type().Equal(types.I8Ptr) {
			// Call basalt_array_len
			arrayLenFunc, ok := c.functionTable["basalt_array_len"]
			if !ok {
				return nil, fmt.Errorf("runtime function basalt_array_len not found")
			}
			return c.currentBlock.NewCall(arrayLenFunc, objectValue), nil
		}
		// If it's a string, we could also support string.len here
		if c.isStringType(objectValue.Type()) {
			// Call strlen
			strlenFunc, ok := c.functionTable["strlen"]
			if !ok {
				return nil, fmt.Errorf("strlen function not found")
			}
			return c.currentBlock.NewCall(strlenFunc, objectValue), nil
		}
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
