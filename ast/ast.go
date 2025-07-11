package ast

import (
	"bytes"
	"strings"

	"github.com/behzade/basalt/token"
)

// Node is the base interface for all AST nodes.
type Node interface {
	TokenLiteral() string
	String() string // For debugging and printing the AST
}

// Statement is a node that doesn't produce a value.
type Statement interface {
	Node
	statementNode()
}

// Expression is a node that produces a value.
type Expression interface {
	Node
	expressionNode()
}

// Program is the root node of every AST.
type Program struct {
	Statements []Statement
}

func (p *Program) TokenLiteral() string {
	if len(p.Statements) > 0 {
		return p.Statements[0].TokenLiteral()
	}
	return ""
}

func (p *Program) String() string {
	var out bytes.Buffer
	for _, s := range p.Statements {
		out.WriteString(s.String())
	}
	return out.String()
}

// LetStatement represents `let <name> = <value>;` or `let mut <name> = <value>;`
type LetStatement struct {
	Token   token.Token // the token.LET token
	Name    *Identifier
	Value   Expression
	Mutable bool // Add this field
}

func (ls *LetStatement) statementNode()       {}
func (ls *LetStatement) TokenLiteral() string { return ls.Token.Literal }
func (ls *LetStatement) String() string {
	var out bytes.Buffer
	out.WriteString(ls.TokenLiteral() + " ")
	if ls.Mutable {
		out.WriteString("mut ")
	}
	out.WriteString(ls.Name.String())
	out.WriteString(" = ")
	if ls.Value != nil {
		out.WriteString(ls.Value.String())
	}
	out.WriteString(";")
	return out.String()
}

// ReturnStatement represents `return <value>;`
type ReturnStatement struct {
	Token       token.Token // the 'return' token
	ReturnValue Expression
}

func (rs *ReturnStatement) statementNode()       {}
func (rs *ReturnStatement) TokenLiteral() string { return rs.Token.Literal }
func (rs *ReturnStatement) String() string {
	var out bytes.Buffer
	out.WriteString(rs.TokenLiteral() + " ")
	if rs.ReturnValue != nil {
		out.WriteString(rs.ReturnValue.String())
	}
	out.WriteString(";")
	return out.String()
}

// ExpressionStatement represents a statement containing a single expression.
type ExpressionStatement struct {
	Token      token.Token // the first token of the expression
	Expression Expression
}

func (es *ExpressionStatement) statementNode()       {}
func (es *ExpressionStatement) TokenLiteral() string { return es.Token.Literal }
func (es *ExpressionStatement) String() string {
	if es.Expression != nil {
		return es.Expression.String()
	}
	return ""
}

// Identifier represents a variable name.
type Identifier struct {
	Token token.Token // the token.IDENT token
	Value string
}

func (i *Identifier) expressionNode()      {}
func (i *Identifier) TokenLiteral() string { return i.Token.Literal }
func (i *Identifier) String() string       { return i.Value }

// Boolean represents a boolean literal.
type Boolean struct {
	Token token.Token
	Value bool
}

func (b *Boolean) expressionNode()      {}
func (b *Boolean) TokenLiteral() string { return b.Token.Literal }
func (b *Boolean) String() string       { return b.Token.Literal }

// IntegerLiteral represents an integer value.
type IntegerLiteral struct {
	Token token.Token // the token.INT token
	Value int64
}

func (il *IntegerLiteral) expressionNode()      {}
func (il *IntegerLiteral) TokenLiteral() string { return il.Token.Literal }
func (il *IntegerLiteral) String() string       { return il.Token.Literal }

// FloatLiteral represents a floating-point value.
type FloatLiteral struct {
	Token token.Token // the token.FLOAT token
	Value float64
}

func (fl *FloatLiteral) expressionNode()      {}
func (fl *FloatLiteral) TokenLiteral() string { return fl.Token.Literal }
func (fl *FloatLiteral) String() string       { return fl.Token.Literal }

// StringLiteral represents a string value.
type StringLiteral struct {
	Token token.Token // the token.STRING token
	Value string
}

func (sl *StringLiteral) expressionNode()      {}
func (sl *StringLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *StringLiteral) String() string       { return "\"" + sl.Value + "\"" }

// ArrayLiteral represents an array literal [1, 2, 3].
type ArrayLiteral struct {
	Token    token.Token // The '[' token
	Elements []Expression
}

func (al *ArrayLiteral) expressionNode()      {}
func (al *ArrayLiteral) TokenLiteral() string { return al.Token.Literal }
func (al *ArrayLiteral) String() string {
	var out bytes.Buffer
	elements := []string{}
	for _, e := range al.Elements {
		elements = append(elements, e.String())
	}
	out.WriteString("[")
	out.WriteString(strings.Join(elements, ", "))
	out.WriteString("]")
	return out.String()
}

// IndexExpression represents array indexing like arr[0] or slicing like arr[1:3].
type IndexExpression struct {
	Token   token.Token // The '[' token
	Left    Expression  // The array being indexed
	Start   Expression  // The start index expression (renamed from Index)
	End     Expression  // The end index expression (nil for simple indexing)
	IsSlice bool        // True if this is a slicing operation (has colon)
}

func (ie *IndexExpression) expressionNode()      {}
func (ie *IndexExpression) TokenLiteral() string { return ie.Token.Literal }
func (ie *IndexExpression) String() string {
	var out bytes.Buffer
	out.WriteString("(")
	out.WriteString(ie.Left.String())
	out.WriteString("[")
	if ie.Start != nil {
		out.WriteString(ie.Start.String())
	}
	if ie.IsSlice {
		out.WriteString(":")
		if ie.End != nil {
			out.WriteString(ie.End.String())
		}
	}
	out.WriteString("])")
	return out.String()
}

// PrefixExpression represents an expression with a prefix operator.
type PrefixExpression struct {
	Token    token.Token // The prefix token, e.g. !
	Operator string
	Right    Expression
}

func (pe *PrefixExpression) expressionNode()      {}
func (pe *PrefixExpression) TokenLiteral() string { return pe.Token.Literal }
func (pe *PrefixExpression) String() string {
	var out bytes.Buffer
	out.WriteString("(")
	out.WriteString(pe.Operator)
	out.WriteString(pe.Right.String())
	out.WriteString(")")
	return out.String()
}

// InfixExpression represents an expression with an infix operator.
type InfixExpression struct {
	Token    token.Token // The operator token, e.g. +
	Left     Expression
	Operator string
	Right    Expression
}

func (ie *InfixExpression) expressionNode()      {}
func (ie *InfixExpression) TokenLiteral() string { return ie.Token.Literal }
func (ie *InfixExpression) String() string {
	var out bytes.Buffer
	out.WriteString("(")
	out.WriteString(ie.Left.String())
	out.WriteString(" " + ie.Operator + " ")
	out.WriteString(ie.Right.String())
	out.WriteString(")")
	return out.String()
}

// FunctionLiteral represents a function definition.
type FunctionLiteral struct {
	Token      token.Token // The 'fn' token
	Parameters []*Identifier
	Body       *BlockStatement
}

func (fl *FunctionLiteral) expressionNode()      {}
func (fl *FunctionLiteral) TokenLiteral() string { return fl.Token.Literal }
func (fl *FunctionLiteral) String() string {
	var out bytes.Buffer
	params := []string{}
	for _, p := range fl.Parameters {
		params = append(params, p.String())
	}
	out.WriteString(fl.TokenLiteral())
	out.WriteString("(")
	out.WriteString(strings.Join(params, ", "))
	out.WriteString(") ")
	out.WriteString(fl.Body.String())
	return out.String()
}

// CallExpression represents a function call.
type CallExpression struct {
	Token     token.Token // The '(' token
	Function  Expression  // Identifier or FunctionLiteral
	Arguments []Expression
}

func (ce *CallExpression) expressionNode()      {}
func (ce *CallExpression) TokenLiteral() string { return ce.Token.Literal }
func (ce *CallExpression) String() string {
	var out bytes.Buffer
	args := []string{}
	for _, a := range ce.Arguments {
		args = append(args, a.String())
	}
	out.WriteString(ce.Function.String())
	out.WriteString("(")
	out.WriteString(strings.Join(args, ", "))
	out.WriteString(")")
	return out.String()
}

// IfExpression represents an if-else conditional expression.
type IfExpression struct {
	Token       token.Token     // The 'if' token
	Condition   Expression      // The condition to evaluate
	Consequence *BlockStatement // The block to execute if condition is true
	Alternative *BlockStatement // The block to execute if condition is false (can be nil)
}

func (ie *IfExpression) expressionNode()      {}
func (ie *IfExpression) TokenLiteral() string { return ie.Token.Literal }
func (ie *IfExpression) String() string {
	var out bytes.Buffer
	out.WriteString("if")
	out.WriteString(ie.Condition.String())
	out.WriteString(" ")
	out.WriteString(ie.Consequence.String())
	if ie.Alternative != nil {
		out.WriteString("else ")
		out.WriteString(ie.Alternative.String())
	}
	return out.String()
}

// ForExpression represents a for loop.
type ForExpression struct {
	Token       token.Token     // The 'for' token
	Condition   Expression      // The condition to evaluate
	Consequence *BlockStatement // The block to execute while condition is true
}

func (fe *ForExpression) expressionNode()      {}
func (fe *ForExpression) TokenLiteral() string { return fe.Token.Literal }
func (fe *ForExpression) String() string {
	var out bytes.Buffer
	out.WriteString("for ")
	out.WriteString(fe.Condition.String())
	out.WriteString(" ")
	out.WriteString(fe.Consequence.String())
	return out.String()
}

// BlockStatement represents a block of statements, e.g., the body of a function or an if-else block.
type BlockStatement struct {
	Token      token.Token // The '{' token
	Statements []Statement
}

func (bs *BlockStatement) statementNode()       {}
func (bs *BlockStatement) TokenLiteral() string { return bs.Token.Literal }
func (bs *BlockStatement) String() string {
	var out bytes.Buffer
	out.WriteString(bs.Token.Literal)
	for _, s := range bs.Statements {
		out.WriteString(s.String())
	}
	out.WriteString("}")
	return out.String()
}

// MemberAccessExpression represents access to a member of an object using the dot operator (object.member)
type MemberAccessExpression struct {
	Token token.Token // The '.' token
	Left  Expression  // The object being accessed (e.g., io)
	Right *Identifier // The member being accessed (e.g., VERSION)
}

func (mae *MemberAccessExpression) expressionNode()      {}
func (mae *MemberAccessExpression) TokenLiteral() string { return mae.Token.Literal }
func (mae *MemberAccessExpression) String() string {
	var out bytes.Buffer
	out.WriteString("(")
	out.WriteString(mae.Left.String())
	out.WriteString(".")
	out.WriteString(mae.Right.String())
	out.WriteString(")")
	return out.String()
}

// PathExpression represents a module path like std::io::fs
type PathExpression struct {
	Token    token.Token   // The first identifier token
	Segments []*Identifier // The path segments (e.g., ["std", "io", "fs"])
}

func (pe *PathExpression) expressionNode()      {}
func (pe *PathExpression) TokenLiteral() string { return pe.Token.Literal }
func (pe *PathExpression) String() string {
	var out bytes.Buffer
	for i, segment := range pe.Segments {
		if i > 0 {
			out.WriteString("::")
		}
		out.WriteString(segment.String())
	}
	return out.String()
}

// ImportStatement represents `import <path>` or `import <path> as <alias>;`
type ImportStatement struct {
	Token token.Token     // The 'import' token
	Path  *PathExpression // The module path (e.g., std::io)
	Alias *Identifier     // Optional alias (can be nil)
}

func (is *ImportStatement) statementNode()       {}
func (is *ImportStatement) TokenLiteral() string { return is.Token.Literal }
func (is *ImportStatement) String() string {
	var out bytes.Buffer
	out.WriteString(is.TokenLiteral() + " ")
	out.WriteString(is.Path.String())
	if is.Alias != nil {
		out.WriteString(" as ")
		out.WriteString(is.Alias.String())
	}
	out.WriteString(";")
	return out.String()
}

// StructField represents a field in a struct definition with name and type
type StructField struct {
	Name *Identifier // Field name
	Type *Identifier // Field type (e.g., int64)
}

// StructLiteral represents an anonymous struct definition: struct { a: int64, b: string }
type StructLiteral struct {
	Token  token.Token    // The 'struct' token
	Fields []*StructField // The field definitions
}

func (sl *StructLiteral) expressionNode()      {}
func (sl *StructLiteral) TokenLiteral() string { return sl.Token.Literal }
func (sl *StructLiteral) String() string {
	var out bytes.Buffer
	fields := []string{}
	for _, f := range sl.Fields {
		fields = append(fields, f.Name.String()+": "+f.Type.String())
	}
	out.WriteString("struct { ")
	out.WriteString(strings.Join(fields, ", "))
	out.WriteString(" }")
	return out.String()
}

// StructInstanceExpression represents struct instantiation: struct_def { a: 42, b: "hello" }
type StructInstanceExpression struct {
	Token      token.Token           // The '{' token
	StructExpr Expression            // The struct definition being instantiated
	Fields     map[string]Expression // Field name -> value expression
}

func (sie *StructInstanceExpression) expressionNode()      {}
func (sie *StructInstanceExpression) TokenLiteral() string { return sie.Token.Literal }
func (sie *StructInstanceExpression) String() string {
	var out bytes.Buffer
	fields := []string{}
	for name, value := range sie.Fields {
		fields = append(fields, name+": "+value.String())
	}
	out.WriteString(sie.StructExpr.String())
	out.WriteString(" { ")
	out.WriteString(strings.Join(fields, ", "))
	out.WriteString(" }")
	return out.String()
}

// ErrorPropagationExpression represents the ? operator for error propagation
type ErrorPropagationExpression struct {
	Token      token.Token // The '?' token
	Expression Expression  // The expression the ? is applied to
}

func (epe *ErrorPropagationExpression) expressionNode()      {}
func (epe *ErrorPropagationExpression) TokenLiteral() string { return epe.Token.Literal }
func (epe *ErrorPropagationExpression) String() string {
	var out bytes.Buffer
	out.WriteString("(")
	out.WriteString(epe.Expression.String())
	out.WriteString("?)")
	return out.String()
}

// HashPair represents a key-value pair in a hash literal
type HashPair struct {
	Key   Expression
	Value Expression
}

// HashLiteral represents a hash map literal like {"key": value, 42: "answer"}
type HashLiteral struct {
	Token token.Token // The '{' token
	Pairs []HashPair  // Key-value pairs in source order
}

func (hl *HashLiteral) expressionNode()      {}
func (hl *HashLiteral) TokenLiteral() string { return hl.Token.Literal }
func (hl *HashLiteral) String() string {
	var out bytes.Buffer
	pairs := []string{}
	for _, pair := range hl.Pairs {
		pairs = append(pairs, pair.Key.String()+": "+pair.Value.String())
	}
	out.WriteString("{")
	out.WriteString(strings.Join(pairs, ", "))
	out.WriteString("}")
	return out.String()
}
