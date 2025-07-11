package stdlib

import (
	"strings"

	"github.com/behzade/basalt/object"
)

// createStringsModule creates the std::strings module with string manipulation functions
func createStringsModule() *object.Module {
	env := object.NewEnvironment()

	// split function - splits string by separator, returns Array of Strings
	splitBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 2 {
				return &object.Error{Message: "split() takes exactly 2 arguments"}
			}

			str, ok := args[0].(*object.String)
			if !ok {
				return &object.Error{Message: "split() first argument must be a string"}
			}

			sep, ok := args[1].(*object.String)
			if !ok {
				return &object.Error{Message: "split() second argument must be a string"}
			}

			parts := strings.Split(str.Value, sep.Value)
			elements := make([]object.Object, len(parts))
			for i, part := range parts {
				elements[i] = &object.String{Value: part}
			}

			return &object.Array{Elements: elements}
		},
	}

	// join function - joins Array of Strings with separator, returns String
	joinBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 2 {
				return &object.Error{Message: "join() takes exactly 2 arguments"}
			}

			arr, ok := args[0].(*object.Array)
			if !ok {
				return &object.Error{Message: "join() first argument must be an array"}
			}

			sep, ok := args[1].(*object.String)
			if !ok {
				return &object.Error{Message: "join() second argument must be a string"}
			}

			parts := make([]string, len(arr.Elements))
			for i, elem := range arr.Elements {
				str, ok := elem.(*object.String)
				if !ok {
					return &object.Error{Message: "join() array must contain only strings"}
				}
				parts[i] = str.Value
			}

			result := strings.Join(parts, sep.Value)
			return &object.String{Value: result}
		},
	}

	// contains function - checks if string contains substring, returns Boolean
	containsBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 2 {
				return &object.Error{Message: "contains() takes exactly 2 arguments"}
			}

			str, ok := args[0].(*object.String)
			if !ok {
				return &object.Error{Message: "contains() first argument must be a string"}
			}

			substr, ok := args[1].(*object.String)
			if !ok {
				return &object.Error{Message: "contains() second argument must be a string"}
			}

			result := strings.Contains(str.Value, substr.Value)
			return &object.Boolean{Value: result}
		},
	}

	// Set the functions in the environment
	env.Set("split", splitBuiltin, false)
	env.Set("join", joinBuiltin, false)
	env.Set("contains", containsBuiltin, false)

	// Create and return the module
	return &object.Module{Env: env}
}
