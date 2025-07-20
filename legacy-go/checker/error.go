package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

// TypeError represents a type checking error
type TypeError struct {
	Message string
	Token   token.Token
}

func (e *TypeError) Error() string {
	return e.Message
}

// Checker methods for error handling
func (c *Checker) addError(message string, token token.Token) {
	c.errors = append(c.errors, &TypeError{Message: message, Token: token})
}

func (c *Checker) addErrorWithLocation(message string, node ast.Node) {
	var location string
	if node != nil {
		// Get token information from the node
		switch n := node.(type) {
		case *ast.Identifier:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.LetStatement:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.CallExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.InfixExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.PrefixExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IntegerLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.FloatLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StringLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.Boolean:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ArrayLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IndexExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.MemberAccessExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.IfExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ForExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.FunctionLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StructLiteral:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.StructInstanceExpression:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		case *ast.ExternStatement:
			location = fmt.Sprintf("line %d, column %d: ", n.Token.Line, n.Token.Column)
		}
	}
	c.errors = append(c.errors, &TypeError{Message: location + message})
}
