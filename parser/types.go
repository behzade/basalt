package parser

import (
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

func (p *Parser) parseTypeAnnotation() *ast.TypeAnnotation {
	// Temporarily remove LT and GT from infix operators to prevent them from being consumed
	// as comparison operators when parsing generic parameters
	ltInfix := p.infixParseFns[token.LT]
	gtInfix := p.infixParseFns[token.GT]
	delete(p.infixParseFns, token.LT)
	delete(p.infixParseFns, token.GT)

	// Restore LT and GT infix operators when we're done
	defer func() {
		p.infixParseFns[token.LT] = ltInfix
		p.infixParseFns[token.GT] = gtInfix
	}()

	isPointer := false
	if p.peekTokenIs(token.ASTERISK) {
		isPointer = true
		p.nextToken() // consume '*'
	}

	// Expect either an IDENT or the RAWPTR keyword.
	if !p.peekTokenIs(token.IDENT) && !p.peekTokenIs(token.RAWPTR) {
		p.peekError(token.IDENT) // Error still reports IDENT as expected type
		return nil
	}
	p.nextToken()

	// Handle rawptr as a special case
	if p.curTokenIs(token.RAWPTR) {
		return &ast.TypeAnnotation{
			Token:         p.curToken,
			Value:         "rawptr",
			IsPointer:     isPointer,
			GenericParams: nil,
		}
	}

	// Use the path parsing helper to handle `MyModule::MyType`
	path := p.parsePathExpression()
	if path == nil {
		return nil
	}

	// Serialize the path segments back into a single string for the AST node.
	// This avoids changing the AST, but still allows the parser to understand the syntax.
	pathSegments := make([]string, len(path.Segments))
	for i, seg := range path.Segments {
		pathSegments[i] = seg.Value
	}

	typeAnnotation := &ast.TypeAnnotation{
		Token:         path.Token,
		Value:         strings.Join(pathSegments, "::"),
		IsPointer:     isPointer,
		GenericParams: nil,
	}

	// Check if this type has generic parameters
	if p.peekTokenIs(token.LT) {
		p.nextToken() // consume '<'

		genericParams := p.parseGenericParameters()
		if genericParams == nil {
			return nil
		}

		typeAnnotation.GenericParams = genericParams
	}

	return typeAnnotation
}

// parseGenericParameters parses a comma-separated list of type annotations within < >
func (p *Parser) parseGenericParameters() []*ast.TypeAnnotation {
	var params []*ast.TypeAnnotation

	// Handle empty generic parameters
	if p.peekTokenIs(token.GT) {
		p.nextToken() // consume '>'
		return params
	}

	// Parse first parameter
	param := p.parseTypeAnnotation()
	if param == nil {
		return nil
	}
	params = append(params, param)

	// Parse remaining parameters
	for p.peekTokenIs(token.COMMA) {
		p.nextToken() // consume ','

		param := p.parseTypeAnnotation()
		if param == nil {
			return nil
		}
		params = append(params, param)
	}

	// Expect closing '>'
	if !p.expectPeek(token.GT) {
		return nil
	}

	return params
}

func (p *Parser) parsePathExpression() *ast.PathExpression {
	path := &ast.PathExpression{
		Token:    p.curToken,
		Segments: []*ast.Identifier{},
	}

	if !p.curTokenIs(token.IDENT) {
		return nil
	}
	path.Segments = append(path.Segments, &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal})

	for p.peekTokenIs(token.COLONCOLON) {
		p.nextToken() // consume ::
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		path.Segments = append(path.Segments, &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal})
	}

	return path
}
