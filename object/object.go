package object

import "fmt"

type ObjectType string

const (
	INTEGER_OBJ      = "INTEGER"
	BOOLEAN_OBJ      = "BOOLEAN"
	RETURN_VALUE_OBJ = "RETURN_VALUE"
	SOME_OBJ         = "SOME"
	NONE_OBJ         = "NONE"
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

type Boolean struct {
	Value bool
}

func (b *Boolean) Type() ObjectType { return BOOLEAN_OBJ }
func (b *Boolean) Inspect() string  { return fmt.Sprintf("%t", b.Value) }

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

// --- Environment ---

type Environment struct {
	store map[string]Object
}

func NewEnvironment() *Environment {
	s := make(map[string]Object)
	return &Environment{store: s}
}

func (e *Environment) Get(name string) (Object, bool) {
	obj, ok := e.store[name]
	return obj, ok
}

func (e *Environment) Set(name string, val Object) Object {
	e.store[name] = val
	return val
}
