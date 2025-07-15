package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
)

// Control flow checking methods

func (c *Checker) checkIfExpression(node *ast.IfExpression) Type {
	condType := c.Check(node.Condition)
	if !condType.Equals(&BooleanType{}) {
		c.addError(fmt.Sprintf("if condition must be boolean, got %s", condType.String()), node.Token)
	}
	consequenceType := c.Check(node.Consequence)
	if node.Alternative != nil {
		alternativeType := c.Check(node.Alternative)
		if !consequenceType.Equals(alternativeType) {
			c.addError(fmt.Sprintf("if branches have different types: %s vs %s", consequenceType.String(), alternativeType.String()), node.Token)
		}
		return consequenceType
	}
	return &NoneType{}
}

func (c *Checker) checkForExpression(node *ast.ForExpression) Type {
	condType := c.Check(node.Condition)
	if !condType.Equals(&BooleanType{}) {
		c.addError(fmt.Sprintf("for condition must be boolean, got %s", condType.String()), node.Token)
	}
	c.Check(node.Consequence)
	return &NoneType{}
}

func (c *Checker) checkUnsafeStatement(node *ast.UnsafeStatement) Type {
	// Set the unsafe context flag, check the body, then reset it.
	c.isInUnsafeContext = true
	defer func() { c.isInUnsafeContext = false }()

	return c.Check(node.Body)
}
