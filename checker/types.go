package checker

import (
	"fmt"
	"strings"
)

// Type represents a type in the Basalt type system
type Type interface {
	String() string
	Equals(other Type) bool
}

// Basic types
type (
	IntegerType struct{}
	FloatType   struct{}
	BooleanType struct{}
	StringType  struct{}
	NoneType    struct{}
)

// HashMapType represents a hash map type with key and value types
type HashMapType struct {
	KeyType   Type
	ValueType Type
}

func (t *HashMapType) String() string {
	return fmt.Sprintf("HashMap<%s, %s>", t.KeyType.String(), t.ValueType.String())
}

func (t *HashMapType) Equals(other Type) bool {
	if otherHashMap, ok := other.(*HashMapType); ok {
		return t.KeyType.Equals(otherHashMap.KeyType) && t.ValueType.Equals(otherHashMap.ValueType)
	}
	return false
}

// RawPointerType represents the special rawptr type for unsafe operations
type RawPointerType struct{}

func (t *RawPointerType) String() string         { return "rawptr" }
func (t *RawPointerType) Equals(other Type) bool { _, ok := other.(*RawPointerType); return ok }

// PointerType represents a pointer to another type (e.g., *ArcHeader)
type PointerType struct {
	InnerType Type
}

func (t *PointerType) String() string {
	return "*" + t.InnerType.String()
}

func (t *PointerType) Equals(other Type) bool {
	if otherPtr, ok := other.(*PointerType); ok {
		return t.InnerType.Equals(otherPtr.InnerType)
	}
	return false
}

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
	Members *TypeEnvironment // Each module has its own scope
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
