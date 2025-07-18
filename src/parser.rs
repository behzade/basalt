use crate::ast::*;
use crate::token::{Span, Token}; // Assuming Span is a type alias for SimpleSpan
use chumsky::Parser;
use chumsky::input::{IterInput, ValueInput}; // Import the correct input type
use chumsky::prelude::*;
use std::collections::HashMap;

// --- START OF CHANGES ---

// Define concrete iterator types to avoid "impl Trait in return position" issues.
// This assumes you are parsing a Vec<(Token, Span)> by cloning an iterator over it.
// This requires your Token enum to be Clone.
type TokenSliceIter<'a> = std::slice::Iter<'a, (Token<'a>, Span)>;
type TokenIter<'a> = std::iter::Cloned<TokenSliceIter<'a>>;

// The main fix: Redefine the parser's input to be IterInput, which correctly
// separates tokens from spans for the parser.
type ParserInput<'a> = IterInput<TokenIter<'a>, Span>;

// The error type remains the same.
pub type ParseError<'a> = extra::Err<Rich<'a, Token<'a>, Span>>;

// --- END OF CHANGES ---

fn expr_parser<'tokens, 'src: 'tokens, I>(
) -> impl Parser<'tokens, I, Spanned<Expr<'src>>, extra::Err<Rich<'tokens, Token<'src>, Span>>> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    recursive(|expr| {
        let inline_expr = recursive(|inline_expr| {
            let val = select! {
                Token::Bool(x) => Literal::Bool(x),
                Token::I64(n) => Literal::I64(n),
                Token::F64(n) => Literal::F64(n),
                Token::Str(s) => Literal::Str(s),
            }
            .labelled("value");

            let ident = select! { 
                Token::Identifier(x) => Path::Identifier(x),
            };

            // A list of expressions
            let items = expr
                .clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>();

            // A let expression
            let let_ = just(Token::Let)
                .ignore_then(ident)
                .then_ignore(just(Token::Equal))
                .then(inline_expr)
                .then_ignore(just(Token::Semicolon))
                .then(expr.clone())
                .map(|((name, val), body)| Expr::Let(VariableDeclaration { mutable: false, name: name, value: value, type_annotation: todo!() }));
    }
        )})
}

fn funcs_parser<'tokens, 'src: 'tokens, I>() -> impl Parser<
    'tokens,
    I,
    HashMap<&'src str, Func<'src>>,
    extra::Err<Rich<'tokens, Token<'src>, Span>>,
> + Clone
where
    I: ValueInput<'tokens, Token = Token<'src>, Span = Span>,
{
    let ident = select! { Token::Ident(ident) => ident };

    // Argument lists are just identifiers separated by commas, surrounded by parentheses
    let args = ident
        .separated_by(just(Token::Ctrl(',')))
        .allow_trailing()
        .collect()
        .delimited_by(just(Token::Ctrl('(')), just(Token::Ctrl(')')))
        .labelled("function args");

    let func = just(Token::Fn)
        .ignore_then(
            ident
                .map_with(|name, e| (name, e.span()))
                .labelled("function name"),
        )
        .then(args)
        .map_with(|start, e| (start, e.span()))
        .then(
            expr_parser()
                .delimited_by(just(Token::Ctrl('{')), just(Token::Ctrl('}')))
                // Attempt to recover anything that looks like a function body but contains errors
                .recover_with(via_parser(nested_delimiters(
                    Token::Ctrl('{'),
                    Token::Ctrl('}'),
                    [
                        (Token::Ctrl('('), Token::Ctrl(')')),
                        (Token::Ctrl('['), Token::Ctrl(']')),
                    ],
                    |span| (Expr::Error, span),
                ))),
        )
        .map(|(((name, args), span), body)| (name, Func { args, span, body }))
        .labelled("function");

    func.repeated()
        .collect::<Vec<_>>()
        .validate(|fs, _, emitter| {
            let mut funcs = HashMap::new();
            for ((name, name_span), f) in fs {
                if funcs.insert(name, f).is_some() {
                    emitter.emit(Rich::custom(
                        name_span,
                        format!("Function '{name}' already exists"),
                    ));
                }
            }
            funcs
        })
};
