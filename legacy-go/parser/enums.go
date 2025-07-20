package parser

import (
	"github.com/behzade/basalt/ast"
	"github.com/behzade/basalt/token"
)

// parseEnumInstantiation parses enum instantiation expressions like "Option::Some(42)"
func (p *Parser) parseEnumInstantiation() ast.Expression {
	enumPath := p.parsePathExpression()
	if enumPath == nil {
		return nil
	}
	// The path must have at least two parts (Enum::Variant)
	if len(enumPath.Segments) < 2 {
		p.errors = append(p.errors, ParserError{"enum instantiation requires a '::' separator", enumPath.Token.Line, enumPath.Token.Column})
		return nil
	}

	// This logic is slightly adjusted to work with the pre-parsed path
	variantIndex := len(enumPath.Segments) - 1
	variant := enumPath.Segments[variantIndex]
	enumPath.Segments = enumPath.Segments[:variantIndex] // The path is everything before the variant

	exp := &ast.EnumInstantiationExpression{
		Token:     enumPath.Token,
		Enum:      enumPath,
		Variant:   variant,
		Arguments: []ast.Expression{},
	}

	if p.peekTokenIs(token.LPAREN) {
		p.nextToken()
		exp.Arguments = p.parseCallArguments()
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
