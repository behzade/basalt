package evaluator

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/object"
)

var (
	TRUE  = &object.Boolean{Value: true}
	FALSE = &object.Boolean{Value: false}
	NONE  = &object.None{}
)

// Eval evaluates an AST node.
func Eval(node ast.Node, env *object.Environment) object.Object {
	switch node := node.(type) {
	// Statements
	case *ast.Program:
		return evalProgram(node, env)
	case *ast.ExpressionStatement:
		return Eval(node.Expression, env)
	case *ast.ReturnStatement:
		val := Eval(node.ReturnValue, env)
		return &object.ReturnValue{Value: val}
	case *ast.LetStatement:
		val := Eval(node.Value, env)
		env.Set(node.Name.Value, val)
		// A let statement itself doesn't yield a value in an expression context,
		// so we return NONE. The final value of a program will be the last
		// expression statement, like `a;`
		return NONE

	// Expressions
	case *ast.IntegerLiteral:
		return &object.Some{Value: &object.Integer{Value: node.Value}}
	case *ast.Boolean:
		return &object.Some{Value: nativeBoolToBooleanObject(node.Value)}
	case *ast.Identifier:
		val, ok := env.Get(node.Value)
		if !ok {
			return NONE
		}
		return val
	}

	return NONE
}

func evalProgram(program *ast.Program, env *object.Environment) object.Object {
	var result object.Object

	for _, statement := range program.Statements {
		result = Eval(statement, env)

		// When a return statement is encountered, unwrap its value.
		if returnValue, ok := result.(*object.ReturnValue); ok {
			return returnValue.Value
		}
	}

	return result
}

func nativeBoolToBooleanObject(input bool) *object.Boolean {
	if input {
		return TRUE
	}
	return FALSE
}
