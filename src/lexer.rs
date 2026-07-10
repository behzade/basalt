//! This module contains the lexer, which is responsible for turning a raw source string
//! into a stream of tokens that the parser can understand. It handles various language
//! constructs such as keywords, identifiers, literals, operators, and comments.

use chumsky::prelude::*;

use crate::token::Token;

/// Creates a lexer that tokenizes the source code.
///
/// The lexer returns a vector of tokens, each associated with a span indicating its
/// location in the source file. It also includes robust error recovery.
pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<(Token<'src>, SimpleSpan)>, extra::Err<Rich<'src, char>>> {
    // A parser for operators
    let op = choice((
        just("==").to(Token::Op("==".to_string())),
        just("!=").to(Token::Op("!=".to_string())),
        just(">=").to(Token::Op(">=".to_string())),
        just("<=").to(Token::Op("<=".to_string())),
        just("|").to(Token::Op("|".to_string())),
        just("...").to(Token::Op("...".to_string())),
        just("->").to(Token::Arrow),
        one_of("+-*/<>=!")
            .repeated()
            .exactly(1)
            .to_slice()
            .map(|s: &str| Token::Op(s.to_string())),
    ));

    // A parser for punctuation
    let punc = choice((
        just(":").to(Token::Colon),
        just(",").to(Token::Comma),
        just(";").to(Token::Semicolon),
        just("(").to(Token::LParen),
        just(")").to(Token::RParen),
        just("{").to(Token::LBrace),
        just("}").to(Token::RBrace),
        just("[").to(Token::LBracket),
        just("]").to(Token::RBracket),
        just(".").to(Token::Op(".".to_string())),
        just("#").to(Token::Hash),
    ));

    // A parser for literals
    let literal = {
        // Parser for fractional numbers, e.g., 3.14
        let frac = just('.').then(text::digits(10));
        let numeric_suffix = choice((
            just("i8"),
            just("i16"),
            just("i32"),
            just("i64"),
            just("u8"),
            just("u16"),
            just("u32"),
            just("u64"),
            just("f32"),
            just("f64"),
        ));

        // Parser for numbers, handling both integers and floats
        let number = text::int(10)
            .then(frac.or_not())
            .then(numeric_suffix.or_not())
            .to_slice()
            .map(|s: &str| {
                if let Some(raw) = s.strip_suffix("i8") {
                    return Token::I8(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("i16") {
                    return Token::I16(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("i32") {
                    return Token::I32(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("i64") {
                    return Token::I64(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("u8") {
                    return Token::U8(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("u16") {
                    return Token::U16(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("u32") {
                    return Token::U32(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("u64") {
                    return Token::U64(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("f32") {
                    return Token::F32(raw.parse().unwrap());
                }
                if let Some(raw) = s.strip_suffix("f64") {
                    return Token::F64(raw.parse().unwrap());
                }
                if s.contains('.') {
                    Token::F64(s.parse().unwrap())
                } else if let Ok(value) = s.parse::<i32>() {
                    Token::I32(value)
                } else {
                    Token::I64(s.parse().unwrap())
                }
            });

        // Parser for strings, correctly handling escaped characters
        let string = just('"')
            .ignore_then(
                // CORRECTED: Use `filter` directly instead of `any().filter(...)`
                none_of("\"\\")
                    .or(just('\\').ignore_then(any()))
                    .repeated()
                    .to_slice(),
            )
            .then_ignore(just('"'))
            .map(Token::Str);

        choice((number, string))
    };

    // A parser for identifiers and keywords
    let ident = text::ident().map(|ident: &str| match ident {
        "let" => Token::Let,
        "mut" => Token::Mut,
        "memory" => Token::Memory,
        "reset" => Token::Reset,
        "type" => Token::Type,
        "struct" => Token::Struct,
        "extern" => Token::Extern,
        "unsafe" => Token::Unsafe,
        "fn" => Token::Fn,
        "import" => Token::Import,
        "as" => Token::As,
        "while" => Token::While,
        "return" => Token::Return,
        "effect" => Token::Effect,
        "handler" => Token::Handler,
        "perform" => Token::Perform,
        "handle" => Token::Handle,
        "with" => Token::With,
        "effects" => Token::Effects,
        "match" => Token::Match,
        "if" => Token::If,
        "else" => Token::Else,
        "pub" => Token::Pub,
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        _ => Token::Ident(ident),
    });

    // A parser for single-line comments
    let comment = just("//")
        .ignore_then(any().and_is(just('\n').not()).repeated().to_slice())
        .map(Token::Comment);

    // A single token parser, combining all the smaller parsers
    let token = comment
        .or(literal)
        .or(ident)
        .or(op) // Operators must come before punctuation to handle "..." correctly
        .or(punc)
        // We use `recover_with` and `skip_then_retry_until` to handle errors gracefully.
        // This strategy skips characters one by one until it finds a character that can
        // be considered the start of a new token (here, whitespace or the end of input),
        // and then retries the main parser.
        .recover_with(skip_then_retry_until(
            any().ignored(),
            text::whitespace().or(end()).ignored(),
        ));

    // The final lexer: applies the token parser repeatedly, separated by whitespace,
    // and collects the results into a vector.
    token
        .map_with(|tok, e| (tok, e.span()))
        .padded()
        .repeated()
        .collect()
}
