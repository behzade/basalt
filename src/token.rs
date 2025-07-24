use std::fmt;

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
    Impl,
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
    Semi,        // ;
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
            Token::Impl => write!(f, "impl"),
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
            Token::Semi => write!(f, ";"),
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
