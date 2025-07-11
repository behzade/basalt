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

	// Create and return the module
	return &object.Module{Env: env}
}
