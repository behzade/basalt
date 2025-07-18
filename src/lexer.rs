use crate::token::{Span, Token};
use chumsky::prelude::*;

pub fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<(Token<'src>, Span)>, extra::Err<Rich<'src, char>>> {
    // A parser for floating-point numbers, ensuring it's tried before integers.
    let float = text::int(10)
        .then(just('.').then(text::digits(10)))
        .to_slice()
        .from_str()
        .unwrapped()
        .map(Token::F64);

    // A parser for integers.
    let int = text::int(10)
        .to_slice()
        .from_str()
        .unwrapped()
        .map(Token::I64);

    // A parser for string literals.
    let string = just('"')
        .ignore_then(none_of('"').repeated().to_slice())
        .then_ignore(just('"'))
        .map(Token::Str);

    // A parser for operators. Multi-character operators are listed first to ensure
    // they are preferred over single-character ones (e.g., `==` over `=`).
    let op = choice((
        just("->").to(Token::Arrow),
        just("=>").to(Token::FatArrow),
        just("::").to(Token::DoubleColon),
        just("==").to(Token::EqualEqual),
        just("!=").to(Token::NotEqual),
        just("<=").to(Token::LessThanOrEqual),
        just(">=").to(Token::GreaterThanOrEqual),
        just('!').to(Token::Not),
        just('+').to(Token::Plus),
        just('-').to(Token::Minus),
        just('*').to(Token::Star),
        just('/').to(Token::Slash),
        just('=').to(Token::Equal),
        just('<').to(Token::LessThan),
        just('>').to(Token::GreaterThan),
        just('|').to(Token::Pipe),
        just('.').to(Token::Dot),
    ));

    // A parser for punctuation.
    let punc = choice((
        just('(').to(Token::LParen),
        just(')').to(Token::RParen),
        just('{').to(Token::LBrace),
        just('}').to(Token::RBrace),
        just('[').to(Token::LBracket),
        just(']').to(Token::RBracket),
        just(',').to(Token::Comma),
        just(':').to(Token::Colon),
        just(';').to(Token::Semicolon),
    ));

    // A parser for identifiers and keywords.
    let ident = text::ident().map(|s: &str| match s {
        "let" => Token::Let,
        "mut" => Token::Mut,
        "struct" => Token::Struct,
        "enum" => Token::Enum,
        "trait" => Token::Trait,
        "impl" => Token::Impl,
        "for" => Token::For,
        "fn" => Token::Fn,
        "return" => Token::Return,
        "if" => Token::If,
        "else" => Token::Else,
        "match" => Token::Match,
        "effect" => Token::Effect,
        "handler" => Token::Handler,
        "perform" => Token::Perform,
        "handle" => Token::Handle,
        "with" => Token::With,
        "extern" => Token::Extern,
        "import" => Token::Import,
        "as" => Token::As,
        "true" => Token::Bool(true),
        "false" => Token::Bool(false),
        s => Token::Identifier(s),
    });

    // A single token is one of the above, in order of precedence.
    let token = float.or(int).or(string).or(op).or(punc).or(ident);

    // A parser for single-line comments.
    let comment = just("//")
        .then(any().and_is(just('\n').not()).repeated())
        .padded();

    // The final lexer: applies the token parser repeatedly, separated by whitespace or comments,
    // and collects the output into a vector of (token, span) pairs.
    token
        .map_with(|tok, e| (tok, e.span()))
        .padded_by(comment.repeated())
        .padded()
        .repeated()
        .collect()
}
