package checker

import (
	"fmt"

	"github.com/behzade/basalt/ast"
)

// Enum checking methods

func (c *Checker) checkEnumLiteral(node *ast.EnumLiteral) Type {
	variants := make(map[string]*EnumVariantType)

	for _, variant := range node.Variants {
		variantType := &EnumVariantType{
			Name:        variant.Name.Value,
			PayloadType: nil,
		}

		if variant.Payload != nil {
			variantType.PayloadType = c.parseTypeAnnotation(variant.Payload)
		}

		variants[variant.Name.Value] = variantType
	}

	enumType := &EnumType{
		Name:     "",
		Variants: variants,
	}

	return enumType
}

func (c *Checker) checkEnumInstantiationExpression(node *ast.EnumInstantiationExpression) Type {
	enumName := node.Enum.Segments[0].Value
	enumTypeVal, ok := c.env.Get(enumName)
	if !ok {
		c.addError(fmt.Sprintf("unknown enum type: %s", enumName), node.Token)
		return &NoneType{}
	}

	enumType, ok := enumTypeVal.(*EnumType)
	if !ok {
		c.addError(fmt.Sprintf("%s is not an enum type", enumName), node.Token)
		return &NoneType{}
	}

	variantName := node.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		c.addError(fmt.Sprintf("variant %s not found in enum %s", variantName, enumName), node.Token)
		return &NoneType{}
	}

	if variant.PayloadType == nil {
		if len(node.Arguments) > 0 {
			c.addError(fmt.Sprintf("variant %s::%s expects no arguments, got %d", enumName, variantName, len(node.Arguments)), node.Token)
		}
	} else {
		if len(node.Arguments) != 1 {
			c.addError(fmt.Sprintf("variant %s::%s expects 1 argument, got %d", enumName, variantName, len(node.Arguments)), node.Token)
		} else {
			argType := c.Check(node.Arguments[0])
			if !c.isAssignable(argType, variant.PayloadType) {
				c.addError(fmt.Sprintf("variant %s::%s expects argument of type %s, got %s", enumName, variantName, variant.PayloadType.String(), argType.String()), node.Token)
			}
		}
	}

	return enumType
}

func (c *Checker) checkMatchExpression(node *ast.MatchExpression) Type {
	conditionType := c.Check(node.Condition)
	enumType, ok := conditionType.(*EnumType)
	if !ok {
		c.addError(fmt.Sprintf("match expression can only be used with enum types, got %s", conditionType.String()), node.Token)
		return &NoneType{}
	}

	coveredVariants := make(map[string]bool)
	var armTypes []Type

	for _, arm := range node.Arms {
		patternType := c.checkMatchPattern(arm.Pattern, enumType)
		if patternType == nil {
			continue
		}

		variantName := arm.Pattern.Variant.Value
		coveredVariants[variantName] = true

		consequenceEnv := NewEnclosedTypeEnvironment(c.env)
		c.addPatternVariables(arm.Pattern, enumType, consequenceEnv)

		savedEnv := c.env
		c.env = consequenceEnv
		armType := c.Check(arm.Consequence)
		c.env = savedEnv

		armTypes = append(armTypes, armType)
	}

	for variantName := range enumType.Variants {
		if !coveredVariants[variantName] {
			c.addError(fmt.Sprintf("match expression is not exhaustive: missing variant %s::%s", enumType.Name, variantName), node.Token)
		}
	}

	if len(armTypes) == 0 {
		return &NoneType{}
	}

	firstType := armTypes[0]
	for i, armType := range armTypes {
		if !armType.Equals(firstType) {
			c.addError(fmt.Sprintf("match arm %d returns type %s, expected %s", i+1, armType.String(), firstType.String()), node.Token)
		}
	}

	return firstType
}

func (c *Checker) checkMatchPattern(pattern *ast.EnumInstantiationExpression, enumType *EnumType) *EnumVariantType {
	patternEnumName := pattern.Enum.Segments[0].Value
	if patternEnumName != enumType.Name {
		c.addError(fmt.Sprintf("pattern uses enum %s, but matching against %s", patternEnumName, enumType.Name), pattern.Token)
		return nil
	}

	variantName := pattern.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		c.addError(fmt.Sprintf("variant %s not found in enum %s", variantName, enumType.Name), pattern.Token)
		return nil
	}

	if variant.PayloadType == nil {
		if len(pattern.Arguments) > 0 {
			c.addError(fmt.Sprintf("variant %s::%s has no payload, but pattern has %d arguments", enumType.Name, variantName, len(pattern.Arguments)), pattern.Token)
		}
	} else {
		if len(pattern.Arguments) != 1 {
			c.addError(fmt.Sprintf("variant %s::%s expects 1 pattern argument, got %d", enumType.Name, variantName, len(pattern.Arguments)), pattern.Token)
		} else {
			if node, ok := pattern.Arguments[0].(*ast.Identifier); !ok {
				c.addError(
					fmt.Sprintf("pattern argument must be an identifier, got %T", pattern.Arguments[0]),
					node.Token,
				)
			}
		}
	}

	return variant
}

func (c *Checker) addPatternVariables(pattern *ast.EnumInstantiationExpression, enumType *EnumType, env *TypeEnvironment) {
	variantName := pattern.Variant.Value
	variant, ok := enumType.Variants[variantName]
	if !ok {
		return
	}

	if variant.PayloadType != nil && len(pattern.Arguments) == 1 {
		if ident, ok := pattern.Arguments[0].(*ast.Identifier); ok {
			env.Set(ident.Value, variant.PayloadType)
		}
	}
}
