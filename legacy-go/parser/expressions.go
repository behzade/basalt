package parser

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

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

func (p *Parser) parseErrorPropagationExpression(left ast.Expression) ast.Expression {
	exp := &ast.ErrorPropagationExpression{
		Token:      p.curToken,
		Expression: left,
	}

	return exp
}
