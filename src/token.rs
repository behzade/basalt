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
    Type,
    Struct,
    Enum,
    Trait,
    Interface,
    Satisfies,
    Fn,
    Impl,
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
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Str(&'src str),

    // Identifier
    Ident(&'src str),

    // Operators and Punctuation
    Op(String),  // For operators like +, -, *, /, <, >, ==, =
    Colon,       // :
    Comma,       // ,
    Arrow,       // ->
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Semicolon,   // ;
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
            Token::Type => write!(f, "type"),
            Token::Interface => write!(f, "interface"),
            Token::Satisfies => write!(f, "satisfies"),
            Token::Fn => write!(f, "fn"),
            Token::Impl => write!(f, "impl"),
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
            Token::I8(i) => write!(f, "{}", i),
            Token::I16(i) => write!(f, "{}", i),
            Token::I32(i) => write!(f, "{}", i),
            Token::I64(i) => write!(f, "{}", i),
            Token::U8(i) => write!(f, "{}", i),
            Token::U16(i) => write!(f, "{}", i),
            Token::U32(i) => write!(f, "{}", i),
            Token::U64(i) => write!(f, "{}", i),
            Token::F32(fl) => write!(f, "{}", fl),
            Token::F64(fl) => write!(f, "{}", fl),
            Token::Str(s) => write!(f, "\"{}\"", s),

            // Identifier
            Token::Ident(ident) => write!(f, "{}", ident),

            // Operators and Punctuation
            Token::Op(op) => write!(f, "{}", op),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Arrow => write!(f, "->"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Semicolon => write!(f, ";"),

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
    Type,
    Struct,
    Enum,
    Trait,
    Interface,
    Satisfies,
    Fn,
    Impl,
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
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Str(String),

    // Identifier
    Ident(String),

    // Operators and Punctuation
    Op(String),  // For operators like +, -, *, /, <, >, ==, =
    Colon,       // :
    Comma,       // ,
    Arrow,       // ->
    LParen,      // (
    RParen,      // )
    LBrace,      // {
    RBrace,      // }
    LBracket,    // [
    RBracket,    // ]
    Semicolon,   // ;
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
            Token::Type => OwnedToken::Type,
            Token::Interface => OwnedToken::Interface,
            Token::Satisfies => OwnedToken::Satisfies,
            Token::Fn => OwnedToken::Fn,
            Token::Impl => OwnedToken::Impl,
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
            Token::I8(i) => OwnedToken::I8(i),
            Token::I16(i) => OwnedToken::I16(i),
            Token::I32(i) => OwnedToken::I32(i),
            Token::I64(i) => OwnedToken::I64(i),
            Token::U8(i) => OwnedToken::U8(i),
            Token::U16(i) => OwnedToken::U16(i),
            Token::U32(i) => OwnedToken::U32(i),
            Token::U64(i) => OwnedToken::U64(i),
            Token::F32(fl) => OwnedToken::F32(fl),
            Token::F64(fl) => OwnedToken::F64(fl),
            Token::Str(s) => OwnedToken::Str(s.to_string()),
            Token::Ident(ident) => OwnedToken::Ident(ident.to_string()),
            Token::Op(op) => OwnedToken::Op(op.to_string()),
            Token::Colon => OwnedToken::Colon,
            Token::Comma => OwnedToken::Comma,
            Token::Arrow => OwnedToken::Arrow,
            Token::LParen => OwnedToken::LParen,
            Token::RParen => OwnedToken::RParen,
            Token::LBrace => OwnedToken::LBrace,
            Token::RBrace => OwnedToken::RBrace,
            Token::LBracket => OwnedToken::LBracket,
            Token::RBracket => OwnedToken::RBracket,
            Token::Semicolon => OwnedToken::Semicolon,
            Token::Hash => OwnedToken::Hash,
            Token::Comment(c) => OwnedToken::Comment(c.to_string()),
        }
    }
}
