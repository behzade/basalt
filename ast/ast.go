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

// TypeAnnotation represents a type annotation like int64, string, [int64], etc.
type TypeAnnotation struct {
	Token token.Token // The type token
	Value string      // The type name (e.g., "int64", "string", "bool")
}

func (ta *TypeAnnotation) expressionNode()      {}
func (ta *TypeAnnotation) TokenLiteral() string { return ta.Token.Literal }
func (ta *TypeAnnotation) String() string       { return ta.Value }

// Parameter represents a function parameter with optional type annotation
type Parameter struct {
	Name *Identifier     // Parameter name
	Type *TypeAnnotation // Optional type annotation
}

func (p *Parameter) String() string {
	if p.Type != nil {
		return p.Name.String() + ": " + p.Type.String()
	}
	return p.Name.String()
}

// LetStatement represents `let <name> = <value>;` or `let mut <name> = <value>;` or `let <name>: <type> = <value>;`
type LetStatement struct {
	Token   token.Token // the token.LET token
	Name    *Identifier
	Type    *TypeAnnotation // Optional type annotation
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
	if ls.Type != nil {
		out.WriteString(": ")
		out.WriteString(ls.Type.String())
	}
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
	// Add this field to track if a semicolon was present, distinguishing
	// a statement (true) from a value-producing expression (false).
	HasSemicolon bool
}

func (es *ExpressionStatement) statementNode()       {}
func (es *ExpressionStatement) TokenLiteral() string { return es.Token.Literal }
func (es *ExpressionStatement) String() string {
	if es.Expression != nil {
		// If it had a semicolon, append it for accurate AST representation.
		if es.HasSemicolon {
			return es.Expression.String() + ";"
		}
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

// FunctionLiteral represents a function definition with optional type annotations.
type FunctionLiteral struct {
	Token      token.Token     // The 'fn' token
	Parameters []*Parameter    // Parameters with type annotations
	ReturnType *TypeAnnotation // Optional return type annotation
	Body       *BlockStatement
	IsVariadic bool // New flag for variadic functions
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
	if fl.IsVariadic {
		if len(params) > 0 {
			out.WriteString(", ")
		}
		out.WriteString("...")
	}
	out.WriteString(")")
	if fl.ReturnType != nil {
		out.WriteString(": ")
		out.WriteString(fl.ReturnType.String())
	}
	out.WriteString(" ")
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

	// Sort field names to ensure deterministic output
	fieldNames := make([]string, 0, len(sie.Fields))
	for name := range sie.Fields {
		fieldNames = append(fieldNames, name)
	}

	// Sort the field names alphabetically
	for i := 0; i < len(fieldNames); i++ {
		for j := i + 1; j < len(fieldNames); j++ {
			if fieldNames[i] > fieldNames[j] {
				fieldNames[i], fieldNames[j] = fieldNames[j], fieldNames[i]
			}
		}
	}

	fields := make([]string, 0, len(fieldNames))
	for _, name := range fieldNames {
		fields = append(fields, name+": "+sie.Fields[name].String())
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

type ExternStatement struct {
	Token      token.Token // The 'extern' token
	Function   *Identifier
	Parameters []*Parameter
	ReturnType *TypeAnnotation
	IsVariadic bool // New flag for variadic functions
}

func (es *ExternStatement) statementNode()       {}
func (es *ExternStatement) TokenLiteral() string { return es.Token.Literal }
func (es *ExternStatement) String() string {
	var out bytes.Buffer
	params := []string{}
	for _, p := range es.Parameters {
		params = append(params, p.String())
	}

	out.WriteString(es.TokenLiteral() + " fn " + es.Function.String())
	out.WriteString("(")
	out.WriteString(strings.Join(params, ", "))
	if es.IsVariadic {
		if len(params) > 0 {
			out.WriteString(", ")
		}
		out.WriteString("...")
	}
	out.WriteString(")")

	if es.ReturnType != nil {
		out.WriteString(" -> " + es.ReturnType.String())
	}

	out.WriteString(";")
	return out.String()
}

// EnumVariant represents a single variant in an enum definition
type EnumVariant struct {
	Name    *Identifier     // Variant name
	Payload *TypeAnnotation // Optional payload type (can be nil)
}

// EnumStatement represents an enum definition: enum Option { Some(int64), None }
type EnumStatement struct {
	Token    token.Token    // The 'enum' token
	Name     *Identifier    // Enum name
	Variants []*EnumVariant // The enum variants
}

func (es *EnumStatement) statementNode()       {}
func (es *EnumStatement) TokenLiteral() string { return es.Token.Literal }
func (es *EnumStatement) String() string {
	var out bytes.Buffer
	variants := []string{}
	for _, v := range es.Variants {
		if v.Payload != nil {
			variants = append(variants, v.Name.String()+"("+v.Payload.String()+")")
		} else {
			variants = append(variants, v.Name.String())
		}
	}
	out.WriteString("enum ")
	out.WriteString(es.Name.String())
	out.WriteString(" { ")
	out.WriteString(strings.Join(variants, ", "))
	out.WriteString(" }")
	return out.String()
}

// EnumLiteral represents an enum literal expression: enum { Some(int64), None }
type EnumLiteral struct {
	Token    token.Token    // The 'enum' token
	Variants []*EnumVariant // The enum variants
}

func (el *EnumLiteral) expressionNode()      {}
func (el *EnumLiteral) TokenLiteral() string { return el.Token.Literal }
func (el *EnumLiteral) String() string {
	var out bytes.Buffer
	variants := []string{}
	for _, v := range el.Variants {
		if v.Payload != nil {
			variants = append(variants, v.Name.String()+"("+v.Payload.String()+")")
		} else {
			variants = append(variants, v.Name.String())
		}
	}
	out.WriteString("enum { ")
	out.WriteString(strings.Join(variants, ", "))
	out.WriteString(" }")
	return out.String()
}

// EnumInstantiationExpression represents enum instantiation: Option::Some(42)
type EnumInstantiationExpression struct {
	Token     token.Token     // The first token of the enum path
	Enum      *PathExpression // The enum path (e.g., Option)
	Variant   *Identifier     // The variant name (e.g., Some)
	Arguments []Expression    // Arguments for the variant (empty for variants without payload)
}

func (eie *EnumInstantiationExpression) expressionNode()      {}
func (eie *EnumInstantiationExpression) TokenLiteral() string { return eie.Token.Literal }
func (eie *EnumInstantiationExpression) String() string {
	var out bytes.Buffer
	out.WriteString(eie.Enum.String())
	out.WriteString("::")
	out.WriteString(eie.Variant.String())
	if len(eie.Arguments) > 0 {
		args := []string{}
		for _, arg := range eie.Arguments {
			args = append(args, arg.String())
		}
		out.WriteString("(")
		out.WriteString(strings.Join(args, ", "))
		out.WriteString(")")
	}
	return out.String()
}

// MatchArm represents a single arm in a match expression
type MatchArm struct {
	Pattern     *EnumInstantiationExpression // The pattern to match
	Consequence Expression                   // The expression to execute if pattern matches
}

// MatchExpression represents a match expression: match value { Pattern => expr, ... }
type MatchExpression struct {
	Token     token.Token // The 'match' token
	Condition Expression  // The value being matched
	Arms      []*MatchArm // The match arms
}

func (me *MatchExpression) expressionNode()      {}
func (me *MatchExpression) TokenLiteral() string { return me.Token.Literal }
func (me *MatchExpression) String() string {
	var out bytes.Buffer
	arms := []string{}
	for _, arm := range me.Arms {
		arms = append(arms, arm.Pattern.String()+" => "+arm.Consequence.String())
	}
	out.WriteString("match ")
	out.WriteString(me.Condition.String())
	out.WriteString(" { ")
	out.WriteString(strings.Join(arms, ", "))
	out.WriteString(" }")
	return out.String()
}
