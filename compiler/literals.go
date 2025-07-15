package compiler

import (
	"fmt"
	"strings"

	"github.com/behzade/basalt/ast"
	"github.com/llir/llvm/ir/constant"
	"github.com/llir/llvm/ir/enum"
	"github.com/llir/llvm/ir/types"
	"github.com/llir/llvm/ir/value"
)

// compileIntegerLiteral compiles an integer literal
func (c *Compiler) compileIntegerLiteral(expr *ast.IntegerLiteral) (value.Value, error) {
	return constant.NewInt(types.I64, expr.Value), nil
}

// compileBoolean compiles a boolean literal
func (c *Compiler) compileBoolean(expr *ast.Boolean) (value.Value, error) {
	if expr.Value {
		return constant.NewBool(true), nil
	}
	return constant.NewBool(false), nil
}

// compileFloatLiteral compiles a float literal
func (c *Compiler) compileFloatLiteral(expr *ast.FloatLiteral) (value.Value, error) {
	return constant.NewFloat(types.Double, expr.Value), nil
}

// compileStringLiteral compiles a string literal to a global string constant
func (c *Compiler) compileStringLiteral(expr *ast.StringLiteral) (value.Value, error) {
	// 1. Un-escape the raw string value to handle sequences like \n
	processedValue, err := unescapeString(expr.Value)
	if err != nil {
		return nil, err // Propagate error if the escape sequence is invalid
	}

	// Create a global string constant
	// LLVM string constants are arrays of i8 with null terminator
	stringValue := processedValue + "\x00" // Add null terminator

	// Create character array type
	charArrayType := types.NewArray(uint64(len(stringValue)), types.I8)

	// Create the global string constant
	globalName := fmt.Sprintf("str_%d", c.blockCounter)
	c.blockCounter++

	// Create constant character array
	chars := make([]constant.Constant, len(stringValue))
	for i, char := range stringValue {
		chars[i] = constant.NewInt(types.I8, int64(char))
	}
	charArray := constant.NewArray(charArrayType, chars...)

	// Create global variable for the string
	global := c.module.NewGlobalDef(globalName, charArray)
	global.Linkage = enum.LinkagePrivate
	global.UnnamedAddr = enum.UnnamedAddrUnnamedAddr

	// Return a pointer to the first character (i8*)
	// Use GetElementPtr to get pointer to first element
	zero := constant.NewInt(types.I64, 0)
	return constant.NewGetElementPtr(charArrayType, global, zero, zero), nil
}

// unescapeString processes escape sequences in string literals
func unescapeString(raw string) (string, error) {
	var sb strings.Builder
	// The raw string from the parser includes the surrounding quotes,
	// so we can iterate from the second character to the second-to-last.
	// If your parser provides the string WITHOUT quotes, use `for i := 0; i < len(raw); i++`
	for i := 0; i < len(raw); i++ {
		char := raw[i]
		if char == '\\' {
			// Make sure there is a character after the backslash
			if i+1 >= len(raw) {
				return "", fmt.Errorf("invalid escape sequence at end of string")
			}
			i++ // Move to the character after '\'
			switch raw[i] {
			case 'n':
				sb.WriteRune('\n')
			case 't':
				sb.WriteRune('\t')
			case '\\':
				sb.WriteRune('\\')
			case '"':
				sb.WriteRune('"')
			// Add other escapes as needed (e.g., \r)
			default:
				// Optional: return an error for unknown escape sequences
				return "", fmt.Errorf("unknown escape sequence: \\%c", raw[i])
			}
		} else {
			sb.WriteRune(rune(char))
		}
	}
	return sb.String(), nil
}
