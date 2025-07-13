package parser

import (
	"fmt"
	"strconv"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/lexer"
	"github.com/behzade/basalt/token"
)

const (
	_ int = iota
	LOWEST
	ASSIGNMENT      // =
	EQUALS          // ==
	LESSGREATER     // > or <
	SUM             // +
	PRODUCT         // *
	PREFIX          // -X or !X
	CALL            // myFunction(X)
	INDEX           // array[index]
	MEMBER_ACCESS   // object.member
	STRUCT_INSTANCE // struct_def { ... }
)

var precedences = map[token.TokenType]int{
	token.ASSIGN:   ASSIGNMENT,
	token.EQ:       EQUALS,
	token.NOT_EQ:   EQUALS,
	token.LT:       LESSGREATER,
	token.GT:       LESSGREATER,
	token.PLUS:     SUM,
	token.MINUS:    SUM,
	token.SLASH:    PRODUCT,
	token.ASTERISK: PRODUCT,
	token.LPAREN:   CALL,
	token.LBRACKET: INDEX,
	token.DOT:      MEMBER_ACCESS,
	token.LBRACE:   STRUCT_INSTANCE,
}

type (
	prefixParseFn  func() ast.Expression
	infixParseFn   func(ast.Expression) ast.Expression
	postfixParseFn func(ast.Expression) ast.Expression
)

type ParserError struct {
	Msg  string
	Line int
	Col  int
}

// Parser holds the lexer and the current/peek tokens.
type Parser struct {
	l      *lexer.Lexer
	errors []ParserError

	curToken  token.Token
	peekToken token.Token

	prefixParseFns  map[token.TokenType]prefixParseFn
	infixParseFns   map[token.TokenType]infixParseFn
	postfixParseFns map[token.TokenType]postfixParseFn
}

// New creates a new Parser.
func New(l *lexer.Lexer) *Parser {
	p := &Parser{
		l:      l,
		errors: []ParserError{},
	}

	p.prefixParseFns = make(map[token.TokenType]prefixParseFn)
	p.registerPrefix(token.IDENT, p.parseIdentifier)
	p.registerPrefix(token.INT, p.parseIntegerLiteral)
	p.registerPrefix(token.FLOAT, p.parseFloatLiteral)
	p.registerPrefix(token.STRING, p.parseStringLiteral)
	p.registerPrefix(token.BANG, p.parsePrefixExpression)
	p.registerPrefix(token.MINUS, p.parsePrefixExpression)
	p.registerPrefix(token.TRUE, p.parseBoolean)
	p.registerPrefix(token.FALSE, p.parseBoolean)
	p.registerPrefix(token.LPAREN, p.parseGroupedExpression)
	p.registerPrefix(token.IF, p.parseIfExpression)
	p.registerPrefix(token.FUNCTION, p.parseFunctionLiteral)
	p.registerPrefix(token.LBRACKET, p.parseArrayLiteral)
	p.registerPrefix(token.STRUCT, p.parseStructLiteral)
	p.registerPrefix(token.LBRACE, p.parseHashLiteral)
	p.registerPrefix(token.FOR, p.parseForExpression)
	p.registerPrefix(token.MATCH, p.parseMatchExpression)
	p.registerPrefix(token.ENUM, p.parseEnumExpression)

	p.infixParseFns = make(map[token.TokenType]infixParseFn)
	p.registerInfix(token.PLUS, p.parseInfixExpression)
	p.registerInfix(token.MINUS, p.parseInfixExpression)
	p.registerInfix(token.SLASH, p.parseInfixExpression)
	p.registerInfix(token.ASTERISK, p.parseInfixExpression)
	p.registerInfix(token.EQ, p.parseInfixExpression)
	p.registerInfix(token.NOT_EQ, p.parseInfixExpression)
	p.registerInfix(token.LT, p.parseInfixExpression)
	p.registerInfix(token.GT, p.parseInfixExpression)
	p.registerInfix(token.LPAREN, p.parseCallExpression)
	p.registerInfix(token.LBRACKET, p.parseIndexExpression)
	p.registerInfix(token.DOT, p.parseMemberAccessExpression)
	p.registerInfix(token.LBRACE, p.parseStructInstanceExpression)
	p.registerInfix(token.ASSIGN, p.parseInfixExpression)

	p.postfixParseFns = make(map[token.TokenType]postfixParseFn)
	p.registerPostfix(token.QUESTION, p.parseErrorPropagationExpression)

	// Read two tokens, so curToken and peekToken are both set.
	p.nextToken()
	p.nextToken()

	return p
}

func (p *Parser) Errors() []ParserError {
	return p.errors
}

func (p *Parser) peekError(t token.TokenType) {
	msg := fmt.Sprintf("expected next token to be %s, got %s instead",
		t, p.peekToken.Type)
	p.errors = append(p.errors, ParserError{msg, p.peekToken.Line, p.peekToken.Column})
}

func (p *Parser) nextToken() {
	p.curToken = p.peekToken
	p.peekToken = p.l.NextToken()
}

// ParseProgram is the entry point for parsing.
func (p *Parser) ParseProgram() *ast.Program {
	program := &ast.Program{}
	program.Statements = []ast.Statement{}

	for !p.curTokenIs(token.EOF) {
		stmt := p.parseStatement()
		if stmt != nil {
			program.Statements = append(program.Statements, stmt)
		}
		p.nextToken()
	}

	return program
}

func (p *Parser) parseStatement() ast.Statement {
	// Consume all attribute tokens before parsing the statement itself.
	attributes := p.parseAttributes()

	var stmt ast.Statement
	switch p.curToken.Type {
	case token.LET:
		stmt = p.parseLetStatement()
	case token.RETURN:
		stmt = p.parseReturnStatement()
	case token.IMPORT:
		stmt = p.parseImportStatement()
	case token.EXTERN:
		stmt = p.parseExternStatement()
	default:
		stmt = p.parseExpressionStatement()
	}

	// Apply attributes to function definitions
	if len(attributes) > 0 {
		if letStmt, ok := stmt.(*ast.LetStatement); ok {
			if fnLit, ok := letStmt.Value.(*ast.FunctionLiteral); ok {
				fnLit.Attributes = attributes
			} else {
				msg := "attributes can only be applied to a function definition"
				p.errors = append(p.errors, ParserError{msg, letStmt.Token.Line, letStmt.Token.Column})
			}
		} else {
			msg := "attributes can only be applied to let statements containing functions"
			p.errors = append(p.errors, ParserError{msg, p.curToken.Line, p.curToken.Column})
		}
	}
	return stmt
}

func (p *Parser) parseLetStatement() *ast.LetStatement {
	stmt := &ast.LetStatement{Token: p.curToken}

	// Check if the next token is MUT
	if p.peekTokenIs(token.MUT) {
		stmt.Mutable = true
		p.nextToken() // consume the MUT token
	}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	stmt.Name = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	// Check for optional type annotation
	if p.peekTokenIs(token.COLON) {
		p.nextToken() // consume the colon
		stmt.Type = p.parseTypeAnnotation()
		if stmt.Type == nil {
			return nil
		}
	}

	if !p.expectPeek(token.ASSIGN) {
		return nil
	}

	p.nextToken()
	stmt.Value = p.parseExpression(LOWEST)

	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}

	return stmt
}

func (p *Parser) parseReturnStatement() *ast.ReturnStatement {
	stmt := &ast.ReturnStatement{Token: p.curToken}

	p.nextToken()

	stmt.ReturnValue = p.parseExpression(LOWEST)

	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}

	return stmt
}

func (p *Parser) parseExpressionStatement() *ast.ExpressionStatement {
	stmt := &ast.ExpressionStatement{Token: p.curToken}
	stmt.Expression = p.parseExpression(LOWEST)

	// Check for an optional semicolon.
	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
		stmt.HasSemicolon = true
	} else {
		stmt.HasSemicolon = false
	}

	return stmt
}

func (p *Parser) noPrefixParseFnError(t token.TokenType) {
	msg := fmt.Sprintf("no prefix parse function for %s found", t)
	p.errors = append(p.errors, ParserError{msg, p.curToken.Line, p.curToken.Column})
}

func (p *Parser) parseExpression(precedence int) ast.Expression {
	prefix := p.prefixParseFns[p.curToken.Type]
	if prefix == nil {
		p.noPrefixParseFnError(p.curToken.Type)
		return nil
	}
	leftExp := prefix()

	for !p.peekTokenIs(token.SEMICOLON) && precedence < p.peekPrecedence() {
		infix := p.infixParseFns[p.peekToken.Type]
		if infix == nil {
			return leftExp
		}

		p.nextToken()

		leftExp = infix(leftExp)
	}

	// Handle postfix operators
	for {
		postfix := p.postfixParseFns[p.peekToken.Type]
		if postfix == nil {
			break
		}

		p.nextToken()

		leftExp = postfix(leftExp)
	}

	return leftExp
}

func (p *Parser) parseIdentifier() ast.Expression {
	// Check if this is an enum instantiation (Enum::Variant)
	if p.peekTokenIs(token.COLONCOLON) {
		return p.parseEnumInstantiation()
	}
	return &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}
}

// parseEnumInstantiation parses enum instantiation expressions like "Option::Some(42)"
func (p *Parser) parseEnumInstantiation() ast.Expression {
	// Parse the enum path (e.g., "Option")
	enumPath := &ast.PathExpression{
		Token:    p.curToken,
		Segments: []*ast.Identifier{{Token: p.curToken, Value: p.curToken.Literal}},
	}

	if !p.expectPeek(token.COLONCOLON) {
		return nil
	}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	// Parse the variant name (e.g., "Some")
	variant := &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	exp := &ast.EnumInstantiationExpression{
		Token:     enumPath.Token,
		Enum:      enumPath,
		Variant:   variant,
		Arguments: []ast.Expression{},
	}

	// Check for optional arguments
	if p.peekTokenIs(token.LPAREN) {
		p.nextToken() // consume '('
		exp.Arguments = p.parseCallArguments()
	}

	return exp
}

func (p *Parser) parseIntegerLiteral() ast.Expression {
	lit := &ast.IntegerLiteral{Token: p.curToken}

	value, err := strconv.ParseInt(p.curToken.Literal, 0, 64)
	if err != nil {
		msg := fmt.Sprintf("could not parse %q as integer", p.curToken.Literal)
		p.errors = append(p.errors, ParserError{msg, p.curToken.Line, p.curToken.Column})
		return nil
	}
	lit.Value = value
	return lit
}

func (p *Parser) parseFloatLiteral() ast.Expression {
	lit := &ast.FloatLiteral{Token: p.curToken}

	value, err := strconv.ParseFloat(p.curToken.Literal, 64)
	if err != nil {
		msg := fmt.Sprintf("could not parse %q as float", p.curToken.Literal)
		p.errors = append(p.errors, ParserError{msg, p.curToken.Line, p.curToken.Column})
		return nil
	}
	lit.Value = value
	return lit
}

func (p *Parser) parseStringLiteral() ast.Expression {
	return &ast.StringLiteral{Token: p.curToken, Value: p.curToken.Literal}
}

func (p *Parser) parseBoolean() ast.Expression {
	return &ast.Boolean{Token: p.curToken, Value: p.curTokenIs(token.TRUE)}
}

func (p *Parser) parseGroupedExpression() ast.Expression {
	p.nextToken()

	exp := p.parseExpression(LOWEST)

	if !p.expectPeek(token.RPAREN) {
		return nil
	}

	return exp
}

func (p *Parser) parseIfExpression() ast.Expression {
	expression := &ast.IfExpression{Token: p.curToken}

	p.nextToken()

	// Temporarily remove LBRACE from infix operators to prevent it from being consumed
	lbraceInfix := p.infixParseFns[token.LBRACE]
	delete(p.infixParseFns, token.LBRACE)

	expression.Condition = p.parseExpression(LOWEST)

	// Restore LBRACE infix operator
	p.infixParseFns[token.LBRACE] = lbraceInfix

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	expression.Consequence = p.parseBlockStatement()

	if p.peekTokenIs(token.ELSE) {
		p.nextToken()

		if !p.expectPeek(token.LBRACE) {
			return nil
		}

		expression.Alternative = p.parseBlockStatement()
	}

	return expression
}

func (p *Parser) parseForExpression() ast.Expression {
	expression := &ast.ForExpression{Token: p.curToken}

	p.nextToken()

	// Temporarily remove LBRACE from infix operators to prevent it from being consumed
	lbraceInfix := p.infixParseFns[token.LBRACE]
	delete(p.infixParseFns, token.LBRACE)

	expression.Condition = p.parseExpression(LOWEST)

	// Restore LBRACE infix operator
	p.infixParseFns[token.LBRACE] = lbraceInfix

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	expression.Consequence = p.parseBlockStatement()

	return expression
}

func (p *Parser) parsePrefixExpression() ast.Expression {
	expression := &ast.PrefixExpression{
		Token:    p.curToken,
		Operator: p.curToken.Literal,
	}

	p.nextToken()

	expression.Right = p.parseExpression(PREFIX)

	return expression
}

func (p *Parser) parseInfixExpression(left ast.Expression) ast.Expression {
	expression := &ast.InfixExpression{
		Token:    p.curToken,
		Operator: p.curToken.Literal,
		Left:     left,
	}

	precedence := p.curPrecedence()
	p.nextToken()
	expression.Right = p.parseExpression(precedence)

	return expression
}

func (p *Parser) parseFunctionLiteral() ast.Expression {
	lit := &ast.FunctionLiteral{Token: p.curToken}

	if !p.expectPeek(token.LPAREN) {
		return nil
	}

	lit.Parameters, lit.IsVariadic = p.parseFunctionParameters()

	// Check for return type annotation with ->
	if p.peekTokenIs(token.ARROW) {
		p.nextToken() // consume the ->
		lit.ReturnType = p.parseTypeAnnotation()
		if lit.ReturnType == nil {
			return nil
		}
	}

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	lit.Body = p.parseBlockStatement()

	return lit
}

func (p *Parser) parseFunctionParameters() ([]*ast.Parameter, bool) {
	var parameters []*ast.Parameter
	isVariadic := false

	if p.peekTokenIs(token.RPAREN) {
		p.nextToken()
		return parameters, isVariadic
	}

	p.nextToken()

	// Handle first parameter or ellipsis
	if p.curTokenIs(token.ELLIPSIS) {
		isVariadic = true
		if !p.expectPeek(token.RPAREN) {
			return nil, false
		}
		return parameters, isVariadic
	}

	// It's a regular parameter
	param := &ast.Parameter{
		Name: &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal},
	}
	if p.peekTokenIs(token.COLON) {
		p.nextToken()
		param.Type = p.parseTypeAnnotation()
		if param.Type == nil {
			return nil, false
		}
	}
	parameters = append(parameters, param)

	// Handle subsequent parameters
	for p.peekTokenIs(token.COMMA) {
		p.nextToken()
		p.nextToken()

		if p.curTokenIs(token.ELLIPSIS) {
			isVariadic = true
			break
		}

		param := &ast.Parameter{
			Name: &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal},
		}
		if p.peekTokenIs(token.COLON) {
			p.nextToken()
			param.Type = p.parseTypeAnnotation()
			if param.Type == nil {
				return nil, false
			}
		}
		parameters = append(parameters, param)
	}

	if !p.expectPeek(token.RPAREN) {
		return nil, false
	}

	return parameters, isVariadic
}

func (p *Parser) parseBlockStatement() *ast.BlockStatement {
	block := &ast.BlockStatement{Token: p.curToken}
	block.Statements = []ast.Statement{}

	p.nextToken()

	for !p.curTokenIs(token.RBRACE) && !p.curTokenIs(token.EOF) {
		stmt := p.parseStatement()
		if stmt != nil {
			block.Statements = append(block.Statements, stmt)
		}
		p.nextToken()
	}

	return block
}

func (p *Parser) parseCallExpression(function ast.Expression) ast.Expression {
	exp := &ast.CallExpression{Token: p.curToken, Function: function}
	exp.Arguments = p.parseCallArguments()
	return exp
}

func (p *Parser) parseCallArguments() []ast.Expression {
	args := []ast.Expression{}

	if p.peekTokenIs(token.RPAREN) {
		p.nextToken()
		return args
	}

	p.nextToken()
	args = append(args, p.parseExpression(LOWEST))

	for p.peekTokenIs(token.COMMA) {
		p.nextToken()
		p.nextToken()
		args = append(args, p.parseExpression(LOWEST))
	}

	if !p.expectPeek(token.RPAREN) {
		return nil
	}

	return args
}

func (p *Parser) parseMemberAccessExpression(left ast.Expression) ast.Expression {
	exp := &ast.MemberAccessExpression{
		Token: p.curToken, // The '.' token
		Left:  left,
	}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	exp.Right = &ast.Identifier{
		Token: p.curToken,
		Value: p.curToken.Literal,
	}

	return exp
}

func (p *Parser) parseArrayLiteral() ast.Expression {
	array := &ast.ArrayLiteral{Token: p.curToken}
	array.Elements = p.parseExpressionList(token.RBRACKET)
	return array
}

// parseHashLiteral parses hash map literals like {"key": value, 42: "answer"}
func (p *Parser) parseHashLiteral() ast.Expression {
	hash := &ast.HashLiteral{Token: p.curToken}
	hash.Pairs = []ast.HashPair{}

	if p.peekTokenIs(token.RBRACE) {
		p.nextToken()
		return hash
	}

	p.nextToken()

	// Parse first key-value pair
	key := p.parseExpression(LOWEST)
	if !p.expectPeek(token.COLON) {
		return nil
	}

	p.nextToken()
	value := p.parseExpression(LOWEST)
	hash.Pairs = append(hash.Pairs, ast.HashPair{Key: key, Value: value})

	// Parse additional key-value pairs
	for p.peekTokenIs(token.COMMA) {
		p.nextToken()
		p.nextToken()

		key := p.parseExpression(LOWEST)
		if !p.expectPeek(token.COLON) {
			return nil
		}

		p.nextToken()
		value := p.parseExpression(LOWEST)
		hash.Pairs = append(hash.Pairs, ast.HashPair{Key: key, Value: value})
	}

	if !p.expectPeek(token.RBRACE) {
		return nil
	}

	return hash
}

func (p *Parser) parseIndexExpression(left ast.Expression) ast.Expression {
	exp := &ast.IndexExpression{Token: p.curToken, Left: left}

	p.nextToken()

	// Handle cases like [:end] where start is omitted
	if p.curTokenIs(token.COLON) {
		// This is a slicing operation
		exp.IsSlice = true
		// Start is nil for cases like [:end]
		exp.Start = nil
		// Move to the end expression
		p.nextToken()

		// Handle cases like [:] where both start and end are omitted
		if p.curTokenIs(token.RBRACKET) {
			exp.End = nil
		} else {
			// Parse the end expression
			exp.End = p.parseExpression(LOWEST)
		}
	} else {
		// Parse the start expression
		exp.Start = p.parseExpression(LOWEST)

		// Check if this is a slicing operation (has a colon)
		if p.peekTokenIs(token.COLON) {
			exp.IsSlice = true
			p.nextToken() // consume the colon
			p.nextToken() // move to the end expression or ]

			// Handle cases like [start:] where end is omitted
			if p.curTokenIs(token.RBRACKET) {
				// End is nil for cases like [start:]
				exp.End = nil
			} else {
				// Parse the end expression
				exp.End = p.parseExpression(LOWEST)
			}
		}
	}

	// Only expect the closing bracket if we're not already at it
	if !p.curTokenIs(token.RBRACKET) {
		if !p.expectPeek(token.RBRACKET) {
			return nil
		}
	}

	return exp
}

func (p *Parser) parseExpressionList(end token.TokenType) []ast.Expression {
	args := []ast.Expression{}

	if p.peekTokenIs(end) {
		p.nextToken()
		return args
	}

	p.nextToken()
	args = append(args, p.parseExpression(LOWEST))

	for p.peekTokenIs(token.COMMA) {
		p.nextToken()
		p.nextToken()
		args = append(args, p.parseExpression(LOWEST))
	}

	if !p.expectPeek(end) {
		return nil
	}

	return args
}

func (p *Parser) curTokenIs(t token.TokenType) bool {
	return p.curToken.Type == t
}

func (p *Parser) peekTokenIs(t token.TokenType) bool {
	return p.peekToken.Type == t
}

func (p *Parser) expectPeek(t token.TokenType) bool {
	if p.peekTokenIs(t) {
		p.nextToken()
		return true
	}
	p.peekError(t)
	return false
}

func (p *Parser) registerPrefix(tokenType token.TokenType, fn prefixParseFn) {
	p.prefixParseFns[tokenType] = fn
}

func (p *Parser) registerInfix(tokenType token.TokenType, fn infixParseFn) {
	p.infixParseFns[tokenType] = fn
}

func (p *Parser) registerPostfix(tokenType token.TokenType, fn postfixParseFn) {
	p.postfixParseFns[tokenType] = fn
}

func (p *Parser) peekPrecedence() int {
	if p, ok := precedences[p.peekToken.Type]; ok {
		return p
	}
	return LOWEST
}

func (p *Parser) curPrecedence() int {
	if p, ok := precedences[p.curToken.Type]; ok {
		return p
	}
	return LOWEST
}

// parseAttributes parses attributes like #[nogc]. It consumes the tokens
// and returns a slice of attribute names.
func (p *Parser) parseAttributes() []string {
	var attributes []string

	// Loop to handle multiple attributes, e.g., #[inline] #[nogc]
	for p.curTokenIs(token.HASH) {
		if !p.peekTokenIs(token.LBRACKET) {
			return attributes // Not a valid attribute, stop parsing.
		}
		p.nextToken() // consume '#'
		p.nextToken() // consume '['

		if p.curTokenIs(token.IDENT) {
			attrName := p.curToken.Literal
			if attrName == "nogc" {
				attributes = append(attributes, attrName)
			} else {
				msg := "unsupported attribute: " + attrName
				p.errors = append(p.errors, ParserError{Msg: msg, Line: p.curToken.Line, Col: p.curToken.Column})
			}
		} else {
			msg := "expected attribute name"
			p.errors = append(p.errors, ParserError{Msg: msg, Line: p.curToken.Line, Col: p.curToken.Column})
		}

		if !p.expectPeek(token.RBRACKET) {
			return attributes // Malformed attribute.
		}

		// Position parser for the next token (either another '#' or the actual statement).
		if p.peekTokenIs(token.HASH) {
			p.nextToken()
		} else {
			p.nextToken() // Move to the next token after the attribute
			break
		}
	}
	return attributes
}

// parseImportStatement parses import statements like "import std::io;" or "import std::fs as filesystem;"
func (p *Parser) parseImportStatement() *ast.ImportStatement {
	stmt := &ast.ImportStatement{Token: p.curToken}

	// Move to the first identifier of the path
	if !p.expectPeek(token.IDENT) {
		return nil
	}

	// Parse the module path (e.g., std::io::fs)
	path := &ast.PathExpression{
		Token:    p.curToken,
		Segments: []*ast.Identifier{},
	}

	// Add the first identifier
	path.Segments = append(path.Segments, &ast.Identifier{
		Token: p.curToken,
		Value: p.curToken.Literal,
	})

	// Parse additional path segments separated by ::
	for p.peekTokenIs(token.COLONCOLON) {
		p.nextToken() // consume ::
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		path.Segments = append(path.Segments, &ast.Identifier{
			Token: p.curToken,
			Value: p.curToken.Literal,
		})
	}

	stmt.Path = path

	// Check for optional alias
	if p.peekTokenIs(token.AS) {
		p.nextToken() // consume 'as'
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		stmt.Alias = &ast.Identifier{
			Token: p.curToken,
			Value: p.curToken.Literal,
		}
	}

	// Expect semicolon
	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}

	return stmt
}

// In parser/parser.go

func (p *Parser) parseExternStatement() *ast.ExternStatement {
	stmt := &ast.ExternStatement{Token: p.curToken} // Current token is 'extern'

	if !p.expectPeek(token.FUNCTION) {
		return nil
	} // Consumes 'fn'
	if !p.expectPeek(token.IDENT) {
		return nil
	} // Consumes the function name
	stmt.Function = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	if !p.expectPeek(token.LPAREN) {
		return nil
	} // Consumes '('
	stmt.Parameters, stmt.IsVariadic = p.parseFunctionParameters() // On return, current token is ')'

	if !p.peekTokenIs(token.ARROW) { // extern fn puts() -> none; might not have an arrow
		if p.peekTokenIs(token.SEMICOLON) {
			p.nextToken()
		}
		return stmt
	}

	p.nextToken() // Consumes '->'

	// Parse the return type annotation
	stmt.ReturnType = p.parseTypeAnnotation()
	if stmt.ReturnType == nil {
		return nil
	}

	// Optionally consume a final semicolon.
	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}
	return stmt
}

// parseStructLiteral parses struct definitions like "struct { a: int64, b: string }"
func (p *Parser) parseStructLiteral() ast.Expression {
	lit := &ast.StructLiteral{Token: p.curToken}

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	lit.Fields = p.parseStructFields()

	// The RBRACE is consumed by parseStructFields
	return lit
}

// parseStructFields parses the field definitions inside a struct
func (p *Parser) parseStructFields() []*ast.StructField {
	fields := []*ast.StructField{}

	if p.peekTokenIs(token.RBRACE) {
		p.nextToken()
		return fields
	}

	p.nextToken()

	// Parse first field
	field := &ast.StructField{}
	if !p.curTokenIs(token.IDENT) {
		return nil
	}
	field.Name = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	if !p.expectPeek(token.COLON) {
		return nil
	}
	if !p.expectPeek(token.IDENT) {
		return nil
	}
	field.Type = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}
	fields = append(fields, field)

	// Parse additional fields
	for p.peekTokenIs(token.COMMA) {
		p.nextToken() // consume comma
		p.nextToken() // move to next field

		field := &ast.StructField{}
		if !p.curTokenIs(token.IDENT) {
			return nil
		}
		field.Name = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

		if !p.expectPeek(token.COLON) {
			return nil
		}
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		field.Type = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}
		fields = append(fields, field)
	}

	if !p.expectPeek(token.RBRACE) {
		return nil
	}

	return fields
}

// parseStructInstanceExpression parses struct instantiation like "struct_def { a: 42, b: "hello" }"
func (p *Parser) parseStructInstanceExpression(left ast.Expression) ast.Expression {
	exp := &ast.StructInstanceExpression{
		Token:      p.curToken, // The '{' token
		StructExpr: left,
		Fields:     make(map[string]ast.Expression),
	}

	if p.peekTokenIs(token.RBRACE) {
		p.nextToken()
		return exp
	}

	p.nextToken()

	// Parse first field
	if !p.curTokenIs(token.IDENT) {
		return nil
	}

	fieldName := p.curToken.Literal

	if !p.expectPeek(token.COLON) {
		return nil
	}

	p.nextToken()
	fieldValue := p.parseExpression(LOWEST)
	exp.Fields[fieldName] = fieldValue

	// Parse additional fields
	for p.peekTokenIs(token.COMMA) {
		p.nextToken()
		p.nextToken()

		if !p.curTokenIs(token.IDENT) {
			return nil
		}

		fieldName := p.curToken.Literal

		if !p.expectPeek(token.COLON) {
			return nil
		}

		p.nextToken()
		fieldValue := p.parseExpression(LOWEST)
		exp.Fields[fieldName] = fieldValue
	}

	if !p.expectPeek(token.RBRACE) {
		return nil
	}

	return exp
}

func (p *Parser) parseErrorPropagationExpression(left ast.Expression) ast.Expression {
	exp := &ast.ErrorPropagationExpression{
		Token:      p.curToken,
		Expression: left,
	}

	return exp
}

// parseEnumExpression parses enum expressions like "enum { Some(int64), None }"
func (p *Parser) parseEnumExpression() ast.Expression {
	expr := &ast.EnumLiteral{Token: p.curToken}

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	expr.Variants = p.parseEnumVariants()

	// parseEnumVariants should leave us positioned at the closing brace
	if !p.curTokenIs(token.RBRACE) {
		return nil
	}

	return expr
}

// parseEnumVariants parses the variants inside an enum definition
func (p *Parser) parseEnumVariants() []*ast.EnumVariant {
	variants := []*ast.EnumVariant{}

	if p.peekTokenIs(token.RBRACE) {
		return variants
	}

	p.nextToken()

	for !p.curTokenIs(token.RBRACE) && !p.curTokenIs(token.EOF) {
		variant := &ast.EnumVariant{}
		if !p.curTokenIs(token.IDENT) {
			return nil
		}
		variant.Name = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

		// Check for optional payload
		if p.peekTokenIs(token.LPAREN) {
			p.nextToken() // consume '('
			variant.Payload = p.parseTypeAnnotation()
			if variant.Payload == nil {
				return nil
			}
			if !p.expectPeek(token.RPAREN) {
				return nil
			}
		}

		variants = append(variants, variant)

		if p.peekTokenIs(token.COMMA) {
			p.nextToken()
			p.nextToken()
		} else if p.peekTokenIs(token.RBRACE) {
			break
		} else {
			return nil // Malformed
		}
	}

	return variants
}

// parseMatchExpression parses match expressions like "match value { Pattern => expr, ... }"
func (p *Parser) parseMatchExpression() ast.Expression {
	exp := &ast.MatchExpression{Token: p.curToken}

	p.nextToken()

	// Temporarily remove LBRACE from infix operators to prevent it from being consumed
	lbraceInfix := p.infixParseFns[token.LBRACE]
	delete(p.infixParseFns, token.LBRACE)

	exp.Condition = p.parseExpression(LOWEST)

	// Restore LBRACE infix operator
	p.infixParseFns[token.LBRACE] = lbraceInfix

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	exp.Arms = p.parseMatchArms()

	if !p.expectPeek(token.RBRACE) {
		return nil
	}

	return exp
}

// parseMatchArms parses the arms inside a match expression
func (p *Parser) parseMatchArms() []*ast.MatchArm {
	arms := []*ast.MatchArm{}

	if p.peekTokenIs(token.RBRACE) {
		return arms
	}

	p.nextToken()

	for !p.curTokenIs(token.RBRACE) && !p.curTokenIs(token.EOF) {
		arm := &ast.MatchArm{}

		// Parse the pattern (EnumInstantiationExpression)
		pattern := p.parseEnumInstantiationPattern()
		if pattern == nil {
			return nil
		}
		arm.Pattern = pattern

		if !p.expectPeek(token.FATARROW) {
			return nil
		}

		p.nextToken()
		arm.Consequence = p.parseExpression(LOWEST)

		arms = append(arms, arm)

		if p.peekTokenIs(token.COMMA) {
			p.nextToken() // consume comma
			// Check if we're at the end (trailing comma)
			if p.peekTokenIs(token.RBRACE) {
				break
			}
			// Move to next match arm
			p.nextToken()
		} else if p.peekTokenIs(token.RBRACE) {
			break
		} else {
			return nil // Malformed
		}
	}

	return arms
}

// parseEnumInstantiationPattern parses enum instantiation patterns like "Option::Some(x)"
func (p *Parser) parseEnumInstantiationPattern() *ast.EnumInstantiationExpression {
	if !p.curTokenIs(token.IDENT) {
		return nil
	}

	// Parse the enum path (e.g., "Option")
	enumPath := &ast.PathExpression{
		Token:    p.curToken,
		Segments: []*ast.Identifier{{Token: p.curToken, Value: p.curToken.Literal}},
	}

	if !p.expectPeek(token.COLONCOLON) {
		return nil
	}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	// Parse the variant name (e.g., "Some")
	variant := &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	exp := &ast.EnumInstantiationExpression{
		Token:     enumPath.Token,
		Enum:      enumPath,
		Variant:   variant,
		Arguments: []ast.Expression{},
	}

	// Check for optional arguments
	if p.peekTokenIs(token.LPAREN) {
		p.nextToken() // consume '('

		// For patterns, we expect identifiers (pattern variables)
		if !p.peekTokenIs(token.RPAREN) {
			p.nextToken()
			if p.curTokenIs(token.IDENT) {
				exp.Arguments = append(exp.Arguments, &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal})
			}
		}

		if !p.expectPeek(token.RPAREN) {
			return nil
		}
	}

	return exp
}

func (p *Parser) parseTypeAnnotation() *ast.TypeAnnotation {
	// Handle pointer syntax: *Type
	if p.peekTokenIs(token.ASTERISK) {
		p.nextToken() // consume '*'
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		return &ast.TypeAnnotation{
			Token:     p.curToken,
			Value:     p.curToken.Literal,
			IsPointer: true,
		}
	}

	// Handle simple type
	if !p.expectPeek(token.IDENT) {
		return nil
	}
	return &ast.TypeAnnotation{
		Token:     p.curToken,
		Value:     p.curToken.Literal,
		IsPointer: false,
	}
}
