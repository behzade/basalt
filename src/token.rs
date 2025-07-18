use chumsky::span::SimpleSpan;
use std::fmt;

/// A `Span` represents a range in the source code, used for error reporting.
pub type Span = SimpleSpan;

/// The `Token` enum represents all possible tokens in the language.
#[derive(Clone, Debug, PartialEq)]
pub enum Token<'src> {
    // Literals
    I64(i64),
    F64(f64),
    Bool(bool),
    Str(&'src str),
    Identifier(&'src str),

    // Keywords
    Let,
    Mut,
    Struct,
    Enum,
    Trait,
    Impl,
    For,
    Fn,
    Return,
    If,
    Else,
    Match,
    Effect,
    Handler,
    Perform,
    Handle,
    With,
    Extern,
    Import,
    As,

    // Operators
    Not,
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    EqualEqual,
    NotEqual,
    LessThan,
    GreaterThan,
    LessThanOrEqual,
    GreaterThanOrEqual,

    // Delimiters & Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Comma,
    Colon,
    DoubleColon,
    Semicolon,
    Dot,
    Arrow,    // ->
    FatArrow, // =>
    Scope,    // ::
    Pipe,     // |
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Token::I64(n) => write!(f, "{}", n),
            Token::F64(n) => write!(f, "{}", n),
            Token::Bool(b) => write!(f, "{}", b),
            Token::Str(s) => write!(f, "\"{}\"", s),
            Token::Identifier(s) => write!(f, "{}", s),
            Token::Let => write!(f, "let"),
            Token::Mut => write!(f, "mut"),
            Token::Struct => write!(f, "struct"),
            Token::Enum => write!(f, "enum"),
            Token::Trait => write!(f, "trait"),
            Token::Impl => write!(f, "impl"),
            Token::For => write!(f, "for"),
            Token::Fn => write!(f, "fn"),
            Token::Return => write!(f, "return"),
            Token::If => write!(f, "if"),
            Token::Else => write!(f, "else"),
            Token::Match => write!(f, "match"),
            Token::Effect => write!(f, "effect"),
            Token::Handler => write!(f, "handler"),
            Token::Perform => write!(f, "perform"),
            Token::Handle => write!(f, "handle"),
            Token::With => write!(f, "with"),
            Token::Extern => write!(f, "extern"),
            Token::Import => write!(f, "import"),
            Token::As => write!(f, "as"),
            Token::Not => write!(f, "!"),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Equal => write!(f, "="),
            Token::EqualEqual => write!(f, "=="),
            Token::NotEqual => write!(f, "!="),
            Token::LessThan => write!(f, "<"),
            Token::GreaterThan => write!(f, ">"),
            Token::LessThanOrEqual => write!(f, "<="),
            Token::GreaterThanOrEqual => write!(f, ">="),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::Comma => write!(f, ","),
            Token::Colon => write!(f, ":"),
            Token::DoubleColon => write!(f, "::"),
            Token::Semicolon => write!(f, ";"),
            Token::Dot => write!(f, "."),
            Token::Arrow => write!(f, "->"),
            Token::FatArrow => write!(f, "=>"),
            Token::Scope => write!(f, "::"),
            Token::Pipe => write!(f, "|"),
        }
    }
}
