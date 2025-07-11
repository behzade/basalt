package evaluator

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/object"
	"github.com/behzade/basalt/stdlib"
)

var (
	TRUE  = &object.Boolean{Value: true}
	FALSE = &object.Boolean{Value: false}
	NONE  = &object.None{}
)

// isError checks if an object is an error
func isError(obj object.Object) bool {
	if obj != nil {
		return obj.Type() == object.ERROR_OBJ
	}
	return false
}

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
		if isError(val) {
			return val
		}
		return &object.ReturnValue{Value: val}
	case *ast.LetStatement:
		val := Eval(node.Value, env)
		if isError(val) {
			return val
		}
		env.Set(node.Name.Value, val)
		// A let statement itself doesn't yield a value in an expression context,
		// so we return NONE. The final value of a program will be the last
		// expression statement, like `a;`
		return NONE
	case *ast.ImportStatement:
		return evalImportStatement(node, env)

	// Expressions
	case *ast.IntegerLiteral:
		return &object.Some{Value: &object.Integer{Value: node.Value}}
	case *ast.Boolean:
		return &object.Some{Value: nativeBoolToBooleanObject(node.Value)}
	case *ast.StringLiteral:
		return &object.Some{Value: &object.String{Value: node.Value}}
	case *ast.ArrayLiteral:
		elements := evalArrayElements(node.Elements, env)
		if len(elements) == 1 && isError(elements[0]) {
			return elements[0]
		}
		return &object.Some{Value: &object.Array{Elements: elements}}
	case *ast.IndexExpression:
		left := Eval(node.Left, env)
		if isError(left) {
			return left
		}

		// Handle slicing vs simple indexing
		if node.IsSlice {
			// This is a slicing operation
			var start, end object.Object

			// Evaluate start (can be nil for [:end])
			if node.Start != nil {
				start = Eval(node.Start, env)
				if isError(start) {
					return start
				}
			}

			// Evaluate end (can be nil for [start:])
			if node.End != nil {
				end = Eval(node.End, env)
				if isError(end) {
					return end
				}
			}

			return evalSliceExpression(left, start, end)
		} else {
			// This is a simple indexing operation
			if node.Start == nil {
				return &object.Error{Message: "index expression missing"}
			}

			index := Eval(node.Start, env)
			if isError(index) {
				return index
			}

			return evalIndexExpression(left, index)
		}
	case *ast.PrefixExpression:
		right := Eval(node.Right, env)
		if isError(right) {
			return right
		}
		return evalPrefixExpression(node.Operator, right)
	case *ast.InfixExpression:
		left := Eval(node.Left, env)
		if isError(left) {
			return left
		}
		right := Eval(node.Right, env)
		if isError(right) {
			return right
		}
		return evalInfixExpression(node.Operator, left, right)
	case *ast.Identifier:
		val, ok := env.Get(node.Value)
		if !ok {
			return &object.Error{Message: fmt.Sprintf("identifier not found: %s", node.Value)}
		}
		return val
	case *ast.FunctionLiteral:
		params := node.Parameters
		body := node.Body
		return &object.Function{Parameters: params, Env: env, Body: body}
	case *ast.CallExpression:
		function := Eval(node.Function, env)
		if isError(function) {
			return function
		}
		args := evalArguments(node.Arguments, env)
		if len(args) == 1 && isError(args[0]) {
			return args[0]
		}
		return applyFunction(function, args)
	case *ast.IfExpression:
		return evalIfExpression(node, env)
	case *ast.BlockStatement:
		return evalBlockStatement(node, env)
	case *ast.MemberAccessExpression:
		left := Eval(node.Left, env)
		if isError(left) {
			return left
		}

		memberName := node.Right.Value

		// Handle method access on Some-wrapped values
		if some, ok := left.(*object.Some); ok {
			switch some.Value.Type() {
			case object.STRING_OBJ:
				if memberName == "len" {
					str := some.Value.(*object.String)
					return &object.Builtin{
						Fn: func(args ...object.Object) object.Object {
							if len(args) != 0 {
								return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=0", len(args))}
							}
							return &object.Some{Value: &object.Integer{Value: int64(len(str.Value))}}
						},
					}
				}
				return &object.Error{Message: fmt.Sprintf("undefined method '%s' on string", memberName)}
			case object.ARRAY_OBJ:
				if memberName == "len" {
					arr := some.Value.(*object.Array)
					return &object.Builtin{
						Fn: func(args ...object.Object) object.Object {
							if len(args) != 0 {
								return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=0", len(args))}
							}
							return &object.Some{Value: &object.Integer{Value: int64(len(arr.Elements))}}
						},
					}
				}
				return &object.Error{Message: fmt.Sprintf("undefined method '%s' on array", memberName)}
			case object.SLICE_OBJ:
				if memberName == "len" {
					slice := some.Value.(*object.Slice)
					return &object.Builtin{
						Fn: func(args ...object.Object) object.Object {
							if len(args) != 0 {
								return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=0", len(args))}
							}
							return &object.Some{Value: &object.Integer{Value: int64(len(slice.Elements))}}
						},
					}
				}
				return &object.Error{Message: fmt.Sprintf("undefined method '%s' on slice", memberName)}
			}
		}

		// Handle module access (original behavior)
		if module, ok := left.(*object.Module); ok {
			member, exists := module.Env.Get(memberName)
			if !exists {
				return &object.Error{Message: fmt.Sprintf("undefined member '%s' on module", memberName)}
			}
			return member
		}

		return &object.Error{Message: fmt.Sprintf("member access not supported on type %s", left.Type())}
	}

	return &object.Error{Message: "unknown node type"}
}

func evalProgram(program *ast.Program, env *object.Environment) object.Object {
	var result object.Object

	for _, statement := range program.Statements {
		result = Eval(statement, env)

		// Check for errors first
		if isError(result) {
			return result
		}

		// When a return statement is encountered, unwrap its value.
		if returnValue, ok := result.(*object.ReturnValue); ok {
			return returnValue.Value
		}
	}

	return result
}

func evalBlockStatement(block *ast.BlockStatement, env *object.Environment) object.Object {
	var result object.Object

	for _, statement := range block.Statements {
		result = Eval(statement, env)

		// Check for errors first
		if isError(result) {
			return result
		}

		// When a return statement is encountered, return it wrapped
		// (unlike evalProgram, we don't unwrap it here to let the caller handle it)
		if returnValue, ok := result.(*object.ReturnValue); ok {
			return returnValue
		}
	}

	return result
}

func evalIfExpression(ie *ast.IfExpression, env *object.Environment) object.Object {
	condition := Eval(ie.Condition, env)
	if isError(condition) {
		return condition
	}

	if isTruthy(condition) {
		return Eval(ie.Consequence, env)
	} else if ie.Alternative != nil {
		return Eval(ie.Alternative, env)
	} else {
		return NONE
	}
}

func isTruthy(obj object.Object) bool {
	switch obj {
	case NONE:
		return false
	case FALSE:
		return false
	default:
		// For wrapped values (Some), check the inner value
		if some, ok := obj.(*object.Some); ok {
			switch some.Value {
			case FALSE:
				return false
			default:
				// All other values (including integers, even 0) are truthy
				return true
			}
		}
		return true
	}
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
		return &object.Error{Message: fmt.Sprintf("unknown operator: %s", operator)}
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
	// Return error for non-integer values
	return &object.Error{Message: fmt.Sprintf("unknown operator: -%s", right.Type())}
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
			// Handle string infix expressions
			if leftStr, ok := leftSome.Value.(*object.String); ok {
				if rightStr, ok := rightSome.Value.(*object.String); ok {
					return evalStringInfixExpression(operator, leftStr, rightStr)
				}
			}
			// Handle boolean comparisons
			if leftBool, ok := leftSome.Value.(*object.Boolean); ok {
				if rightBool, ok := rightSome.Value.(*object.Boolean); ok {
					return evalBooleanInfixExpression(operator, leftBool, rightBool)
				}
			}
			// If we get here, we have a type mismatch within Some objects
			return &object.Error{Message: fmt.Sprintf("type mismatch: %s %s %s", leftSome.Value.Type(), operator, rightSome.Value.Type())}
		}
	}

	// Return error for unsupported operand types
	return &object.Error{Message: fmt.Sprintf("type mismatch: %s %s %s", left.Type(), operator, right.Type())}
}

func evalStringInfixExpression(operator string, left, right *object.String) object.Object {
	leftVal := left.Value
	rightVal := right.Value

	switch operator {
	case "+":
		return &object.Some{Value: &object.String{Value: leftVal + rightVal}}
	case "==":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal == rightVal)}
	case "!=":
		return &object.Some{Value: nativeBoolToBooleanObject(leftVal != rightVal)}
	default:
		return &object.Error{Message: fmt.Sprintf("unknown operator: %s", operator)}
	}
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
		return &object.Error{Message: fmt.Sprintf("unknown operator: %s", operator)}
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
		return &object.Error{Message: fmt.Sprintf("unknown operator: %s", operator)}
	}
}

func evalArguments(exps []ast.Expression, env *object.Environment) []object.Object {
	result := []object.Object{}
	for _, e := range exps {
		evaluated := Eval(e, env)
		if isError(evaluated) {
			return []object.Object{evaluated}
		}
		result = append(result, evaluated)
	}
	return result
}

func applyFunction(fn object.Object, args []object.Object) object.Object {
	switch fn := fn.(type) {
	case *object.Function:
		extendedEnv := extendFunctionEnv(fn, args)
		evaluated := Eval(fn.Body, extendedEnv)
		return unwrapReturnValue(evaluated)
	case *object.Builtin:
		return fn.Fn(args...)
	default:
		return &object.Error{Message: fmt.Sprintf("not a function: %s", fn.Type())}
	}
}

func extendFunctionEnv(fn *object.Function, args []object.Object) *object.Environment {
	env := object.NewEnclosedEnvironment(fn.Env)

	for paramIdx, param := range fn.Parameters {
		env.Set(param.Value, args[paramIdx])
	}

	return env
}

func unwrapReturnValue(obj object.Object) object.Object {
	if returnValue, ok := obj.(*object.ReturnValue); ok {
		return returnValue.Value
	}
	return obj
}

func evalImportStatement(importStmt *ast.ImportStatement, env *object.Environment) object.Object {
	// Convert the path expression to a string key (e.g., "std::io")
	modulePath := importStmt.Path.String()

	// Look up the module in the standard library registry
	module, ok := stdlib.Registry[modulePath]
	if !ok {
		return &object.Error{Message: fmt.Sprintf("module not found: %s", modulePath)}
	}

	// Determine the variable name for the import
	var variableName string
	if importStmt.Alias != nil {
		// Use the alias if provided
		variableName = importStmt.Alias.Value
	} else {
		// Use the last part of the module path (e.g., "io" from "std::io")
		pathSegments := importStmt.Path.Segments
		if len(pathSegments) > 0 {
			variableName = pathSegments[len(pathSegments)-1].Value
		} else {
			return &object.Error{Message: "invalid module path"}
		}
	}

	// Bind the module object to the variable name in the current environment
	env.Set(variableName, module)

	// The import statement itself evaluates to NONE
	return NONE
}

func evalArrayElements(elems []ast.Expression, env *object.Environment) []object.Object {
	result := []object.Object{}

	for _, e := range elems {
		evaluated := Eval(e, env)
		if isError(evaluated) {
			return []object.Object{evaluated}
		}
		result = append(result, evaluated)
	}

	// Check that all elements are of the same type
	if len(result) > 0 {
		firstType := getActualType(result[0])
		for i := 1; i < len(result); i++ {
			if getActualType(result[i]) != firstType {
				return []object.Object{&object.Error{Message: "array elements must be of the same type"}}
			}
		}
	}

	return result
}

func evalIndexExpression(left, index object.Object) object.Object {
	switch {
	case left.Type() == object.SOME_OBJ && index.Type() == object.SOME_OBJ:
		leftSome := left.(*object.Some)
		indexSome := index.(*object.Some)

		if leftSome.Value.Type() == object.ARRAY_OBJ && indexSome.Value.Type() == object.INTEGER_OBJ {
			return evalArrayIndexExpression(leftSome.Value, indexSome.Value)
		}

		return &object.Error{Message: fmt.Sprintf("index operator not supported: %s", leftSome.Value.Type())}
	default:
		return &object.Error{Message: fmt.Sprintf("index operator not supported: %s", left.Type())}
	}
}

func evalArrayIndexExpression(array, index object.Object) object.Object {
	arrayObject := array.(*object.Array)
	idx := index.(*object.Integer).Value
	max := int64(len(arrayObject.Elements) - 1)

	if idx < 0 || idx > max {
		return &object.Error{Message: "index out of bounds"}
	}

	return arrayObject.Elements[idx]
}

func evalSliceExpression(left, start, end object.Object) object.Object {
	// Handle wrapped values (Some objects)
	if some, ok := left.(*object.Some); ok {
		switch some.Value.Type() {
		case object.ARRAY_OBJ:
			return evalArraySliceExpression(some.Value, start, end)
		case object.STRING_OBJ:
			return evalStringSliceExpression(some.Value, start, end)
		default:
			return &object.Error{Message: fmt.Sprintf("slice operator not supported: %s", some.Value.Type())}
		}
	}

	return &object.Error{Message: fmt.Sprintf("slice operator not supported: %s", left.Type())}
}

func evalArraySliceExpression(array, start, end object.Object) object.Object {
	arrayObject := array.(*object.Array)

	var startIdx, endIdx int64

	// Handle start index
	if start != nil {
		if some, ok := start.(*object.Some); ok {
			if integer, ok := some.Value.(*object.Integer); ok {
				startIdx = integer.Value
			} else {
				return &object.Error{Message: "start index must be an integer"}
			}
		} else {
			return &object.Error{Message: "start index must be an integer"}
		}
	} else {
		startIdx = 0
	}

	// Handle end index
	if end != nil {
		if some, ok := end.(*object.Some); ok {
			if integer, ok := some.Value.(*object.Integer); ok {
				endIdx = integer.Value
			} else {
				return &object.Error{Message: "end index must be an integer"}
			}
		} else {
			return &object.Error{Message: "end index must be an integer"}
		}
	} else {
		endIdx = int64(len(arrayObject.Elements))
	}

	// Bounds checking
	if startIdx < 0 || startIdx > int64(len(arrayObject.Elements)) {
		return &object.Error{Message: "start index out of bounds"}
	}
	if endIdx < 0 || endIdx > int64(len(arrayObject.Elements)) {
		return &object.Error{Message: "end index out of bounds"}
	}
	if startIdx > endIdx {
		return &object.Some{Value: &object.Slice{Elements: []object.Object{}}}
	}

	// Create slice with elements from start to end (exclusive)
	sliceElements := make([]object.Object, endIdx-startIdx)
	copy(sliceElements, arrayObject.Elements[startIdx:endIdx])

	return &object.Some{Value: &object.Slice{Elements: sliceElements}}
}

func evalStringSliceExpression(str, start, end object.Object) object.Object {
	stringObject := str.(*object.String)

	var startIdx, endIdx int64

	// Handle start index
	if start != nil {
		if some, ok := start.(*object.Some); ok {
			if integer, ok := some.Value.(*object.Integer); ok {
				startIdx = integer.Value
			} else {
				return &object.Error{Message: "start index must be an integer"}
			}
		} else {
			return &object.Error{Message: "start index must be an integer"}
		}
	} else {
		startIdx = 0
	}

	// Handle end index
	if end != nil {
		if some, ok := end.(*object.Some); ok {
			if integer, ok := some.Value.(*object.Integer); ok {
				endIdx = integer.Value
			} else {
				return &object.Error{Message: "end index must be an integer"}
			}
		} else {
			return &object.Error{Message: "end index must be an integer"}
		}
	} else {
		endIdx = int64(len(stringObject.Value))
	}

	// Bounds checking
	if startIdx < 0 || startIdx > int64(len(stringObject.Value)) {
		return &object.Error{Message: "start index out of bounds"}
	}
	if endIdx < 0 || endIdx > int64(len(stringObject.Value)) {
		return &object.Error{Message: "end index out of bounds"}
	}
	if startIdx > endIdx {
		return &object.Some{Value: &object.String{Value: ""}}
	}

	// Create substring
	substring := stringObject.Value[startIdx:endIdx]

	return &object.Some{Value: &object.String{Value: substring}}
}

func getActualType(obj object.Object) object.ObjectType {
	if some, ok := obj.(*object.Some); ok {
		return some.Value.Type()
	}
	return obj.Type()
}
