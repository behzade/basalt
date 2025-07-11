package stdlib

import (
	"fmt"

	"github.com/behzade/basalt/object"
)

// Registry holds all the Go-implemented standard library modules
var Registry = map[string]*object.Module{
	"std::io": createIOModule(),
}

// createIOModule creates the std::io module with the puts function
func createIOModule() *object.Module {
	env := object.NewEnvironment()

	// Create the puts builtin function
	putsBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			for _, arg := range args {
				fmt.Println(arg.Inspect())
			}
			return &object.None{}
		},
	}

	env.Set("puts", putsBuiltin, false) // builtin functions are immutable

	// Add a VERSION constant
	env.Set("VERSION", &object.Integer{Value: 1}, false) // constants are immutable

	// Create and return the module
	return &object.Module{Env: env}
}
