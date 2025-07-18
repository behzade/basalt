use crate::ast::*;
use crate::token::{Token, Span};
use chumsky::prelude::*;
use chumsky::Parser;
use std::collections::HashMap;

// A type alias for the input stream: a slice of (Token, Span) tuples.
type ParserInput<'a> = &'a [(Token<'a>, Span)];
// The custom error type for the parser.
pub type ParseError<'a> = extra::Err<Rich<'a, Token<'a>, Span>>;

/// The main parser function that handles all items and constructs the Program AST.
pub fn program_parser<'a>() -> impl Parser<'a, ParserInput<'a>, Program<'a>, ParseError<'a>> {
    // Forward declarations for mutually recursive parsers.
    let mut expr = Recursive::declare();
    let mut item = Recursive::declare();

    // A parser for an identifier string.
    let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

    // A parser for a path, e.g., `my_mod::my_func`.
    let path = ident.clone()
        .map(Path::Identifier)
        .map_with(|ident, e| Spanned(ident, e.span()))
        .foldl(
            select! { Token::DoubleColon => () }
                .ignore_then(ident.clone().map_with(|ident, e| Spanned(ident, e.span())))
                .repeated(),
            |(acc_path, acc_span), (segment, seg_span)| {
                let span = acc_span.start..seg_span.end;
                let new_path = Path::Namespaced(Box::new((acc_path, acc_span)), segment);
                (new_path, span)
            },
        )
        .labelled("path");

    // A helper for parsing comma-separated, optionally-trailed lists.
    let comma_separated = |p| p.separated_by(select! { Token::Comma => () }).allow_trailing().collect();

    // A parser for a type annotation.
    let type_parser = recursive(|type_p| {
        let generic_args = comma_separated(type_p.clone())
            .delimited_by(select! { Token::LessThan => () }, select! { Token::GreaterThan => () });

        let simple_type = select! {
            Token::Identifier("i64") => Type::I64,
            Token::Identifier("f64") => Type::F64,
            Token::Identifier("bool") => Type::Bool,
            Token::Identifier("string") => Type::StringType,
            Token::Identifier("none") => Type::None,
        };
        
        let custom_or_generic = path.clone()
            .then(generic_args.or_not())
            .map(|((p, _), generics)| {
                match (p, generics) {
                    (Path::Identifier("Array"), Some(mut args)) if args.len() == 1 => Type::Array(Box::new(args.pop().unwrap())),
                    (Path::Identifier("Map"), Some(mut args)) if args.len() == 2 => {
                        let val = args.pop().unwrap();
                        let key = args.pop().unwrap();
                        Type::Map(Box::new(key), Box::new(val))
                    }
                    (p, _) => Type::Custom(p),
                }
            });

        simple_type.or(custom_or_generic).map_with(|ident, e| Spanned(ident, e.span()))
    }).labelled("type");

    // Parser for a block of statements and an optional final expression.
    let block = {
        let stmt = item.clone().map(Stmt::Item)
            .or(expr.clone().then_ignore(select!{ Token::Semicolon => () }).map(Stmt::Expr))
            .map_with(|ident, e| Spanned(ident, e.span()));

        stmt.repeated().collect::<Vec<_>>()
            .then(expr.clone().or_not())
            .delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () })
            .map(|(stmts, last_expr)| Expr::Block {
                stmts,
                last_expr: last_expr.map(Box::new),
            })
            .labelled("block")
    };
    
    // An atom is the highest-precedence expression unit.
    let atom = {
        let literal = select! {
            Token::I64(i) => Literal::I64(i),
            Token::F64(f) => Literal::F64(f),
            Token::Bool(b) => Literal::Bool(b),
            Token::Str(s) => Literal::String(s),
        }.labelled("literal");

        let array_literal = comma_separated(expr.clone())
            .delimited_by(select!{ Token::LBracket => () }, select!{ Token::RBracket => () })
            .map(Literal::Array);

        let map_field = expr.clone().then_ignore(select!{ Token::Colon => () }).then(expr.clone());
        let map_literal = comma_separated(map_field)
            .delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () })
            .map(Literal::Map);

        let struct_field = ident.clone().then_ignore(select!{ Token::Colon => () }).then(expr.clone());
        let struct_inst = path.clone()
            .then(comma_separated(struct_field)
                .delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () })
            )
            .map(|(name, fields)| Expr::StructInstantiation(StructInstantiation {
                name, generic_args: None, fields: fields.into_iter().collect::<HashMap<_, _>>(),
            }));
            
        choice((
            literal.map(Expr::Literal),
            array_literal.map(Expr::Literal),
            struct_inst,
            map_literal.map(Expr::Literal), // Must come after struct instantiation to disambiguate `{}`
            path.clone().map(|(p, _)| Expr::Path(p)),
            expr.clone().delimited_by(select!{ Token::LParen => () }, select!{ Token::RParen => () }),
            block,
        )).map_with(|ident, e| Spanned(ident, e.span()))
    };

    // Postfix operators (function calls, indexing, member access).
    let postfix = atom.clone().foldl(
        choice((
            comma_separated(expr.clone())
                .delimited_by(select!{ Token::LParen => () }, select!{ Token::RParen => () })
                .map_with(|ident, e| Spanned(ident, e.span()))
                .map(|(args, args_span)| move |(callee, callee_span): Spanned<Expr<'a>>| {
                    (Expr::FunctionCall(FunctionCall { callee: Box::new((callee, callee_span)), generic_args: None, args }), callee_span.start..args_span.end)
                }),
            expr.clone()
                .delimited_by(select!{ Token::LBracket => () }, select!{ Token::RBracket => () })
                .map_with(|ident, e| Spanned(ident, e.span()))
                .map(|(index, index_span)| move |(left, left_span): Spanned<Expr<'a>>| {
                    (Expr::Index { left: Box::new((left, left_span)), index: Box::new(index) }, left_span.start..index_span.end)
                }),
            select!{ Token::Dot => () }.ignore_then(ident.clone())
                .map_with(|ident, e| Spanned(ident, e.span()))
                .map(|(member, member_span)| move |(object, object_span): Spanned<Expr<'a>>| {
                    (Expr::MemberAccess(MemberAccess { object: Box::new((object, object_span)), member }), object_span.start..member_span.end)
                }),
        )).repeated(),
        |(callee, callee_span), op_func| op_func((callee, callee_span)),
    );

    let unary = select!{ Token::Minus => UnaryOp::Neg, Token::Not => UnaryOp::Not }
        .map_with(|ident, e| Spanned(ident, e.span()))
        .repeated()
        .then(postfix)
        .foldr(|(op, op_span), (right_expr, right_span)| {
            (Expr::Unary { op, right: Box::new((right_expr, right_span)) }, op_span.start..right_span.end)
        });

    let binary = |op, next| next.clone().foldl(
        op.then(next).repeated(),
        |(l, ls), (op, (r, rs))| (Expr::Binary(BinaryOperation { left: Box::new((l, ls)), op, right: Box::new((r, rs)) }), ls.start..rs.end)
    );

    let product = binary(select!{ Token::Star => BinaryOp::Mul, Token::Slash => BinaryOp::Div }, unary.clone());
    let sum = binary(select!{ Token::Plus => BinaryOp::Add, Token::Minus => BinaryOp::Sub }, product.clone());
    let comparison = binary(select!{
        Token::EqualEqual => BinaryOp::Eq, Token::NotEqual => BinaryOp::Neq,
        Token::LessThan => BinaryOp::Lt, Token::LessThanOrEqual => BinaryOp::Lte,
        Token::GreaterThan => BinaryOp::Gt, Token::GreaterThanOrEqual => BinaryOp::Gte,
    }, sum.clone());

    expr.define(comparison);

    // Item parsers
    let fn_sig = {
        let param = ident.clone().then_ignore(select!{ Token::Colon => () }).then(type_parser.clone());
        comma_separated(param).delimited_by(select!{ Token::LParen => () }, select!{ Token::RParen => () })
            .then_ignore(select!{ Token::Arrow => () })
            .then(type_parser.clone())
            .then(select!{ Token::Slash => () }.ignore_then(comma_separated(path.clone()).delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () })).or_not())
            .map(|((params, return_type), effects)| FunctionSignature { params, return_type, effects })
    };

    let fn_def = ident.clone()
        .then(fn_sig.clone())
        .then(atom.clone().delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () }).or_not())
        .map(|((name, signature), body)| FunctionDef { name, is_extern: false, signature, body });

    let item_def = choice((
        select!{ Token::Struct => () }.ignore_then(ident.clone())
            .then(comma_separated(ident.clone().then_ignore(select!{ Token::Colon => () }).then(type_parser.clone()))
                .delimited_by(select!{ Token::LBrace => () }, select!{ Token::RBrace => () }))
            .map(|(name, fields)| Item::Struct(StructDef { name, generic_params: None, fields })),
        select!{ Token::Fn => () }.ignore_then(fn_def.clone()).map(Item::Function),
        select!{ Token::Extern => () }.ignore_then(select!{ Token::Fn => () }).ignore_then(
            ident.clone().then(fn_sig.clone()).then_ignore(select!{ Token::Semicolon => () })
        ).map(|(name, signature)| Item::Function(FunctionDef { name, is_extern: true, signature, body: None })),
    ));
    
    item.define(item_def);

    item.map_with(|ident, e| Spanned(ident, e.span())).repeated().collect().map(|items| Program { items }).then_ignore(end())
}
