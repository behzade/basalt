package parser

import (
	"fmt"

	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

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

// parseStructLiteral parses struct definitions like "struct { a: int64, b: string }"
func (p *Parser) parseStructLiteral() ast.Expression {
	lit := &ast.StructLiteral{Token: p.curToken}

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	// Temporarily remove LBRACE from infix operators to prevent it from being consumed
	// by parseExpression when we return to parseLetStatement
	lbraceInfix := p.infixParseFns[token.LBRACE]
	delete(p.infixParseFns, token.LBRACE)

	lit.Fields = p.parseStructFields()

	// Restore LBRACE infix operator
	p.infixParseFns[token.LBRACE] = lbraceInfix

	// The RBRACE is consumed by parseStructFields
	return lit
}

// parseStructFields parses the field definitions inside a struct
func (p *Parser) parseStructFields() []*ast.StructField {
	fields := []*ast.StructField{}

	// On entry, curToken is '{'. We need to parse a list of `identifier: type`
	// until we hit '}'.

	// Loop until we see the closing brace.
	for !p.peekTokenIs(token.RBRACE) {
		p.nextToken() // Move to the field name (or first token)

		if !p.curTokenIs(token.IDENT) {
			p.errors = append(p.errors, ParserError{
				Msg:  fmt.Sprintf("expected field name identifier in struct definition, got %s", p.curToken.Type),
				Line: p.curToken.Line,
				Col:  p.curToken.Column,
			})
			return nil
		}

		field := &ast.StructField{
			Name: &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal},
		}

		if !p.expectPeek(token.COLON) {
			return nil
		}

		// This correctly uses the powerful type annotation parser from before
		field.Type = p.parseTypeAnnotation()
		if field.Type == nil {
			return nil
		}
		fields = append(fields, field)

		// After a field, we must see a comma or the closing brace.
		if !p.peekTokenIs(token.RBRACE) {
			if !p.expectPeek(token.COMMA) { // Expect a comma to continue
				return nil
			}
		}
	}

	// Consume the final '}'
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
