package checker

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

// Utility methods for the checker

func (c *Checker) isAssignable(from, to Type) bool {
	if from.Equals(to) {
		return true
	}

	// Inside an unsafe block, allow casting between rawptr and any other pointer type.
	if c.isInUnsafeContext {
		_, isFromPtr := from.(*PointerType)
		_, isToPtr := to.(*PointerType)
		isFromRaw := from.Equals(&RawPointerType{})
		isToRaw := to.Equals(&RawPointerType{})
		isFromInt := from.Equals(&IntegerType{})

		if (isFromRaw && isToPtr) || (isFromPtr && isToRaw) {
			return true
		}
		// Allow integer to rawptr conversion in unsafe context
		if isFromInt && isToRaw {
			return true
		}
	}

	if from.Equals(&IntegerType{}) && to.Equals(&FloatType{}) {
		return true
	}
	if from.Equals(&ArrayType{ElementType: &NoneType{}}) {
		if _, ok := to.(*ArrayType); ok {
			return true
		}
	}

	// Allow empty hash map {} to be assigned to any hash map type
	if fromHashMap, ok := from.(*HashMapType); ok {
		if _, ok := to.(*HashMapType); ok {
			// Empty hash map can be assigned to any specific hash map type
			if fromHashMap.KeyType.Equals(&NoneType{}) && fromHashMap.ValueType.Equals(&NoneType{}) {
				return true
			}
		}
	}

	if from.Equals(&IntegerType{}) {
		if _, ok := to.(*PointerType); ok {
			return true
		}
	}
	return false
}

func (c *Checker) parseTypeAnnotation(typeAnnotation ast.Node) Type {
	var typeName string
	var isPointer bool
	var genericParams []*ast.TypeAnnotation

	switch ta := typeAnnotation.(type) {
	case *ast.TypeAnnotation:
		typeName = ta.Value
		isPointer = ta.IsPointer
		genericParams = ta.GenericParams
	case *ast.Identifier:
		typeName = ta.Value
		isPointer = false
	default:
		c.addError(fmt.Sprintf("invalid type annotation node: %T", typeAnnotation), token.Token{})
		return &NoneType{}
	}

	var innerType Type

	// Handle HashMap<K, V> generic type
	if typeName == "HashMap" && len(genericParams) == 2 {
		keyType := c.parseTypeAnnotation(genericParams[0])
		valueType := c.parseTypeAnnotation(genericParams[1])
		innerType = &HashMapType{KeyType: keyType, ValueType: valueType}
	} else if strings.Contains(typeName, "::") {
		parts := strings.Split(typeName, "::")
		if len(parts) != 2 {
			c.addError(fmt.Sprintf("invalid qualified type: %s", typeName), token.Token{})
			return &NoneType{}
		}
		moduleName, memberName := parts[0], parts[1]

		moduleVal, ok := c.env.Get(moduleName)
		if !ok {
			c.addError(fmt.Sprintf("unknown module: %s", moduleName), token.Token{})
			return &NoneType{}
		}
		moduleType, ok := moduleVal.(*ModuleType)
		if !ok {
			c.addError(fmt.Sprintf("'%s' is not a module", moduleName), token.Token{})
			return &NoneType{}
		}
		memberType, ok := moduleType.Members.Get(memberName)
		if !ok {
			c.addError(fmt.Sprintf("type '%s' not found in module '%s'", memberName, moduleName), token.Token{})
			return &NoneType{}
		}
		innerType = memberType
	} else {
		switch typeName {
		case "int64":
			innerType = &IntegerType{}
		case "float64":
			innerType = &FloatType{}
		case "bool":
			innerType = &BooleanType{}
		case "string":
			innerType = &StringType{}
		case "none":
			innerType = &NoneType{}
		case "rawptr":
			innerType = &RawPointerType{}
		case "HashMap":
			c.addError("HashMap requires generic parameters: HashMap<KeyType, ValueType>", token.Token{Type: token.IDENT, Literal: typeName})
			return &NoneType{}
		default:
			if typ, ok := c.env.Get(typeName); ok {
				innerType = typ
			} else {
				c.addError(fmt.Sprintf("unknown type: %s", typeName), token.Token{Type: token.IDENT, Literal: typeName})
				return &NoneType{}
			}
		}
	}

	if isPointer {
		return &PointerType{InnerType: innerType}
	}
	return innerType
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
