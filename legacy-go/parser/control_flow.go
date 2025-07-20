package parser

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

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

	path := p.parsePathExpression()
	if path == nil || len(path.Segments) < 2 {
		return nil
	}

	variantIndex := len(path.Segments) - 1
	variant := path.Segments[variantIndex]
	enumPath := &ast.PathExpression{Token: path.Token, Segments: path.Segments[:variantIndex]}

	exp := &ast.EnumInstantiationExpression{
		Token:     enumPath.Token,
		Enum:      enumPath,
		Variant:   variant,
		Arguments: []ast.Expression{},
	}

	if p.peekTokenIs(token.LPAREN) {
		p.nextToken()
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
