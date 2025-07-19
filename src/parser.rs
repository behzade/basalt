//! This module contains the parser, which is responsible for turning a stream of tokens
//! into a structured Abstract Syntax Tree (AST). It uses a combination of recursive
//! descent and Pratt parsing to handle the language's syntax, including expressions,
//! statements, and top-level items.

use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::select;

use crate::ast::*;
use crate::token::Token;

// --- Forward-declared parsers for expressions and statements ---

fn expression_parsers<'src>() -> (
    impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
    impl Parser<'src, &'src [Token<'src>], Stmt<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
) {
    let mut expr = Recursive::declare();
    let mut stmt = Recursive::declare();

    let ident = select! { Token::Ident(ident) => ident }.labelled("identifier");

    let path = ident
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>()
        .labelled("path");

    let ty = recursive(|type_p| {
        path.clone()
            .then(
                type_p
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(
                        select! { Token::Op(op) if op == "<" => () },
                        select! { Token::Op(op) if op == ">" => () },
                    )
                    .or_not(),
            )
            .map(|(path, generics)| Type {
                path,
                generics: generics.unwrap_or_default(),
            })
    })
    .labelled("type")
    .boxed();

    let block = stmt
        .clone()
        .repeated()
        .collect::<Vec<_>>()
        .then(expr.clone().or_not())
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|(stmts, last_expr)| Expr::Block {
            stmts,
            last_expr: last_expr.map(Box::new),
        })
        .labelled("block")
        .boxed();

    let atom = choice((
        // Literals
        select! { Token::I64(n) => Expr::Literal(Literal::I64(n)) },
        select! { Token::F64(n) => Expr::Literal(Literal::F64(n)) },
        select! { Token::Bool(b) => Expr::Literal(Literal::Bool(b)) },
        select! { Token::Str(s) => Expr::Literal(Literal::Str(s)) },
        // Variable/path
        path.clone().map(Expr::Path),
        // Grouped expression
        expr.clone()
            .delimited_by(just(Token::LParen), just(Token::RParen)),
        // Block
        block.clone(),
    ))
    .boxed();

    let call_args = expr
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>();

    let expr_parser = atom
        .pratt((
            postfix(
                8,
                call_args.delimited_by(just(Token::LParen), just(Token::RParen)),
                |lhs, args, _extra| Expr::Call {
                    fun: Box::new(lhs),
                    args,
                },
            ),
            prefix(
                7,
                select! { Token::Op(op) if op == "-" => () },
                |_op, rhs, _extra| Expr::Unary {
                    op: UnaryOp::Neg,
                    rhs: Box::new(rhs),
                },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "*" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Mul,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "/" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Div,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "+" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Add,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "-" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Sub,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == "<" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Lt,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == ">" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Gt,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
            infix(
                left(3),
                select! { Token::Op(op) if op == "==" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Eq,
                    lhs: Box::new(l),
                    rhs: Box::new(r),
                },
            ),
        ))
        .labelled("expression");

    expr.define(expr_parser);

    let let_decl = just(Token::Let)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident)
        .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(expr.clone())
        .then_ignore(just(Token::Semi))
        .map(|(((is_mut, name), opt_type), value)| Stmt::Let {
            is_mut: is_mut.is_some(),
            name,
            ty: opt_type,
            value,
        });

    let stmt_parser = choice((
        let_decl,
        just(Token::Return)
            .ignore_then(expr.clone().or_not())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Return),
        expr.clone().then_ignore(just(Token::Semi)).map(Stmt::Expr),
    ))
    .labelled("statement");

    stmt.define(stmt_parser);

    (expr, stmt)
}

fn fn_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, _) = expression_parsers();

    let ident = select! { Token::Ident(ident) => ident };

    let path = ident
        .clone()
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>();

    let ty = recursive(|type_p| {
        path.clone()
            .then(
                type_p
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(
                        select! { Token::Op(op) if op == "<" => () },
                        select! { Token::Op(op) if op == ">" => () },
                    )
                    .or_not(),
            )
            .map(|(path, generics)| Type {
                path,
                generics: generics.unwrap_or_default(),
            })
    })
    .boxed();

    let params = ident
        .clone()
        .then_ignore(just(Token::Colon))
        .then(ty.clone())
        .map(|(name, ty)| (Some(name), ty))
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    let block = expr
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .recover_with(via_parser(empty().to(Expr::Error))); // Basic block recovery

    just(Token::Fn)
        .ignore_then(ident)
        .then(params)
        .then(just(Token::Arrow).ignore_then(ty).or_not())
        .then(block)
        .map(|(((name, params), ret_type), body)| Function {
            name,
            params,
            ret_type,
            effects: Vec::new(), // Simplified for now
            body,
        })
        .labelled("function declaration")
}

/// The main parser function for the entire language.
pub fn file_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Vec<Item<'src>>, extra::Err<Rich<'src, Token<'src>>>> {
    let comment = select! { Token::Comment(_) => () }.repeated();

    let item = item_parser().padded_by(comment);

    item.repeated().collect::<Vec<_>>().then_ignore(end())
}

/// Parses a single top-level item.
fn item_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let fn_decl = fn_parser().map(Item::Fn);

    choice((fn_decl,)) // Add other item parsers here
        .recover_with(skip_then_retry_until(
            any().ignored(),
            one_of([Token::Fn /* other item keywords */]).ignored(),
        ))
}
