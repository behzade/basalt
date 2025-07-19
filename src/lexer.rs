//! This module contains the lexer, which is responsible for turning a raw source string
//! into a stream of tokens that the parser can understand. It handles various language
//! constructs such as keywords, identifiers, literals, operators, and comments.

use chumsky::prelude::*;
use std::fmt;

use crate::token::Token;

impl<'src> fmt::Display for Token<'src> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
            Token::Bool(b) => write!(f, "{}", b),
            Token::I64(i) => write!(f, "{}", i),
            Token::F64(fl) => write!(f, "{}", fl),
            Token::Str(s) => write!(f, "\"{}\"", s),
            Token::Ident(ident) => write!(f, "{}", ident),
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
            Token::Comment(c) => write!(f, "//{}", c),
        }
    }
}


/// Creates a lexer that tokenizes the source code.
///
/// The lexer returns a vector of tokens, each associated with a span indicating its
/// location in the source file. It also includes robust error recovery.
pub fn lexer<'src>(
) -> impl Parser<'src, &'src str, Vec<(Token<'src>, SimpleSpan)>, extra::Err<Rich<'src, char>>> {
    // A parser for operators
    let op = one_of("+-*/<>=!")
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|s: &str| Token::Op(s.to_string()));

    // A parser for punctuation
    let punc = choice((
        just("::").to(Token::DoubleColon),
        just(":").to(Token::Colon),
        just(";").to(Token::Semi),
        just(",").to(Token::Comma),
        just("->").to(Token::Arrow),
        just("=>").to(Token::FatArrow),
        just("(").to(Token::LParen),
        just(")").to(Token::RParen),
        just("{").to(Token::LBrace),
        just("}").to(Token::RBrace),
        just("[").to(Token::LBracket),
        just("]").to(Token::RBracket),
    ));

    // A parser for literals
    let literal = {
        // Parser for fractional numbers, e.g., 3.14
        let frac = just('.').then(text::digits(10));
        
        // Parser for numbers, handling both integers and floats
        let number = text::int(10)
            .then(frac.or_not())
            .to_slice()
            .map(|s: &str| {
                if s.contains('.') {
                    Token::F64(s.parse().unwrap())
                } else {
                    Token::I64(s.parse().unwrap())
                }
            });

        // Parser for strings, handling escaped characters
        let string = just('"')
            .ignore_then(filter(|c| *c != '"').repeated().to_slice())
            .then_ignore(just('"'))
            .map(Token::Str);

        choice((number, string))
    };

    // A parser for identifiers and keywords
    let ident = text::ident().map(|ident: &str| match ident {
        "let" => Token::Let,
        "mut" => Token::Mut,
        "struct" => Token::Struct,
        "enum" => Token::Enum,
        "trait" => Token::Trait,
        "impl" => Token::Impl,
        "for" => Token::For,
        "fn" => Token::Fn,
        "extern" => Token::Extern,
        "import" => Token::Import,
        "as" => Token::As,
        "while" => Token::While,
        "return" => Token::Return,
        "effect" => Token::Effect,
        "handler" => Token::Handler,
        "perform" => Token::Perform,
        "handle" => Token::Handle,
        "with" => Token::With,
        "match" => Token::Match,
        "if" => Token::If,
        "else" => Token::Else,
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        _ => Token::Ident(ident),
    });

    // A parser for single-line comments
    let comment = just("//")
        .ignore_then(any().and_is(just('\n').not()).repeated().to_slice())
        .map(Token::Comment);

    // A single token parser, combining all the smaller parsers
    let token = literal
        .or(ident)
        .or(punc)
        .or(op)
        .or(comment)
        // We use `recover_with` to handle errors gracefully.
        // `skip_then_retry_until` is a good strategy for lexers, as it
        // skips unrecognized characters until it finds something that looks
        // like the start of a new token.
        .recover_with(skip_then_retry_until(any().ignored(), end()));

    // The final lexer: applies the token parser repeatedly, separated by whitespace,
    // and collects the results into a vector.
    token
        .map_with_span(|tok, span| (tok, span))
        .padded()
        .repeated()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer_keywords_and_identifiers() {
        let input = "let mut x = fn_name;";
        let (tokens, errs) = lexer().parse(input).into_output_errors();
        assert!(errs.is_empty(), "Lexer produced errors: {:?}", errs);
        let tokens: Vec<Token> = tokens.unwrap().into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Mut,
                Token::Ident("x"),
                Token::Op("=".to_string()),
                Token::Ident("fn_name"),
                Token::Semi,
            ]
        );
    }

    #[test]
    fn test_lexer_literals() {
        let input = r#"42 3.14 true "hello world""#;
        let (tokens, errs) = lexer().parse(input).into_output_errors();
        assert!(errs.is_empty(), "Lexer produced errors: {:?}", errs);
        let tokens: Vec<Token> = tokens.unwrap().into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            tokens,
            vec![
                Token::I64(42),
                Token::F64(3.14),
                Token::Bool(true),
                Token::Str("hello world"),
            ]
        );
    }

    #[test]
    fn test_lexer_operators_and_punctuation() {
        let input = "-> => :: { } ( ) [ ] + - * / = == ; ,";
        let (tokens, errs) = lexer().parse(input).into_output_errors();
        assert!(errs.is_empty(), "Lexer produced errors: {:?}", errs);
        let tokens: Vec<Token> = tokens.unwrap().into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Arrow,
                Token::FatArrow,
                Token::DoubleColon,
                Token::LBrace,
                Token::RBrace,
                Token::LParen,
                Token::RParen,
                Token::LBracket,
                Token::RBracket,
                Token::Op("+".to_string()),
                Token::Op("-".to_string()),
                Token::Op("*".to_string()),
                Token::Op("/".to_string()),
                Token::Op("=".to_string()),
                Token::Op("==".to_string()),
                Token::Semi,
                Token::Comma,
            ]
        );
    }

    #[test]
    fn test_lexer_with_comments() {
        let input = r#"
            let x = 5; // This is a comment
            // Another comment
            let y = 10;
        "#;
        let (tokens, errs) = lexer().parse(input).into_output_errors();
        assert!(errs.is_empty(), "Lexer produced errors: {:?}", errs);
        let tokens: Vec<Token> = tokens.unwrap().into_iter().map(|(t, _)| t).collect();
        assert_eq!(
            tokens,
            vec![
                Token::Let,
                Token::Ident("x"),
                Token::Op("=".to_string()),
                Token::I64(5),
                Token::Semi,
                Token::Comment(" This is a comment"),
                Token::Comment(" Another comment"),
                Token::Let,
                Token::Ident("y"),
                Token::Op("=".to_string()),
                Token::I64(10),
                Token::Semi,
            ]
        );
    }
}
