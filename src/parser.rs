//! Parser aligned with Tree-sitter grammar (with full spans on every node)

use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::select;

use crate::ast::*;
use crate::token::{SimpleSpan, Token};

fn trivia<'src>() -> impl Parser<'src, &'src [Token<'src>], (), extra::Err<Rich<'src, Token<'src>>>> {
    select! { Token::Comment(_) => (), Token::Semicolon => () }.repeated().ignored()
}

fn spanned<'src, T>(
    p: impl Parser<'src, &'src [Token<'src>], T, extra::Err<Rich<'src, Token<'src>>>>,
) -> impl Parser<'src, &'src [Token<'src>], Spanned<T>, extra::Err<Rich<'src, Token<'src>>>> {
    p.map_with(|node, e| Spanned { node, span: e.span() })
}

fn ident<'src>() -> impl Parser<'src, &'src [Token<'src>], &'src str, extra::Err<Rich<'src, Token<'src>>>> {
    select! { Token::Ident(s) => s }.labelled("identifier")
}

fn path<'src>() -> impl Parser<'src, &'src [Token<'src>], Path<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let slash = select! { Token::Op(op) if op == "/" => () };
    ident().separated_by(slash).at_least(1).collect::<Vec<_>>()
}

fn with_types<'src, F, P>(
    make_ty: F,
) -> impl Parser<'src, &'src [Token<'src>], Vec<Type<'src>>, extra::Err<Rich<'src, Token<'src>>>>
where
    F: Clone + Fn() -> P,
    P: Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>>,
{
    let list = make_ty()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));
    let single = make_ty().map(|t| vec![t]);
    just(Token::With)
        .ignore_then(choice((list, single)))
        .or_not()
        .map(|v| v.unwrap_or_default())
}

fn type_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|type_p| {
        let lt = select! { Token::Op(op) if op == "<" => () };
        let gt = select! { Token::Op(op) if op == ">" => () };

        let path_type = path()
            .then(lt.ignore_then(type_p.clone().separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>()).then_ignore(gt).or_not())
            .map(|(p, g)| TypeNode::Path { path: p, generics: g.unwrap_or_default() })
            .map_with(|node, e| Spanned { node, span: e.span() });

        let record_field = ident().then_ignore(just(Token::Colon)).then(type_p.clone());
        let record_type = record_field
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .map(TypeNode::Record)
            .map_with(|node, e| Spanned { node, span: e.span() });

        let never_type = select! { Token::Op(op) if op == "!" => () }
            .to(TypeNode::Never)
            .map_with(|node, e| Spanned { node, span: e.span() });

        let make_ty = {
            let type_p = type_p.clone();
            move || type_p.clone()
        };

        let fn_type = type_p
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LParen), just(Token::RParen))
            .then_ignore(just(Token::Arrow))
            .then(type_p.clone())
            .then(with_types(make_ty).boxed())
            .map(|((params, ret), effects)| TypeNode::Function { params, ret: Box::new(ret), effects })
            .map_with(|node, e| Spanned { node, span: e.span() });

        choice((fn_type, record_type, never_type, path_type)).boxed()
    })
    .boxed()
}

fn params_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Vec<(Option<&'src str>, Type<'src>)>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser().boxed();
    let named = ident().then_ignore(just(Token::Colon)).then(ty.clone()).map(|(n, t)| (Some(n), t));
    let self_param = just(Token::Mut)
        .or_not()
        .ignore_then(ident())
        .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
        .map(|(name, ty)| {
            (
                Some(name),
                ty.unwrap_or_else(|| Spanned { node: TypeNode::Path { path: vec![name], generics: vec![] }, span: SimpleSpan { context: (), start: 0, end: 0 } }),
            )
        });
    choice((named, self_param))
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
}

fn record_literal_expr<'src>(
    expr: impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    ident()
        .then_ignore(just(Token::Colon))
        .then(expr.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .at_least(1)
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map_with(|fields, e| Spanned { node: ExprNode::RecordLiteral { fields }, span: e.span() })
}

fn expression_bundle<'src>() -> (
    impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
    impl Parser<'src, &'src [Token<'src>], Stmt<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
    impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
) {
    let mut expr = Recursive::declare();
    let mut stmt = Recursive::declare();

    let block = just(Token::LBrace)
        .ignore_then(trivia())
        .ignore_then(
            stmt
                .clone()
                .padded_by(trivia())
                .repeated()
                .collect::<Vec<_>>()
        )
        .then_ignore(trivia())
        .then(expr.clone().padded_by(trivia()).or_not())
        .then_ignore(trivia())
        .then_ignore(just(Token::RBrace))
        .map_with(|(stmts, last_expr), e| Spanned { node: ExprNode::Block { stmts, last_expr: last_expr.map(Box::new) }, span: e.span() })
        .boxed();

    let unit = just(Token::LParen)
        .then_ignore(just(Token::RParen))
        .map_with(|_, e| Spanned { node: ExprNode::Literal(Literal::Unit), span: e.span() });

    let lit = choice((
        select! { Token::I64(n) => Literal::I64(n) },
        select! { Token::F64(n) => Literal::F64(n) },
        select! { Token::Bool(b) => Literal::Bool(b) },
        select! { Token::Str(s) => Literal::Str(s) },
    ))
    .map_with(|l, e| Spanned { node: ExprNode::Literal(l), span: e.span() });

    let perform = just(Token::Perform)
        .ignore_then(ident())
        .then_ignore(just(Token::Op(".".to_string())))
        .then(ident())
        .then(
            expr
                .clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .map_with(|(((e1, e2), args)), e| Spanned { node: ExprNode::Perform { path: vec![e1, e2], args }, span: e.span() });

    // Prefix-style with-block: with { Handler1, Handler2 } { ... }
    let handler_list = path()
        .padded_by(trivia())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    let with_block = just(Token::With)
        .ignore_then(handler_list)
        .then(block.clone())
        .map_with(|(handlers, body), e| {
            // Desugar multiple handlers into nested Handle nodes: leftmost is outermost
            let mut acc = body;
            for h in handlers.into_iter().rev() {
                acc = Spanned { node: ExprNode::Handle { body: Box::new(acc), handler: HandlerBody::Path(h) }, span: e.span() };
            }
            acc
        });

    // Struct init: Path '{' named_fields '}'
    let struct_init = path()
        .map_with(|p, e| (p, e.span()))
        .then(record_literal_expr(expr.clone()))
        .map(|((p, p_span), rec)| {
            let fields = match &rec.node {
                ExprNode::RecordLiteral { fields } => fields.clone(),
                _ => vec![],
            };
            let span = SimpleSpan { context: (), start: p_span.start, end: rec.span.end };
            Spanned { node: ExprNode::StructInit { path: p, generics: vec![], fields }, span }
        })
        .boxed();

    let atom = choice((unit, lit, perform, with_block, block.clone(), struct_init, record_literal_expr(expr.clone()),
        path().map_with(|p, e| Spanned { node: ExprNode::Path(p), span: e.span() }),
        expr.clone().delimited_by(just(Token::LParen), just(Token::RParen))
    ))
    .boxed();

    // if / while / match expressions
    let if_expr = just(Token::If)
        .ignore_then(expr.clone())
        .then(block.clone())
        .then(just(Token::Else).ignore_then(block.clone()).or_not())
        .map_with(|((cond, then_block), else_block), e| Spanned { node: ExprNode::If { cond: Box::new(cond), then_block: Box::new(then_block), else_block: else_block.map(Box::new) }, span: e.span() });

    let while_expr = just(Token::While)
        .ignore_then(expr.clone())
        .then(block.clone())
        .map_with(|(cond, body), e| Spanned { node: ExprNode::While { cond: Box::new(cond), body: Box::new(body) }, span: e.span() });

    // pattern parser for match arms
    let mut pat_rec = Recursive::declare();
    let wildcard = select! { Token::Ident(s) if s == "_" => () }.to(PatternNode::Wildcard).map_with(|n, e| Spanned { node: n, span: e.span() });
    let lit_pat = choice((
        select! { Token::I64(n) => Literal::I64(n) },
        select! { Token::F64(n) => Literal::F64(n) },
        select! { Token::Bool(b) => Literal::Bool(b) },
        select! { Token::Str(s) => Literal::Str(s) },
    ))
    .map(|l| PatternNode::Literal(l))
    .map_with(|n, e| Spanned { node: n, span: e.span() });

    let variant_pat = ident()
        .then(pat_rec.clone().separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>().delimited_by(just(Token::LParen), just(Token::RParen)))
        .map(|(name, args)| PatternNode::Path { path: vec![name], args })
        .map_with(|n, e| Spanned { node: n, span: e.span() });

    let ident_pat = ident()
        .map(|n| PatternNode::Identifier(n))
        .map_with(|n, e| Spanned { node: n, span: e.span() });

    pat_rec.define(choice((wildcard, lit_pat, variant_pat, ident_pat)).boxed());

    let arm = select! { Token::Op(op) if op == "|" => () }
        .ignore_then(pat_rec.clone())
        .then_ignore(just(Token::Arrow))
        .then(expr.clone());

    let match_expr = just(Token::Match)
        .ignore_then(expr.clone())
        .then(arm.repeated().at_least(1).collect::<Vec<_>>())
        .map_with(|(scrutinee, arms), e| Spanned { node: ExprNode::Match { scrutinee: Box::new(scrutinee), arms }, span: e.span() });

    let call_args = expr
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    let expr_p = choice((if_expr, while_expr, match_expr, atom.clone()))
                .clone()
        .pratt((
            postfix(
                9,
                just(Token::Op(".".to_string()))
                    .ignore_then(ident())
                    .then(call_args.clone().or_not()),
                |lhs: Expr<'src>, (name, args), _| match args {
                    Some(args) => Spanned { node: ExprNode::Call { fun: Box::new(Spanned { node: ExprNode::Path(vec![name]), span: lhs.span }), args: { let mut v = vec![lhs.clone()]; v.extend(args); v } }, span: lhs.span },
                    None => Spanned { node: ExprNode::FieldAccess { receiver: Box::new(lhs.clone()), field: name }, span: lhs.span },
                },
            ),
            postfix(
                8,
                call_args.clone(),
                |lhs: Expr<'src>, args, _| Spanned { node: ExprNode::Call { fun: Box::new(lhs.clone()), args }, span: lhs.span },
            ),
            prefix(
                7,
                select! { Token::Op(op) if op == "-" => () },
                |_op, rhs: Expr<'src>, _| Spanned { node: ExprNode::Unary { op: UnaryOp::Neg, rhs: Box::new(rhs.clone()) }, span: rhs.span },
            ),
            prefix(
                7,
                select! { Token::Op(op) if op == "!" => () },
                |_op, rhs: Expr<'src>, _| Spanned { node: ExprNode::Unary { op: UnaryOp::Not, rhs: Box::new(rhs.clone()) }, span: rhs.span },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "*" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Mul, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "/" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Div, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "+" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Add, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "-" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Sub, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == "<" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Lt, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == ">" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Gt, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(3),
                select! { Token::Op(op) if op == "==" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Eq, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
            infix(
                left(3),
                select! { Token::Op(op) if op == "!=" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned { node: ExprNode::Binary { op: BinaryOp::Ne, lhs: Box::new(l.clone()), rhs: Box::new(r.clone()) }, span: SimpleSpan { context: (), start, end } }
                },
            ),
        ))
        .then(
            just(Token::With)
                .ignore_then(just(Token::LBrace))
                .ignore_then(path())
                .then_ignore(just(Token::RBrace))
                .or_not(),
        )
        .map(|(body, maybe)| match maybe {
            Some(handler_path) => Spanned { node: ExprNode::Handle { body: Box::new(body.clone()), handler: HandlerBody::Path(handler_path) }, span: body.span },
            None => body,
        })
        .labelled("expression")
        .boxed();

    // let [mut] name [: Type]? (= expr)?
    let let_decl = just(Token::Let)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident())
        .then(just(Token::Colon).ignore_then(type_parser()).or_not())
        .then(
            select! { Token::Op(op) if op == "=" => () }
                .ignore_then(expr.clone())
                .or_not(),
        )
        .map_with(|(((is_mut, name), ty), value_opt), e| Spanned { node: StmtNode::Let { is_mut: is_mut.is_some(), name, ty, value: value_opt }, span: e.span() });

    // shorthand typed init: name: Type = expr
    let typed_init = ident()
        .then(just(Token::Colon).ignore_then(type_parser()))
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(expr.clone())
        .map_with(|((name, ty), value), e| Spanned { node: StmtNode::Let { is_mut: false, name, ty: Some(ty), value: Some(value) }, span: e.span() });

    // assignment: simple lhs (path with optional .field chain) <- expr
    let lhs = path()
        .map_with(|p, e| Spanned { node: ExprNode::Path(p), span: e.span() })
        .then(just(Token::Op(".".to_string())).ignore_then(ident()).repeated().collect::<Vec<_>>())
        .map(|(base, fields)| fields.into_iter().fold(base, |acc, f| Spanned { node: ExprNode::FieldAccess { receiver: Box::new(acc.clone()), field: f }, span: acc.span }));

    let assign = lhs
        .then_ignore(select! { Token::Op(op) if op == "<-" => () })
        .then(expr.clone())
        .map_with(|(l, r), e| Spanned { node: StmtNode::Assign(l, r), span: e.span() });

    let ret_stmt = just(Token::Return)
        .ignore_then(expr.clone().or_not())
        .map_with(|eopt, e| Spanned { node: StmtNode::Return(eopt), span: e.span() });

    let expr_stmt = expr.clone().map_with(|e1, e| Spanned { node: StmtNode::Expr(e1), span: e.span() });

    // Allow `fn` inside blocks as a local function declaration (treated as no-op for now)
    let local_fn_stmt = just(Token::Fn)
        .ignore_then(ident())
        .then(params_parser())
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(with_types(|| type_parser()))
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(block.clone())
        .map_with(|_, e| Spanned { node: StmtNode::Expr(Spanned { node: ExprNode::Literal(Literal::Unit), span: e.span() }), span: e.span() });
    let stmt_p = choice((local_fn_stmt, let_decl, typed_init, assign, ret_stmt, expr_stmt)).labelled("statement").boxed();

    expr.define(expr_p.clone());
    stmt.define(stmt_p);
    (expr, stmt, block)
}

fn import_item<'src>() -> impl Parser<'src, &'src [Token<'src>], ImportPath<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let alias = just(Token::As).ignore_then(ident()).or_not();
    path().then(alias).map(|(path, alias)| ImportPath { path, alias })
}

fn import_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::Import)
        .ignore_then(
            just(Token::LBrace)
                .ignore_then(trivia())
                .ignore_then(import_item().padded_by(trivia()).repeated().collect::<Vec<_>>())
                .then_ignore(trivia())
                .then_ignore(just(Token::RBrace)),
        )
        .map_with(|imports, e| Spanned { node: ItemNode::ImportBlock { imports }, span: e.span() })
}

fn fn_def_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, _stmt, block) = expression_bundle();
    let ty = type_parser();
    let params = params_parser();
    just(Token::Fn)
        .ignore_then(ident())
        .then(params)
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
         .then(with_types(|| type_parser()))
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(
            // Allow empty function bodies: either a normal expr, or an empty block
            expr.clone().or(block.map(|b| {
                // Ensure the empty block becomes a unit literal if empty
                match &b.node {
                    ExprNode::Block { stmts, last_expr } if stmts.is_empty() && last_expr.is_none() =>
                        Spanned { node: ExprNode::Literal(Literal::Unit), span: b.span },
                    _ => b,
                }
            }))
        )
        .map(|((((name, params), ret_type), effects), body)| Function { name, generics: vec![], params, ret_type, effects, body, is_public: true })
}

fn effect_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], EffectDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser();
    let op = ident()
        .then(params_parser())
        .then(just(Token::Arrow).ignore_then(type_parser()))
        .map(|((name, params), ret_type)| EffectOp { name, params: params.into_iter().map(|(_, t)| t).collect(), ret_type, is_public: true });
    just(Token::Effect)
        .ignore_then(
            ident().then(
                select! { Token::Op(op) if op == "<" => () }
                    .ignore_then(ident().separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>())
                    .then_ignore(select! { Token::Op(op) if op == ">" => () })
                    .or_not()
                    .ignored()
            ).map(|(name, _)| name)
        )
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(op.repeated().collect::<Vec<_>>().delimited_by(just(Token::LBrace), just(Token::RBrace)))
        .map(|(name, operations)| EffectDef { name, operations, is_public: true })
}

fn handler_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], HandlerDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser();
    let fndef = fn_def_parser();
    let effects = just(Token::Colon)
        .ignore_then(type_parser())
        .then(with_types(|| type_parser()))
        .map(|(primary, mut rest)| { let mut v = vec![primary]; v.append(&mut rest); v });
    // Handler supports either '= { ... }' or inline '{ ... }' as in tests
    let make_block = ||
        just(Token::LBrace)
            .ignore_then(trivia())
            .ignore_then(fn_def_parser().padded_by(trivia()).repeated().collect::<Vec<_>>())
            .then_ignore(trivia())
            .then_ignore(just(Token::RBrace));
    just(Token::Handler)
        .ignore_then(ident())
        .then(effects)
        .then(
            choice((
                make_block(),
                just(Token::Op("=".to_string())).ignore_then(make_block()),
            )),
        )
        .map(|((name, effects), functions)| HandlerDef { name, effects, functions, is_public: true })
}

fn interface_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], TraitDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser();
    let param = just(Token::Mut)
        .or_not()
        .ignore_then(ident())
        .then(just(Token::Colon).ignore_then(type_parser()).or_not())
        .map(|(n, t)| (Some(n), t.unwrap_or_else(|| Spanned { node: TypeNode::Path { path: vec![n], generics: vec![] }, span: SimpleSpan { context: (), start: 0, end: 0 } })));
    let sig = ident()
        .then(param.separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>().delimited_by(just(Token::LParen), just(Token::RParen)))
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
         .then(with_types(|| type_parser()))
        .map(|(((name, params), ret_type), _effects)| TraitMethod { name, params, ret_type, is_public: true });
    just(Token::Interface)
        .ignore_then(ident())
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(sig.repeated().collect::<Vec<_>>().delimited_by(just(Token::LBrace), just(Token::RBrace)))
        .map(|(name, methods)| TraitDef { name, methods, is_public: true })
}

fn impl_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], ImplBlock<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser();
    let iface = just(Token::Colon).ignore_then(path()).or_not();
    let fndef = fn_def_parser();
    just(Token::Impl)
        .ignore_then(ty)
        .then(iface)
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(fndef.repeated().collect::<Vec<_>>().delimited_by(just(Token::LBrace), just(Token::RBrace)))
        .map(|((target_type, interface), methods)| ImplBlock { target_type, interface, methods })
}

fn type_alias_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], TypeAliasDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ty = type_parser();
    let generics = ident()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(select! { Token::Op(op) if op == "<" => () }, select! { Token::Op(op) if op == ">" => () })
        .or_not()
        .map(|g| g.unwrap_or_default());

    let variant = ident().then(type_parser().delimited_by(just(Token::LParen), just(Token::RParen)).or_not());
    let union_body = ident().then(type_parser().delimited_by(just(Token::LParen), just(Token::RParen)).or_not()).separated_by(select! { Token::Op(op) if op == "|" => () }).at_least(1).collect::<Vec<_>>().map(TypeAliasBody::Union);
    let record_body = ident().then_ignore(just(Token::Colon)).then(type_parser()).separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>().delimited_by(just(Token::LBrace), just(Token::RBrace)).map(TypeAliasBody::Record);
    let body = choice((record_body, union_body, type_parser().map(TypeAliasBody::Type)));
    just(Token::Type)
        .ignore_then(ident())
        .then(generics)
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(body)
        .map(|((name, generics), aliased)| TypeAliasDef { name, generics, aliased, is_public: true })
}

fn item_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, stmt, _block) = expression_bundle();
    choice((
        import_parser(),
        spanned(fn_def_parser().map(ItemNode::Fn)),
        spanned(interface_parser().map(ItemNode::Trait)),
        spanned(effect_parser().map(ItemNode::Effect)),
        spanned(handler_parser().map(ItemNode::Handler)),
        spanned(type_alias_parser().map(ItemNode::TypeAlias)),
        spanned(impl_parser().map(ItemNode::Impl)),
        stmt.map_with(|s, e| Spanned { node: ItemNode::Stmt(s), span: e.span() }),
    ))
    .padded_by(trivia())
}

pub fn file_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], Vec<Item<'src>>, extra::Err<Rich<'src, Token<'src>>>> {
    trivia()
        .ignore_then(item_parser().padded_by(trivia()).repeated().collect::<Vec<_>>())
        .then_ignore(trivia())
        .then_ignore(end())
}

