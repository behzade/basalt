package ast

import "github.com/behzade/basalt/token"

// All nodes in our AST must implement the Node interface.
type Node interface {
	TokenLiteral() string // Used for debugging and testing
}

// A Statement is a Node that doesn't produce a value.
type Statement interface {
	Node
	statementNode()
}

// An Expression is a Node that produces a value.
type Expression interface {
	Node
	expressionNode()
}

// Program is the root node of every AST our parser produces.
type Program struct {
	Statements []Statement
}

func (p *Program) TokenLiteral() string {
	if len(p.Statements) > 0 {
		return p.Statements[0].TokenLiteral()
	}
	return ""
}

// LetStatement represents a 'let' statement, e.g., `let x = 5;`
type LetStatement struct {
	Token token.Token // the token.LET token
	Name  *Identifier
	Value Expression
}

func (ls *LetStatement) statementNode()       {}
func (ls *LetStatement) TokenLiteral() string { return ls.Token.Literal }

// ReturnStatement represents a 'return' statement, e.g., `return 5;`
type ReturnStatement struct {
	Token       token.Token // the 'return' token
	ReturnValue Expression
}

func (rs *ReturnStatement) statementNode()       {}
func (rs *ReturnStatement) TokenLiteral() string { return rs.Token.Literal }

// Identifier represents a variable name, e.g., `x` in `let x = 5;`
type Identifier struct {
	Token token.Token // the token.IDENT token
	Value string
}

func (i *Identifier) expressionNode()      {}
func (i *Identifier) TokenLiteral() string { return i.Token.Literal }
