package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
)

// Data structure checking methods

func (c *Checker) checkStructLiteral(node *ast.StructLiteral) Type {
	c.addError("struct literals must be assigned to a variable", node.Token)
	return &NoneType{}
}

func (c *Checker) checkStructInstanceExpression(node *ast.StructInstanceExpression) Type {
	structTypeVal := c.Check(node.StructExpr)
	structDef, ok := structTypeVal.(*StructType)
	if !ok {
		c.addError(fmt.Sprintf("cannot instantiate non-struct type: %s", structTypeVal.String()), node.Token)
		return &NoneType{}
	}
	for name, expr := range node.Fields {
		expectedType, ok := structDef.Fields[name]
		if !ok {
			c.addError(fmt.Sprintf("field '%s' not found in struct %s", name, structDef.Name), node.Token)
			continue
		}
		actualType := c.Check(expr)
		if !c.isAssignable(actualType, expectedType) {
			c.addError(fmt.Sprintf("field '%s' expects type %s, got %s", name, expectedType.String(), actualType.String()), node.Token)
		}
	}
	for name := range structDef.Fields {
		if _, ok := node.Fields[name]; !ok {
			c.addError(fmt.Sprintf("missing field '%s' in instantiation of struct %s", name, structDef.Name), node.Token)
		}
	}
	return structDef
}
