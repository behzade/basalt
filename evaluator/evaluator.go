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
	case *ast.PrefixExpression:
		right := Eval(node.Right, env)
		return evalPrefixExpression(node.Operator, right)
	case *ast.InfixExpression:
		left := Eval(node.Left, env)
		right := Eval(node.Right, env)
		return evalInfixExpression(node.Operator, left, right)
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

func evalPrefixExpression(operator string, right object.Object) object.Object {
	switch operator {
	case "!":
		return evalBangOperatorExpression(right)
	case "-":
		return evalMinusPrefixOperatorExpression(right)
	default:
		return NONE
	}
}

func evalBangOperatorExpression(right object.Object) object.Object {
	switch right {
	case TRUE:
		return &object.Some{Value: FALSE}
	case FALSE:
		return &object.Some{Value: TRUE}
	case NONE:
		return &object.Some{Value: TRUE}
	default:
		// For wrapped values (Some), we need to check the inner value
		if some, ok := right.(*object.Some); ok {
			switch some.Value {
			case TRUE:
				return &object.Some{Value: FALSE}
			case FALSE:
				return &object.Some{Value: TRUE}
			default:
				// For any other value (integers, etc.), return FALSE
				return &object.Some{Value: TRUE}
			}
		}
		return &object.Some{Value: FALSE}
	}
}

func evalMinusPrefixOperatorExpression(right object.Object) object.Object {
	// Handle wrapped values
	if some, ok := right.(*object.Some); ok {
		if integer, ok := some.Value.(*object.Integer); ok {
			return &object.Some{Value: &object.Integer{Value: -integer.Value}}
		}
	}
	// For now, return NONE for non-integer values (could be an error object later)
	return NONE
}

func evalInfixExpression(operator string, left, right object.Object) object.Object {
	// Handle integer infix expressions
	if leftSome, ok := left.(*object.Some); ok {
		if rightSome, ok := right.(*object.Some); ok {
			if leftInt, ok := leftSome.Value.(*object.Integer); ok {
				if rightInt, ok := rightSome.Value.(*object.Integer); ok {
					return evalIntegerInfixExpression(operator, leftInt, rightInt)
				}
			}
			// Handle boolean comparisons
			if leftBool, ok := leftSome.Value.(*object.Boolean); ok {
				if rightBool, ok := rightSome.Value.(*object.Boolean); ok {
					return evalBooleanInfixExpression(operator, leftBool, rightBool)
				}
			}
		}
	}
	
	// For now, return NONE for unsupported operand types
	return NONE
}

func evalIntegerInfixExpression(operator string, left, right *object.Integer) object.Object {
	leftVal := left.Value
	rightVal := right.Value
	
	switch operator {
	case "+":
		return &object.Some{Value: &object.Integer{Value: leftVal + rightVal}}
	case "-":
		return &object.Some{Value: &object.Integer{Value: leftVal - rightVal}}
	case "*":
		return &object.Some{Value: &object.Integer{Value: leftVal * rightVal}}
	case "/":
		return &object.Some{Value: &object.Integer{Value: leftVal / rightVal}}
	case "<":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal < rightVal)}
	case ">":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal > rightVal)}
	case "==":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal == rightVal)}
	case "!=":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal != rightVal)}
	default:
		return NONE
	}
}

func evalBooleanInfixExpression(operator string, left, right *object.Boolean) object.Object {
	leftVal := left.Value
	rightVal := right.Value
	
	switch operator {
	case "==":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal == rightVal)}
	case "!=":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal != rightVal)}
	default:
		return NONE
	}
}
