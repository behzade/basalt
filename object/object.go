package object

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
)

type ObjectType string

const (
	INTEGER_OBJ           = "INTEGER"
	FLOAT_OBJ             = "FLOAT"
	BOOLEAN_OBJ           = "BOOLEAN"
	STRING_OBJ            = "STRING"
	ARRAY_OBJ             = "ARRAY"
	SLICE_OBJ             = "SLICE"
	RETURN_VALUE_OBJ      = "RETURN_VALUE"
	SOME_OBJ              = "SOME"
	NONE_OBJ              = "NONE"
	FUNCTION_OBJ          = "FUNCTION"
	ERROR_OBJ             = "ERROR"
	MODULE_OBJ            = "MODULE"
	BUILTIN_OBJ           = "BUILTIN"
	STRUCT_DEFINITION_OBJ = "STRUCT_DEFINITION"
	STRUCT_INSTANCE_OBJ   = "STRUCT_INSTANCE"
)

// Object is the base interface for all types in the language.
type Object interface {
	Type() ObjectType
	Inspect() string
}

// Option is an interface that both Some and None will satisfy.
// This is a "marker interface" to signify that a type is part of the Option family.
type Option interface {
	Object
	option() // Marker method for the Option type.
}

// --- Concrete Value Types ---

type Integer struct {
	Value int64
}

func (i *Integer) Type() ObjectType { return INTEGER_OBJ }
func (i *Integer) Inspect() string  { return fmt.Sprintf("%d", i.Value) }

type Float struct {
	Value float64
}

func (f *Float) Type() ObjectType { return FLOAT_OBJ }
func (f *Float) Inspect() string  { return fmt.Sprintf("%f", f.Value) }

type Boolean struct {
	Value bool
}

func (b *Boolean) Type() ObjectType { return BOOLEAN_OBJ }
func (b *Boolean) Inspect() string  { return fmt.Sprintf("%t", b.Value) }

type String struct {
	Value string
}

func (s *String) Type() ObjectType { return STRING_OBJ }
func (s *String) Inspect() string  { return "\"" + s.Value + "\"" }

type Array struct {
	Elements []Object
}

func (ao *Array) Type() ObjectType { return ARRAY_OBJ }
func (ao *Array) Inspect() string {
	var out strings.Builder
	elements := []string{}
	for _, e := range ao.Elements {
		elements = append(elements, e.Inspect())
	}
	out.WriteString("[")
	out.WriteString(strings.Join(elements, ", "))
	out.WriteString("]")
	return out.String()
}

type Slice struct {
	Elements []Object
}

func (so *Slice) Type() ObjectType { return SLICE_OBJ }
func (so *Slice) Inspect() string {
	var out strings.Builder
	elements := []string{}
	for _, e := range so.Elements {
		elements = append(elements, e.Inspect())
	}
	out.WriteString("[")
	out.WriteString(strings.Join(elements, ", "))
	out.WriteString("]")
	return out.String()
}

// --- Wrapper & Option Types ---

type ReturnValue struct {
	Value Object
}

func (rv *ReturnValue) Type() ObjectType { return RETURN_VALUE_OBJ }
func (rv *ReturnValue) Inspect() string  { return rv.Value.Inspect() }

// Some represents the presence of a value.
type Some struct {
	Value Object
}

func (s *Some) Type() ObjectType { return SOME_OBJ }
func (s *Some) Inspect() string  { return s.Value.Inspect() }
func (s *Some) option()          {} // Implements the Option interface

// None represents the absence of a value.
type None struct{}

func (n *None) Type() ObjectType { return NONE_OBJ }
func (n *None) Inspect() string  { return "None" }
func (n *None) option()          {} // Implements the Option interface

// Error represents a runtime error.
type Error struct {
	Message string
}

func (e *Error) Type() ObjectType { return ERROR_OBJ }
func (e *Error) Inspect() string  { return e.Message }

// Function represents a function value.
type Function struct {
	Parameters []*ast.Identifier
	Body       *ast.BlockStatement
	Env        *Environment
}

func (f *Function) Type() ObjectType { return FUNCTION_OBJ }
func (f *Function) Inspect() string {
	var out strings.Builder
	params := []string{}
	for _, p := range f.Parameters {
		params = append(params, p.String())
	}
	out.WriteString("fn")
	out.WriteString("(")
	out.WriteString(strings.Join(params, ", "))
	out.WriteString(") {\n")
	out.WriteString(f.Body.String())
	out.WriteString("\n}")
	return out.String()
}

// Builtin represents a built-in function.
type BuiltinFunction func(args ...Object) Object

type Builtin struct {
	Fn BuiltinFunction
}

func (b *Builtin) Type() ObjectType { return BUILTIN_OBJ }
func (b *Builtin) Inspect() string  { return "builtin function" }

// Module represents a loaded module containing exported functions and values.
type Module struct {
	Env *Environment
}

func (m *Module) Type() ObjectType { return MODULE_OBJ }
func (m *Module) Inspect() string  { return "<module>" }

// StructDefinition represents a struct type definition with field names and types
type StructDefinition struct {
	Fields map[string]string // field name -> type name (e.g., "a" -> "int64")
}

func (sd *StructDefinition) Type() ObjectType { return STRUCT_DEFINITION_OBJ }
func (sd *StructDefinition) Inspect() string {
	var out strings.Builder
	fields := []string{}
	for name, typeName := range sd.Fields {
		fields = append(fields, name+": "+typeName)
	}
	out.WriteString("struct { ")
	out.WriteString(strings.Join(fields, ", "))
	out.WriteString(" }")
	return out.String()
}

// StructInstance represents an instance of a struct with actual field values
type StructInstance struct {
	Definition *StructDefinition // Reference to the struct definition
	Fields     map[string]Object // field name -> field value
}

func (si *StructInstance) Type() ObjectType { return STRUCT_INSTANCE_OBJ }
func (si *StructInstance) Inspect() string {
	var out strings.Builder
	fields := []string{}
	for name, value := range si.Fields {
		fields = append(fields, name+": "+value.Inspect())
	}
	out.WriteString("{ ")
	out.WriteString(strings.Join(fields, ", "))
	out.WriteString(" }")
	return out.String()
}

// --- Environment ---

type binding struct {
	Value   Object
	Mutable bool
}

type Environment struct {
	store map[string]binding
	outer *Environment
}

func NewEnvironment() *Environment {
	s := make(map[string]binding)
	return &Environment{store: s, outer: nil}
}

func NewEnclosedEnvironment(outer *Environment) *Environment {
	env := NewEnvironment()
	env.outer = outer
	return env
}

func (e *Environment) Get(name string) (Object, bool) {
	binding, ok := e.store[name]
	if !ok && e.outer != nil {
		return e.outer.Get(name)
	}
	return binding.Value, ok
}

func (e *Environment) Set(name string, val Object, mutable bool) Object {
	e.store[name] = binding{Value: val, Mutable: mutable}
	return val
}

func (e *Environment) Reassign(name string, val Object) Object {
	b, ok := e.store[name]
	if !ok && e.outer != nil {
		return e.outer.Reassign(name, val)
	}

	if !ok {
		return &Error{Message: fmt.Sprintf("identifier not found: %s", name)}
	}

	if !b.Mutable {
		return &Error{Message: fmt.Sprintf("cannot reassign immutable variable: %s", name)}
	}

	e.store[name] = binding{Value: val, Mutable: b.Mutable}
	return val
}
