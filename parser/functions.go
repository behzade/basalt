package parser

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

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
