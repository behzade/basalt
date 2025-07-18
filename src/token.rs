// token.rs

/// Represents a single token in the source code.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Special tokens
    Illegal, // Represents a token that is not recognized
    Eof,     // End of File

    // --- Literals ---
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),

    // --- Identifier ---
    Ident(String),

    // --- Keywords ---
    Let,      // let
    Mut,      // mut
    Struct,   // struct
    Trait,    // trait
    Impl,     // impl
    For,      // for
    While,    // while
    If,       // if
    Else,     // else
    Fn,       // fn
    Return,   // return
    Effect,   // effect
    Handler,  // handler
    Handle,   // handle
    With,     // with
    Perform,  // perform
    Extern,   // extern
    Enum,     // enum
    Match,    // match
    True,     // true
    False,    // false
    None,     // none (for effect return type)

    // --- Operators ---
    Assign,         // =
    Plus,           // +
    Minus,          // -
    Star,           // *
    Slash,          // /
    Percent,        // %
    Power,          // **
    
    // Logical Operators
    And,            // &&
    Or,             // ||
    Not,            // !

    // Comparison Operators
    Eq,             // ==
    NotEq,          // !=
    Lt,             // <
    LtEq,           // <=
    Gt,             // >
    GtEq,           // >=

    // --- Punctuation ---
    Comma,          // ,
    Semicolon,      // ;
    Colon,          // :
    Dot,            // .
    LParen,         // (
    RParen,         // )
    LBrace,         // {
    RBrace,         // }
    LBracket,       // [
    RBracket,       // ]
    Arrow,          // ->
    DoubleColon,    // ::
}

/// Looks up an identifier to see if it is a keyword.
pub fn lookup_ident(ident: &str) -> Token {
    match ident {
        "let" => Token::Let,
        "mut" => Token::Mut,
        "struct" => Token::Struct,
        "trait" => Token::Trait,
        "impl" => Token::Impl,
        "for" => Token::For,
        "while" => Token::While,
        "if" => Token::If,
        "else" => Token::Else,
        "fn" => Token::Fn,
        "return" => Token::Return,
        "effect" => Token::Effect,
        "handler" => Token::Handler,
        "handle" => Token::Handle,
        "with" => Token::With,
        "perform" => Token::Perform,
        "extern" => Token::Extern,
        "enum" => Token::Enum,
        "match" => Token::Match,
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        "none" => Token::None,
        _ => Token::Ident(ident.to_string()),
    }
}

