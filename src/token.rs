use std::fmt;

pub type SimpleSpan = chumsky::span::SimpleSpan;

/// Represents the different kinds of tokens recognized by the lexer and used by the parser.
/// The `'src` lifetime parameter is used for tokens that borrow directly from the source code,
/// such as identifiers, strings, and comments, allowing for zero-copy parsing.
#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
    // Keywords
    Let,
    Mut,
    Struct,
    Enum,
    Trait,
    Satisfies,
    For,
    Fn,
    Extern,
    Import,
    As,
    While,
    Return,
    Effect,
    Handler,
    Perform,
    Handle,
    With,
    Match,
    If,
    Else,
    Pub,

    // Literals
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(&'src str),

    // Identifier
    Ident(&'src str),

    // Operators and Punctuation
    Op(String),  // For operators like +, -, *, /, <, >, ==, =
    DoubleColon, // ::
    Colon,       // :
    Comma,       // ,
    Arrow,       // ->
    FatArrow,    // =>
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Hash,        // #

    // Ignored token
    Comment(&'src str),
}

/// Implementation of the `Display` trait for `Token`.
/// This is useful for debugging and for generating more readable error messages.
impl<'src> fmt::Display for Token<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Keywords
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::Struct => write!(f, "struct"),
            Token::Enum => write!(f, "enum"),
            Token::Trait => write!(f, "trait"),
            Token::Satisfies => write!(f, "satisfies"),
            Token::For => write!(f, "for"),
            Token::Fn => write!(f, "fn"),
            Token::Extern => write!(f, "extern"),
            Token::Import => write!(f, "import"),
            Token::As => write!(f, "as"),
            Token::While => write!(f, "while"),
            Token::Return => write!(f, "return"),
            Token::Effect => write!(f, "effect"),
            Token::Handler => write!(f, "handler"),
            Token::Perform => write!(f, "perform"),
            Token::Handle => write!(f, "handle"),
            Token::With => write!(f, "with"),
            Token::Match => write!(f, "match"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Pub => write!(f, "pub"),

            // Literals
            Token::Bool(b) => write!(f, "{}", b),
            Token::I64(i) => write!(f, "{}", i),
            Token::F64(fl) => write!(f, "{}", fl),
            Token::Str(s) => write!(f, "\"{}\"", s),

            // Identifier
            Token::Ident(ident) => write!(f, "{}", ident),

            // Operators and Punctuation
            Token::Op(op) => write!(f, "{}", op),
            Token::DoubleColon => write!(f, "::"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),

            // Comment
            Token::Comment(c) => write!(f, "//{}", c),
            Token::Hash => write!(f, "#"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OwnedToken {
    Let,
    Mut,
    Struct,
    Enum,
    Trait,
    Satisfies,
    For,
    Fn,
    Extern,
    Import,
    As,
    While,
    Return,
    Effect,
    Handler,
    Perform,
    Handle,
    With,
    Match,
    If,
    Else,
    Pub,

    // Literals
    Bool(bool),
    I64(i64),
    F64(f64),
    Str(String),

    // Identifier
    Ident(String),

    // Operators and Punctuation
    Op(String),  // For operators like +, -, *, /, <, >, ==, =
    DoubleColon, // ::
    Colon,       // :
    Comma,       // ,
    Arrow,       // ->
    FatArrow,    // =>
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Hash,        // #

    // Ignored token
    Comment(String),
}

pub type OwnedTokenWithSpan = (OwnedToken, SimpleSpan);

// convert from token to owned token
impl From<Token<'_>> for OwnedToken {
    fn from(token: Token<'_>) -> Self {
        match token {
            Token::Let => OwnedToken::Let,
            Token::Mut => OwnedToken::Mut,
            Token::Struct => OwnedToken::Struct,
            Token::Enum => OwnedToken::Enum,
            Token::Trait => OwnedToken::Trait,
            Token::Satisfies => OwnedToken::Satisfies,
            Token::For => OwnedToken::For,
            Token::Fn => OwnedToken::Fn,
            Token::Extern => OwnedToken::Extern,
            Token::Import => OwnedToken::Import,
            Token::As => OwnedToken::As,
            Token::While => OwnedToken::While,
            Token::Return => OwnedToken::Return,
            Token::Effect => OwnedToken::Effect,
            Token::Handler => OwnedToken::Handler,
            Token::Perform => OwnedToken::Perform,
            Token::Handle => OwnedToken::Handle,
            Token::With => OwnedToken::With,
            Token::Match => OwnedToken::Match,
            Token::If => OwnedToken::If,
            Token::Else => OwnedToken::Else,
            Token::Pub => OwnedToken::Pub,
            Token::Bool(b) => OwnedToken::Bool(b),
            Token::I64(i) => OwnedToken::I64(i),
            Token::F64(fl) => OwnedToken::F64(fl),
            Token::Str(s) => OwnedToken::Str(s.to_string()),
            Token::Ident(ident) => OwnedToken::Ident(ident.to_string()),
            Token::Op(op) => OwnedToken::Op(op.to_string()),
            Token::DoubleColon => OwnedToken::DoubleColon,
            Token::Colon => OwnedToken::Colon,
            Token::Comma => OwnedToken::Comma,
            Token::Arrow => OwnedToken::Arrow,
            Token::FatArrow => OwnedToken::FatArrow,
            Token::LParen => OwnedToken::LParen,
            Token::RParen => OwnedToken::RParen,
            Token::LBrace => OwnedToken::LBrace,
            Token::RBrace => OwnedToken::RBrace,
            Token::LBracket => OwnedToken::LBracket,
            Token::RBracket => OwnedToken::RBracket,
            Token::Hash => OwnedToken::Hash,
            Token::Comment(c) => OwnedToken::Comment(c.to_string()),
        }
    }
}
