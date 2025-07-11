package stdlib

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/object"
)

// Registry holds all the Go-implemented standard library modules
var Registry = map[string]*object.Module{
	"std::io": createIOModule(),
}

// Builtins holds all built-in functions
var Builtins = map[string]*object.Builtin{
	"len": {
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 1 {
				return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=1", len(args))}
			}

			arg := args[0]

			// Handle wrapped values (Some)
			if some, ok := arg.(*object.Some); ok {
				if str, ok := some.Value.(*object.String); ok {
					return &object.Some{Value: &object.Integer{Value: int64(len(str.Value))}}
				}
				return &object.Error{Message: fmt.Sprintf("argument to `len` not supported, got %s", some.Value.Type())}
			}

			return &object.Error{Message: fmt.Sprintf("argument to `len` not supported, got %s", arg.Type())}
		},
	},
}

// createIOModule creates the std::io module with the puts function
func createIOModule() *object.Module {
	env := object.NewEnvironment()

	// Create the puts built-in function
	putsBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			var out strings.Builder
			for i, arg := range args {
				if i > 0 {
					out.WriteString(" ")
				}
				out.WriteString(arg.Inspect())
			}
			fmt.Println(out.String())
			return &object.None{}
		},
	}

	// Add puts to the module's environment
	env.Set("puts", putsBuiltin)

	// Add VERSION constant for testing member access
	env.Set("VERSION", &object.Integer{Value: 1})

	// Create and return the module
	return &object.Module{Env: env}
}
