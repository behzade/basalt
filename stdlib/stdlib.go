package stdlib

import (
	"fmt"
	"os"

	"github.com/behzade/basalt/object"
)

// Registry holds all the Go-implemented standard library modules
var Registry = map[string]*object.Module{
	"std::io":      createIOModule(),
	"std::strings": createStringsModule(),
}

// createIOModule creates the std::io module with I/O functions
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

	// print function - prints arguments to console, returns None
	printBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			for i, arg := range args {
				if i > 0 {
					fmt.Print(" ")
				}
				fmt.Print(arg.Inspect())
			}
			fmt.Println()
			return &object.None{}
		},
	}

	// read_file function - reads file content, returns Result<String, Error>
	readFileBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 1 {
				return object.NewErr("read_file() takes exactly 1 argument")
			}

			path, ok := args[0].(*object.String)
			if !ok {
				return object.NewErr("read_file() argument must be a string")
			}

			content, err := os.ReadFile(path.Value)
			if err != nil {
				return object.NewErr(fmt.Sprintf("failed to read file: %s", err.Error()))
			}

			return object.NewOk(&object.String{Value: string(content)})
		},
	}

	// write_file function - writes content to file, returns Result<None, Error>
	writeFileBuiltin := &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 2 {
				return object.NewErr("write_file() takes exactly 2 arguments")
			}

			path, ok := args[0].(*object.String)
			if !ok {
				return object.NewErr("write_file() first argument must be a string (path)")
			}

			content, ok := args[1].(*object.String)
			if !ok {
				return object.NewErr("write_file() second argument must be a string (content)")
			}

			err := os.WriteFile(path.Value, []byte(content.Value), 0644)
			if err != nil {
				return object.NewErr(fmt.Sprintf("failed to write file: %s", err.Error()))
			}

			return object.NewOk(&object.None{})
		},
	}

	env.Set("puts", putsBuiltin, false) // builtin functions are immutable
	env.Set("print", printBuiltin, false)
	env.Set("read_file", readFileBuiltin, false)
	env.Set("write_file", writeFileBuiltin, false)

	// Add a VERSION constant
	env.Set("VERSION", &object.Integer{Value: 1}, false) // constants are immutable

	// Create and return the module
	return &object.Module{Env: env}
}

// Helper functions for module creation and management

// create_module creates a new module with the given name
func create_module(name string) *object.Module {
	env := object.NewEnvironment()
	return &object.Module{Env: env}
}

// create_function creates a new builtin function and adds it to the module
func create_function(module *object.Module, name string, fn object.BuiltinFunction) {
	builtin := &object.Builtin{Fn: fn}
	module.Env.Set(name, builtin, false)
}

// build_return creates a return value wrapper for the given object
func build_return(value object.Object) *object.ReturnValue {
	return &object.ReturnValue{Value: value}
}
