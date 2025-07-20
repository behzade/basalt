package parser

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

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
	case token.UNSAFE:
		stmt = p.parseUnsafeStatement()
	default:
		stmt = p.parseExpressionStatement()
	}

	// Apply attributes logic remains the same
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

	if p.peekTokenIs(token.MUT) {
		stmt.Mutable = true
		p.nextToken()
	}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	stmt.Name = &ast.Identifier{Token: p.curToken, Value: p.curToken.Literal}

	if p.peekTokenIs(token.COLON) {
		p.nextToken()
		// MODIFIED: This now calls the more powerful type parser
		stmt.Type = p.parseTypeAnnotation()
		if stmt.Type == nil {
			return nil
		}
	}

	// Assignment is optional if there's a type annotation
	if p.peekTokenIs(token.ASSIGN) {
		p.nextToken()
		p.nextToken()
		stmt.Value = p.parseExpression(LOWEST)
	} else if stmt.Type == nil {
		// If there's no type annotation, assignment is required
		if !p.expectPeek(token.ASSIGN) {
			return nil
		}
		p.nextToken()
		stmt.Value = p.parseExpression(LOWEST)
	}

	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}

	return stmt
}

func (p *Parser) parseReturnStatement() *ast.ReturnStatement {
	stmt := &ast.ReturnStatement{Token: p.curToken}

	p.nextToken()

	// Check if this is an empty return (return;)
	if p.curTokenIs(token.SEMICOLON) {
		stmt.ReturnValue = nil
		return stmt
	}

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

func (p *Parser) parseImportStatement() *ast.ImportStatement {
	stmt := &ast.ImportStatement{Token: p.curToken}

	if !p.expectPeek(token.IDENT) {
		return nil
	}

	// Use the new helper to parse the qualified path
	stmt.Path = p.parsePathExpression()
	if stmt.Path == nil {
		return nil
	}

	if p.peekTokenIs(token.AS) {
		p.nextToken()
		if !p.expectPeek(token.IDENT) {
			return nil
		}
		stmt.Alias = &ast.Identifier{
			Token: p.curToken,
			Value: p.curToken.Literal,
		}
	}

	if p.peekTokenIs(token.SEMICOLON) {
		p.nextToken()
	}

	return stmt
}

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

func (p *Parser) parseUnsafeStatement() *ast.UnsafeStatement {
	stmt := &ast.UnsafeStatement{Token: p.curToken}

	if !p.expectPeek(token.LBRACE) {
		return nil
	}

	stmt.Body = p.parseBlockStatement()

	return stmt
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
