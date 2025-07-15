package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

// Checker performs static type checking
type Checker struct {
	env               *TypeEnvironment
	errors            []*TypeError
	isInUnsafeContext bool // Flag to track if we are inside an unsafe block
}

func New() *Checker {
	checker := &Checker{
		env:    NewTypeEnvironment(),
		errors: []*TypeError{},
	}
	checker.setupBuiltins()
	return checker
}

func (c *Checker) setupBuiltins() {
	// No built-ins needed anymore, as they are provided by the stdlib
}

func (c *Checker) Errors() []*TypeError {
	return c.errors
}

// Check performs type checking on the given AST
func (c *Checker) Check(node ast.Node) Type {
	switch node := node.(type) {
	case *ast.Program:
		return c.checkProgram(node)
	case *ast.ModuleStatement:
		return c.checkModuleStatement(node)
	case *ast.LetStatement:
		return c.checkLetStatement(node)
	case *ast.ReturnStatement:
		return c.checkReturnStatement(node)
	case *ast.ExpressionStatement:
		return c.checkExpressionStatement(node)
	case *ast.BlockStatement:
		return c.checkBlockStatement(node)
	case *ast.Identifier:
		return c.checkIdentifier(node)
	case *ast.IntegerLiteral:
		return c.checkIntegerLiteral(node)
	case *ast.FloatLiteral:
		return c.checkFloatLiteral(node)
	case *ast.StringLiteral:
		return c.checkStringLiteral(node)
	case *ast.Boolean:
		return c.checkBoolean(node)
	case *ast.ArrayLiteral:
		return c.checkArrayLiteral(node)
	case *ast.HashLiteral:
		return c.checkHashLiteral(node)
	case *ast.FunctionLiteral:
		return c.checkFunctionLiteral(node)
	case *ast.CallExpression:
		return c.checkCallExpression(node)
	case *ast.MemberAccessExpression:
		return c.checkMemberAccessExpression(node)
	case *ast.IfExpression:
		return c.checkIfExpression(node)
	case *ast.ForExpression:
		return c.checkForExpression(node)
	case *ast.PrefixExpression:
		return c.checkPrefixExpression(node)
	case *ast.InfixExpression:
		return c.checkInfixExpression(node)
	case *ast.IndexExpression:
		return c.checkIndexExpression(node)
	case *ast.StructLiteral:
		return c.checkStructLiteral(node)
	case *ast.StructInstanceExpression:
		return c.checkStructInstanceExpression(node)
	case *ast.ExternStatement:
		return c.checkExternStatement(node)
	case *ast.EnumLiteral:
		return c.checkEnumLiteral(node)
	case *ast.EnumInstantiationExpression:
		return c.checkEnumInstantiationExpression(node)
	case *ast.MatchExpression:
		return c.checkMatchExpression(node)
	case *ast.UnsafeStatement:
		return c.checkUnsafeStatement(node)
	default:
		c.addError(fmt.Sprintf("unknown node type: %T", node), token.Token{Type: token.ILLEGAL, Literal: "unknown"})
		return &NoneType{}
	}
}
