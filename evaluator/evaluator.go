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

	// Built-in Result constructors
	RESULT_OK = &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 1 {
				return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=1", len(args))}
			}
			return &object.Result{Value: args[0], Err: nil}
		},
	}

	RESULT_ERR = &object.Builtin{
		Fn: func(args ...object.Object) object.Object {
			if len(args) != 1 {
				return &object.Error{Message: fmt.Sprintf("wrong number of arguments. got=%d, want=1", len(args))}
			}
			// Extract string from Some object if needed
			var message string
			if some, ok := args[0].(*object.Some); ok {
				if str, ok := some.Value.(*object.String); ok {
					message = str.Value
				} else {
					message = some.Value.Inspect()
				}
			} else if str, ok := args[0].(*object.String); ok {
				message = str.Value
			} else {
				message = args[0].Inspect()
			}
			return &object.Result{Value: nil, Err: &object.Error{Message: message}}
		},
	}
)

func setupBuiltins(env *object.Environment) {
	env.Set("Ok", RESULT_OK, false)
	env.Set("Err", RESULT_ERR, false)
}

// isError checks if an object is an error
func isError(obj object.Object) bool {
	if obj != nil {
		return obj.Type() == object.ERROR_OBJ
	}
	return false
}

// Eval evaluates an AST node.
func Eval(node ast.Node, env *object.Environment) object.Object {
	if node == nil {
		return &object.Error{Message: "cannot evaluate nil node"}
	}

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
		env.Set(node.Name.Value, val, node.Mutable)
		// A let statement itself doesn't yield a value in an expression context,
		// so we return NONE. The final value of a program will be the last
		// expression statement, like `a;`
		return NONE
	case *ast.ImportStatement:
		return evalImportStatement(node, env)

	// Expressions
	case *ast.IntegerLiteral:
		return &object.Some{Value: &object.Integer{Value: node.Value}}
	case *ast.FloatLiteral:
		return &object.Some{Value: &object.Float{Value: node.Value}}
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
	case *ast.HashLiteral:
		return evalHashLiteral(node, env)
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
		// Handle assignment operator specially
		if node.Operator == "=" {
			return evalAssignmentExpression(node.Left, node.Right, env)
		}

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
	case *ast.ForExpression:
		return evalForExpression(node, env)
	case *ast.BlockStatement:
		return evalBlockStatement(node, env)
	case *ast.StructLiteral:
		return evalStructLiteral(node, env)
	case *ast.StructInstanceExpression:
		return evalStructInstanceExpression(node, env)
	case *ast.PathExpression:
		// For now, just return an error since path expressions are used in import statements
		// and should be handled by the import statement evaluation
		return &object.Error{Message: "path expressions are not directly evaluable"}
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

		// Handle struct field access
		if structInstance, ok := left.(*object.StructInstance); ok {
			field, exists := structInstance.Fields[memberName]
			if !exists {
				return &object.Error{Message: fmt.Sprintf("field '%s' not found in struct", memberName)}
			}
			return &object.Some{Value: field}
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
	case *ast.ErrorPropagationExpression:
		return evalErrorPropagationExpression(node, env)
	}

	return &object.Error{Message: fmt.Sprintf("unknown node type: %T", node)}
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

func evalForExpression(fe *ast.ForExpression, env *object.Environment) object.Object {
	for {
		condition := Eval(fe.Condition, env)
		if isError(condition) {
			return condition
		}

		if !isTruthy(condition) {
			break
		}

		result := Eval(fe.Consequence, env)
		if isError(result) {
			return result
		}

		// If the evaluation of the consequence block results in a ReturnValue,
		// break the loop and immediately propagate that object up
		if returnValue, ok := result.(*object.ReturnValue); ok {
			return returnValue
		}
	}

	// If the loop completes normally, return NONE
	return NONE
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
		if float, ok := some.Value.(*object.Float); ok {
			return &object.Some{Value: &object.Float{Value: -float.Value}}
		}
	}
	// Return error for non-numeric values
	return &object.Error{Message: fmt.Sprintf("unknown operator: -%s", right.Type())}
}

func evalInfixExpression(operator string, left, right object.Object) object.Object {
	if leftSome, ok := left.(*object.Some); ok {
		if rightSome, ok := right.(*object.Some); ok {
			// Handle integer/integer operations
			if leftInt, ok := leftSome.Value.(*object.Integer); ok {
				if rightInt, ok := rightSome.Value.(*object.Integer); ok {
					return evalIntegerInfixExpression(operator, leftInt, rightInt)
				}
				// Handle mixed-mode: integer/float (promote integer to float)
				if rightFloat, ok := rightSome.Value.(*object.Float); ok {
					leftAsFloat := &object.Float{Value: float64(leftInt.Value)}
					return evalFloatInfixExpression(operator, leftAsFloat, rightFloat)
				}
			}
			// Handle float/float operations
			if leftFloat, ok := leftSome.Value.(*object.Float); ok {
				if rightFloat, ok := rightSome.Value.(*object.Float); ok {
					return evalFloatInfixExpression(operator, leftFloat, rightFloat)
				}
				// Handle mixed-mode: float/integer (promote integer to float)
				if rightInt, ok := rightSome.Value.(*object.Integer); ok {
					rightAsFloat := &object.Float{Value: float64(rightInt.Value)}
					return evalFloatInfixExpression(operator, leftFloat, rightAsFloat)
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

func evalAssignmentExpression(left ast.Expression, right ast.Expression, env *object.Environment) object.Object {
	// The left side must be an identifier
	identifier, ok := left.(*ast.Identifier)
	if !ok {
		return &object.Error{Message: "left side of assignment must be an identifier"}
	}

	// Evaluate the right side
	value := Eval(right, env)
	if isError(value) {
		return value
	}

	// Use the Reassign method to update the variable
	result := env.Reassign(identifier.Value, value)
	if isError(result) {
		return result
	}

	// Return the assigned value (already wrapped in Some if needed)
	return value
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

func evalFloatInfixExpression(operator string, left, right *object.Float) object.Object {
	leftVal := left.Value
	rightVal := right.Value

	switch operator {
	case "+":
		return &object.Some{Value: &object.Float{Value: leftVal + rightVal}}
	case "-":
		return &object.Some{Value: &object.Float{Value: leftVal - rightVal}}
	case "*":
		return &object.Some{Value: &object.Float{Value: leftVal * rightVal}}
	case "/":
		return &object.Some{Value: &object.Float{Value: leftVal / rightVal}}
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
	// Unwrap Some objects to get the actual function
	if some, ok := fn.(*object.Some); ok {
		fn = some.Value
	}

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
		env.Set(param.Value, args[paramIdx], false) // function parameters are immutable by default
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
	env.Set(variableName, module, false) // imported modules are immutable

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

func evalHashLiteral(node *ast.HashLiteral, env *object.Environment) object.Object {
	pairs := make(map[object.HashKey]object.HashPair)
	seenKeys := make(map[string]bool) // Track keys by their string representation

	var keyType, valueType object.ObjectType
	var keyTypeSet, valueTypeSet bool

	for _, pair := range node.Pairs {
		key := Eval(pair.Key, env)
		if isError(key) {
			return key
		}

		// Unwrap Some objects to get the actual key
		var actualKey object.Object
		if some, ok := key.(*object.Some); ok {
			actualKey = some.Value
		} else {
			actualKey = key
		}

		hashKey, ok := actualKey.(object.Hashable)
		if !ok {
			return &object.Error{Message: fmt.Sprintf("unusable as hash key: %s", actualKey.Type())}
		}

		// Check for duplicate keys
		keyStr := actualKey.Inspect()
		if seenKeys[keyStr] {
			return &object.Error{Message: fmt.Sprintf("duplicate key in hash literal: %s", keyStr)}
		}
		seenKeys[keyStr] = true

		// Check key type consistency
		if !keyTypeSet {
			keyType = actualKey.Type()
			keyTypeSet = true
		} else if actualKey.Type() != keyType {
			return &object.Error{Message: fmt.Sprintf("hash key type mismatch: expected %s, got %s", keyType, actualKey.Type())}
		}

		value := Eval(pair.Value, env)
		if isError(value) {
			return value
		}

		// Unwrap Some objects to get the actual value
		var actualValue object.Object
		if some, ok := value.(*object.Some); ok {
			actualValue = some.Value
		} else {
			actualValue = value
		}

		// Check value type consistency
		if !valueTypeSet {
			valueType = actualValue.Type()
			valueTypeSet = true
		} else if actualValue.Type() != valueType {
			return &object.Error{Message: fmt.Sprintf("hash value type mismatch: expected %s, got %s", valueType, actualValue.Type())}
		}

		hashed := hashKey.HashKey()
		pairs[hashed] = object.HashPair{Key: actualKey, Value: actualValue}
	}

	// Handle empty hash maps - set default types
	if !keyTypeSet {
		keyType = object.STRING_OBJ // Default key type
	}
	if !valueTypeSet {
		valueType = object.STRING_OBJ // Default value type
	}

	return &object.Some{Value: &object.Hash{
		Pairs:     pairs,
		KeyType:   keyType,
		ValueType: valueType,
	}}
}

func evalIndexExpression(left, index object.Object) object.Object {
	switch {
	case left.Type() == object.SOME_OBJ:
		leftSome := left.(*object.Some)

		// Handle the index - it might be Some-wrapped or not
		var actualIndex object.Object
		if indexSome, ok := index.(*object.Some); ok {
			actualIndex = indexSome.Value
		} else {
			actualIndex = index
		}

		if leftSome.Value.Type() == object.ARRAY_OBJ && actualIndex.Type() == object.INTEGER_OBJ {
			return evalArrayIndexExpression(leftSome.Value, actualIndex)
		}

		if leftSome.Value.Type() == object.HASH_OBJ {
			return evalHashIndexExpression(leftSome.Value, actualIndex)
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

func evalHashIndexExpression(hash, key object.Object) object.Object {
	hashObject := hash.(*object.Hash)

	keyHash, ok := key.(object.Hashable)
	if !ok {
		return &object.Error{Message: fmt.Sprintf("unusable as hash key: %s", key.Type())}
	}

	// Check key type consistency
	if key.Type() != hashObject.KeyType {
		return &object.Error{Message: fmt.Sprintf("hash key type mismatch: expected %s, got %s", hashObject.KeyType, key.Type())}
	}

	pair, ok := hashObject.Pairs[keyHash.HashKey()]
	if !ok {
		return NONE
	}

	return &object.Some{Value: pair.Value}
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

// evalStructLiteral evaluates a struct definition and returns a StructDefinition object
func evalStructLiteral(node *ast.StructLiteral, env *object.Environment) object.Object {
	fields := make(map[string]string)

	for _, field := range node.Fields {
		fieldName := field.Name.Value
		fieldType := field.Type.Value

		// Validate that it's a supported type
		if fieldType != "int64" {
			return &object.Error{Message: fmt.Sprintf("unsupported type: %s", fieldType)}
		}

		fields[fieldName] = fieldType
	}

	return &object.StructDefinition{Fields: fields}
}

// evalStructInstanceExpression evaluates struct instantiation and returns a StructInstance object
func evalStructInstanceExpression(node *ast.StructInstanceExpression, env *object.Environment) object.Object {
	// Evaluate the struct definition expression
	structDef := Eval(node.StructExpr, env)
	if isError(structDef) {
		return structDef
	}

	// Ensure it's a struct definition
	definition, ok := structDef.(*object.StructDefinition)
	if !ok {
		return &object.Error{Message: fmt.Sprintf("not a struct definition: %T", structDef)}
	}

	// Evaluate field values
	fieldValues := make(map[string]object.Object)

	for fieldName, fieldExpr := range node.Fields {
		// Check if field exists in the definition
		expectedType, exists := definition.Fields[fieldName]
		if !exists {
			return &object.Error{Message: fmt.Sprintf("field '%s' not found in struct definition", fieldName)}
		}

		// Evaluate the field value
		fieldValue := Eval(fieldExpr, env)
		if isError(fieldValue) {
			return fieldValue
		}

		// Type checking - for now, we only support int64
		if expectedType == "int64" {
			// Check if the value is a Some-wrapped Integer
			if some, ok := fieldValue.(*object.Some); ok {
				if _, ok := some.Value.(*object.Integer); ok {
					fieldValues[fieldName] = some.Value
				} else {
					return &object.Error{Message: fmt.Sprintf("field '%s' expected int64, got %T", fieldName, some.Value)}
				}
			} else {
				return &object.Error{Message: fmt.Sprintf("field '%s' expected int64, got %T", fieldName, fieldValue)}
			}
		}
	}

	// Check that all required fields are provided
	for fieldName := range definition.Fields {
		if _, provided := fieldValues[fieldName]; !provided {
			return &object.Error{Message: fmt.Sprintf("missing field '%s' in struct instantiation", fieldName)}
		}
	}

	return &object.StructInstance{
		Definition: definition,
		Fields:     fieldValues,
	}
}

func evalErrorPropagationExpression(node *ast.ErrorPropagationExpression, env *object.Environment) object.Object {
	// First, evaluate the wrapped expression
	result := Eval(node.Expression, env)
	if isError(result) {
		return result
	}

	// Check if the result is a Result object
	resultObj, ok := result.(*object.Result)
	if !ok {
		return &object.Error{Message: fmt.Sprintf("? operator can only be applied to Result objects, got %T", result)}
	}

	// If there's an error, return it (this propagates the error up)
	if resultObj.Err != nil {
		return resultObj.Err
	}

	// If there's no error, return the value
	return resultObj.Value
}
