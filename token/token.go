package token

// TokenType is a string to allow for flexibility and easy debugging.
type TokenType string

// Token represents a single token with its type and literal value.
type Token struct {
	Type    TokenType
	Literal string
}

// Defines all possible token types in our language.
const (
	// Special Tokens
	ILLEGAL = "ILLEGAL" // A token/character we don't know about
	EOF     = "EOF"     // "End of File"

	// Identifiers + Literals
	IDENT  = "IDENT"  // add, foobar, x, y, ...
	INT    = "INT"    // 1343456
	FLOAT  = "FLOAT"  // 3.14, 2.5, 0.1
	STRING = "STRING" // "hello world"

	// Operators
	ASSIGN   = "="
	PLUS     = "+"
	MINUS    = "-"
	BANG     = "!"
	ASTERISK = "*"
	SLASH    = "/"
	MODULO   = "%"
	POW      = "**"
	LT       = "<"
	GT       = ">"
	EQ       = "=="
	NOT_EQ   = "!="

	// Delimiters
	COMMA      = ","
	SEMICOLON  = ";"
	COLON      = ":"
	COLONCOLON = "::"
	DOT        = "."
	LPAREN     = "("
	RPAREN     = ")"
	LBRACE     = "{"
	RBRACE     = "}"
	LBRACKET   = "["
	RBRACKET   = "]"

	// Keywords
	FUNCTION  = "FUNCTION"
	LET       = "LET"
	MUT       = "MUT"
	TRUE      = "TRUE"
	FALSE     = "FALSE"
	IF        = "IF"
	ELSE      = "ELSE"
	RETURN    = "RETURN"
	STRUCT    = "STRUCT"
	INTERFACE = "INTERFACE"
	IMPORT    = "IMPORT"
	AS        = "AS"
	FOR       = "FOR"
)

// keywords maps keyword strings to their TokenType.
var keywords = map[string]TokenType{
	"fn":        FUNCTION,
	"let":       LET,
	"mut":       MUT,
	"true":      TRUE,
	"false":     FALSE,
	"if":        IF,
	"else":      ELSE,
	"return":    RETURN,
	"struct":    STRUCT,
	"interface": INTERFACE,
	"import":    IMPORT,
	"as":        AS,
	"for":       FOR,
}

// LookupIdent checks the keywords table to see if an identifier is a keyword.
func LookupIdent(ident string) TokenType {
	if tok, ok := keywords[ident]; ok {
		return tok
	}
	return IDENT
}
