package compiler

import (
	"github.com/llir/llvm/ir/types"
)

// StructInfo holds information about a struct type
type StructInfo struct {
	LLVMType   *types.StructType
	FieldNames []string              // Ordered list of field names
	FieldTypes map[string]types.Type // Maps field name to LLVM type
	FieldIndex map[string]int        // Maps field name to index in struct
}

// EnumVariantInfo holds information about an enum variant
type EnumVariantInfo struct {
	Tag         int32      // The tag value for this variant
	PayloadType types.Type // LLVM type for the payload (nil if no payload)
}

// EnumInfo holds information about an enum type
type EnumInfo struct {
	LLVMType    *types.StructType           // The LLVM struct type for the enum
	Variants    map[string]*EnumVariantInfo // Maps variant name to variant info
	TagToName   map[int32]string            // Maps tag values to variant names
	MaxDataSize int                         // Size of the largest variant payload
}
