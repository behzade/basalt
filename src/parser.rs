//! Parser aligned with Tree-sitter grammar (with full spans on every node)

use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::select;

use crate::ast::*;
use crate::token::{SimpleSpan, Token};

fn trivia<'src>() -> impl Parser<'src, &'src [Token<'src>], (), extra::Err<Rich<'src, Token<'src>>>>
{
    select! { Token::Comment(_) => (), Token::Semicolon => () }
        .repeated()
        .ignored()
}

fn spanned<'src, T>(
    p: impl Parser<'src, &'src [Token<'src>], T, extra::Err<Rich<'src, Token<'src>>>>,
) -> impl Parser<'src, &'src [Token<'src>], Spanned<T>, extra::Err<Rich<'src, Token<'src>>>> {
    p.map_with(|node, e| Spanned {
        node,
        span: e.span(),
    })
}

fn ident<'src>()
-> impl Parser<'src, &'src [Token<'src>], &'src str, extra::Err<Rich<'src, Token<'src>>>> {
    select! { Token::Ident(s) => s }.labelled("identifier")
}

fn path<'src>()
-> impl Parser<'src, &'src [Token<'src>], Path<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let colon = select! { Token::Colon => () };
    let double_colon = colon.clone().then(colon).to(());
    ident()
        .separated_by(double_colon)
        .at_least(1)
        .collect::<Vec<_>>()
}

fn effects_types<'src, F, P>(
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
    just(Token::Effects)
        .ignore_then(list)
        .or_not()
        .map(|v| v.unwrap_or_default())
}

fn type_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|type_p| {
        let lt = select! { Token::Op(op) if op == "<" => () };
        let gt = select! { Token::Op(op) if op == ">" => () };

        let path_type = path()
            .then(
                lt.ignore_then(
                    type_p
                        .clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(gt)
                .or_not(),
            )
            .map(|(p, g)| TypeNode::Path {
                path: p,
                generics: g.unwrap_or_default(),
            })
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

        let never_type = select! { Token::Op(op) if op == "!" => () }
            .to(TypeNode::Never)
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

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
            .then(effects_types(make_ty).boxed())
            .map(|((params, ret), effects)| TypeNode::Function {
                params,
                ret: Box::new(ret),
                effects,
            })
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

        let atom = choice((fn_type, never_type, path_type)).boxed();

        // anonymous union types: low precedence, right-associative A | B | C
        atom.clone()
            .then(
                select! { Token::Op(op) if op == "|" => () }
                    .ignore_then(type_p.clone())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .map_with(|(first, mut rest), e| {
                if rest.is_empty() {
                    first
                } else {
                    let mut types = Vec::with_capacity(1 + rest.len());
                    types.push(first);
                    types.append(&mut rest);
                    Spanned {
                        node: TypeNode::Union(types),
                        span: e.span(),
                    }
                }
            })
            .boxed()
    })
    .boxed()
    .labelled("type")
}

// Atom-only type parser: like `type_parser` but WITHOUT the top-level anonymous union `|` parsing.
// This is used where `|` has another meaning at the outer grammar level (e.g., separating tagged
// union variants in a `type` alias), so we must not greedily consume it inside a payload type.
fn type_atom_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|type_p| {
        let lt = select! { Token::Op(op) if op == "<" => () };
        let gt = select! { Token::Op(op) if op == ">" => () };

        let path_type = path()
            .then(
                lt.ignore_then(
                    type_p
                        .clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(gt)
                .or_not(),
            )
            .map(|(p, g)| TypeNode::Path {
                path: p,
                generics: g.unwrap_or_default(),
            })
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

        let never_type = select! { Token::Op(op) if op == "!" => () }
            .to(TypeNode::Never)
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

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
            .then(effects_types(make_ty).boxed())
            .map(|((params, ret), effects)| TypeNode::Function {
                params,
                ret: Box::new(ret),
                effects,
            })
            .map_with(|node, e| Spanned {
                node,
                span: e.span(),
            });

        choice((fn_type, never_type, path_type)).boxed()
    })
    .boxed()
    .labelled("type-atom")
}

fn params_parser<'src>() -> impl Parser<
    'src,
    &'src [Token<'src>],
    Vec<(bool, Option<&'src str>, Type<'src>)>,
    extra::Err<Rich<'src, Token<'src>>>,
> {
    let ty = type_parser().boxed();
    let named = ident()
        .then_ignore(just(Token::Colon))
        .then(ty.clone())
        .map(|(n, t)| (false, Some(n), t));
    let flexible_param = just(Token::Mut)
        .or_not()
        .then(ident())
        .then(just(Token::Colon).ignore_then(ty.clone()).or_not())
        .map_with(|((is_mut, name), ty), e| {
            (
                is_mut.is_some(),
                Some(name),
                ty.unwrap_or_else(|| Spanned {
                    node: TypeNode::Path {
                        path: vec![name],
                        generics: vec![],
                    },
                    span: e.span(),
                }),
            )
        });
    choice((named, flexible_param))
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen))
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
            stmt.clone()
                .padded_by(trivia())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(trivia())
        .then(expr.clone().padded_by(trivia()).or_not())
        .then_ignore(trivia())
        .then_ignore(just(Token::RBrace))
        .map_with(|(stmts, last_expr), e| Spanned {
            node: ExprNode::Block {
                stmts,
                last_expr: last_expr.map(Box::new),
            },
            span: e.span(),
        })
        .labelled("block")
        .boxed();

    let unit = just(Token::LParen)
        .then_ignore(just(Token::RParen))
        .map_with(|_, e| Spanned {
            node: ExprNode::Literal(Literal::Unit),
            span: e.span(),
        });

    let lit = choice((
        select! { Token::I8(n) => Literal::I8(n) },
        select! { Token::I16(n) => Literal::I16(n) },
        select! { Token::I32(n) => Literal::I32(n) },
        select! { Token::I64(n) => Literal::I64(n) },
        select! { Token::U8(n) => Literal::U8(n) },
        select! { Token::U16(n) => Literal::U16(n) },
        select! { Token::U32(n) => Literal::U32(n) },
        select! { Token::U64(n) => Literal::U64(n) },
        select! { Token::F32(n) => Literal::F32(n) },
        select! { Token::F64(n) => Literal::F64(n) },
        select! { Token::Bool(b) => Literal::Bool(b) },
        select! { Token::Str(s) => Literal::Str(s) },
    ))
    .map_with(|l, e| Spanned {
        node: ExprNode::Literal(l),
        span: e.span(),
    });

    let perform = just(Token::Perform)
        .ignore_then(ident())
        .then_ignore(just(Token::Op(".".to_string())))
        .then(ident())
        .then(
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .map_with(|((e1, e2), args), e| Spanned {
            node: ExprNode::Perform {
                path: vec![e1, e2],
                args,
            },
            span: e.span(),
        });

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
                acc = Spanned {
                    node: ExprNode::Handle {
                        body: Box::new(acc),
                        handler: HandlerBody::Path(h),
                    },
                    span: e.span(),
                };
            }
            acc
        });

    // Struct/union init fields: '{' name: expr, ... '}'
    let make_struct_fields = {
        let expr = expr.clone();
        move || {
            ident()
                .then_ignore(just(Token::Colon).labelled("colon"))
                .then(expr.clone())
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace))
                .labelled("struct fields")
        }
    };

    let struct_init = path()
        .map_with(|p, e| (p, e.span()))
        .then(make_struct_fields().map_with(|fs, e| (fs, e.span())))
        .map(|((p, p_span), (fields, fields_span))| {
            let span = SimpleSpan {
                context: (),
                start: p_span.start,
                end: fields_span.end,
            };
            Spanned {
                node: ExprNode::StructInit {
                    path: p,
                    generics: vec![],
                    fields,
                },
                span,
            }
        })
        .labelled("struct init")
        .boxed();

    // Union constructor: Path :: Variant { fields }
    let double_colon = select! { Token::Colon => () }
        .then(select! { Token::Colon => () })
        .to(());
    let union_init = path()
        .map_with(|p, e| (p, e.span()))
        .then_ignore(double_colon)
        .then(ident())
        .then(make_struct_fields().map_with(|fs, e| (fs, e.span())))
        .map(|(((p, p_span), variant), (fields, fields_span))| {
            let span = SimpleSpan {
                context: (),
                start: p_span.start,
                end: fields_span.end,
            };
            Spanned {
                node: ExprNode::UnionInit {
                    path: p,
                    variant,
                    fields,
                },
                span,
            }
        })
        .labelled("union init")
        .boxed();

    // Anonymous function literal expression: fn(...) -> ... effects { ... } { ... }
    // fn literal: fn(params) -> ret effects { effects } { body }
    let fn_lit_expr = just(Token::Fn)
        .ignore_then(params_parser())
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(effects_types(|| type_parser()))
        .then(block.clone())
        .map_with(|(((params, ret_type), effects), body), e| Spanned {
            node: ExprNode::FnLiteral {
                params,
                ret_type,
                effects,
                body: Box::new(body),
            },
            span: e.span(),
        })
        .labelled("fn literal");

    let atom = choice((
        unit,
        lit,
        perform,
        with_block,
        fn_lit_expr,
        block.clone(),
        union_init,
        struct_init,
        path().map_with(|p, e| Spanned {
            node: ExprNode::Path(p),
            span: e.span(),
        }),
        expr.clone()
            .delimited_by(just(Token::LParen), just(Token::RParen)),
    ))
    .boxed();

    // if / while / match expressions
    let if_expr = just(Token::If)
        .ignore_then(expr.clone())
        .then(block.clone())
        .then(just(Token::Else).ignore_then(block.clone()).or_not())
        .map_with(|((cond, then_block), else_block), e| Spanned {
            node: ExprNode::If {
                cond: Box::new(cond),
                then_block: Box::new(then_block),
                else_block: else_block.map(Box::new),
            },
            span: e.span(),
        })
        .labelled("if expression");

    let while_expr = just(Token::While)
        .ignore_then(expr.clone())
        .then(block.clone())
        .map_with(|(cond, body), e| Spanned {
            node: ExprNode::While {
                cond: Box::new(cond),
                body: Box::new(body),
            },
            span: e.span(),
        })
        .labelled("while expression");

    // pattern parser for match arms
    let mut pat_rec = Recursive::declare();
    let wildcard = select! { Token::Ident(s) if s == "_" => () }
        .to(PatternNode::Wildcard)
        .map_with(|n, e| Spanned {
            node: n,
            span: e.span(),
        });
    let lit_pat = choice((
        select! { Token::I8(n) => Literal::I8(n) },
        select! { Token::I16(n) => Literal::I16(n) },
        select! { Token::I32(n) => Literal::I32(n) },
        select! { Token::I64(n) => Literal::I64(n) },
        select! { Token::U8(n) => Literal::U8(n) },
        select! { Token::U16(n) => Literal::U16(n) },
        select! { Token::U32(n) => Literal::U32(n) },
        select! { Token::U64(n) => Literal::U64(n) },
        select! { Token::F32(n) => Literal::F32(n) },
        select! { Token::F64(n) => Literal::F64(n) },
        select! { Token::Bool(b) => Literal::Bool(b) },
        select! { Token::Str(s) => Literal::Str(s) },
    ))
    .map(|l| PatternNode::Literal(l))
    .map_with(|n, e| Spanned {
        node: n,
        span: e.span(),
    });

    // New variant bind pattern: name: Path
    let variant_bind_pat = ident()
        .then_ignore(just(Token::Colon))
        .then(path())
        .map(|(binding, variant_path)| PatternNode::VariantBind {
            binding,
            variant_path,
        })
        .map_with(|n, e| Spanned {
            node: n,
            span: e.span(),
        });

    let ident_pat = ident()
        .map(|n| PatternNode::Identifier(n))
        .map_with(|n, e| Spanned {
            node: n,
            span: e.span(),
        });

    pat_rec.define(choice((wildcard, lit_pat, variant_bind_pat, ident_pat)).boxed());

    let arm = select! { Token::Op(op) if op == "|" => () }
        .ignore_then(pat_rec.clone())
        .then_ignore(just(Token::Arrow))
        .then(expr.clone());

    let match_expr = just(Token::Match)
        .ignore_then(expr.clone())
        .then(arm.repeated().at_least(1).collect::<Vec<_>>())
        .map_with(|(scrutinee, arms), e| Spanned {
            node: ExprNode::Match {
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span: e.span(),
        })
        .labelled("match expression");

    let call_args = expr
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    // Capture span for the parentheses and args
    let call_args_with_span = call_args.clone().map_with(|args, e| (args, e.span()));

    let expr_p = choice((if_expr, while_expr, match_expr, atom.clone()))
        .clone()
        .pratt((
            // Method call postfix: .ident(args)
            postfix(
                9,
                just(Token::Op(".".to_string()))
                    .then(ident())
                    .then(call_args.clone())
                    .map_with(|((_, name), args), e| (name, args, e.span())),
                |lhs: Expr<'src>,
                 (name, args, tail_span): (&'src str, Vec<Expr<'src>>, SimpleSpan),
                 _| {
                    let start = lhs.span.start;
                    let end = tail_span.end;
                    Spanned {
                        node: ExprNode::MethodCall {
                            receiver: Box::new(lhs),
                            method: name,
                            args,
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            // Field access postfix: .ident not followed by '('
            postfix(
                9,
                just(Token::Op(".".to_string()))
                    .then(ident())
                    .then(just(Token::LParen).not().ignored())
                    .map_with(|((_, name), ()), e| (name, e.span())),
                |lhs: Expr<'src>, (name, tail_span): (&'src str, SimpleSpan), _| {
                    let start = lhs.span.start;
                    let end = tail_span.end;
                    Spanned {
                        node: ExprNode::FieldAccess {
                            receiver: Box::new(lhs),
                            field: name,
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            postfix(
                8,
                call_args_with_span.clone(),
                |lhs: Expr<'src>, (args, args_span): (Vec<Expr<'src>>, SimpleSpan), _| {
                    let start = lhs.span.start;
                    let end = args_span.end;
                    Spanned {
                        node: ExprNode::Call {
                            fun: Box::new(lhs),
                            args,
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            prefix(
                7,
                select! { Token::Op(op) if op == "-" => () }.map_with(|_, e| e.span()),
                |op_span: SimpleSpan, rhs: Expr<'src>, _| {
                    let start = op_span.start;
                    let end = rhs.span.end;
                    Spanned {
                        node: ExprNode::Unary {
                            op: UnaryOp::Neg,
                            rhs: Box::new(rhs),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            prefix(
                7,
                select! { Token::Op(op) if op == "!" => () }.map_with(|_, e| e.span()),
                |op_span: SimpleSpan, rhs: Expr<'src>, _| {
                    let start = op_span.start;
                    let end = rhs.span.end;
                    Spanned {
                        node: ExprNode::Unary {
                            op: UnaryOp::Not,
                            rhs: Box::new(rhs),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "*" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Mul,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(6),
                select! { Token::Op(op) if op == "/" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Div,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "+" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Add,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(5),
                select! { Token::Op(op) if op == "-" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Sub,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == "<" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Lt,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(4),
                select! { Token::Op(op) if op == ">" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Gt,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(3),
                select! { Token::Op(op) if op == "==" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Eq,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
                },
            ),
            infix(
                left(3),
                select! { Token::Op(op) if op == "!=" => () },
                |l: Expr<'src>, _op, r: Expr<'src>, _| {
                    let start = l.span.start;
                    let end = r.span.end;
                    Spanned {
                        node: ExprNode::Binary {
                            op: BinaryOp::Ne,
                            lhs: Box::new(l.clone()),
                            rhs: Box::new(r.clone()),
                        },
                        span: SimpleSpan {
                            context: (),
                            start,
                            end,
                        },
                    }
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
            Some(handler_path) => Spanned {
                node: ExprNode::Handle {
                    body: Box::new(body.clone()),
                    handler: HandlerBody::Path(handler_path),
                },
                span: body.span,
            },
            None => body,
        })
        .labelled("expression")
        .boxed();

    // let [mut] name [: Type]? (= expr)?
    let let_decl = just(Token::Let)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident())
        .then(just(Token::Ident("in")).ignore_then(ident()).or_not())
        .then(just(Token::Colon).ignore_then(type_parser()).or_not())
        .then(
            select! { Token::Op(op) if op == "=" => () }
                .ignore_then(expr.clone())
                .or_not(),
        )
        .map_with(|((((is_mut, name), memory), ty), value_opt), e| Spanned {
            node: StmtNode::Let {
                is_mut: is_mut.is_some(),
                name,
                memory,
                ty,
                value: value_opt,
            },
            span: e.span(),
        });

    // shorthand typed init: name: Type = expr
    let typed_init = ident()
        .then(just(Token::Colon).ignore_then(type_parser()))
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(expr.clone())
        .map_with(|((name, ty), value), e| Spanned {
            node: StmtNode::Let {
                is_mut: false,
                name,
                memory: None,
                ty: Some(ty),
                value: Some(value),
            },
            span: e.span(),
        });

    let lhs = path()
        .map_with(|p, e| Spanned {
            node: ExprNode::Path(p),
            span: e.span(),
        })
        .then(
            just(Token::Op(".".to_string()))
                .ignore_then(ident())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .map(|(base, fields)| {
            fields.into_iter().fold(base, |acc, f| Spanned {
                node: ExprNode::FieldAccess {
                    receiver: Box::new(acc.clone()),
                    field: f,
                },
                span: acc.span,
            })
        });

    let assign = lhs
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(expr.clone())
        .map_with(|(l, r), e| Spanned {
            node: StmtNode::Assign(l, r),
            span: e.span(),
        });

    let ret_stmt = just(Token::Return)
        .ignore_then(expr.clone().or_not())
        .map_with(|eopt, e| Spanned {
            node: StmtNode::Return(eopt),
            span: e.span(),
        })
        .labelled("return");

    let expr_stmt = expr.clone().map_with(|e1, e| Spanned {
        node: StmtNode::Expr(e1),
        span: e.span(),
    });

    // Allow `fn` inside blocks as a local function declaration with a direct block body (no '=')
    // Desugar to: let name: (params)->ret effects { effects } = { body }
    // Local function statement remains unsupported; prefer explicit let with fn-literal
    let local_fn_stmt = just(Token::Fn)
        .ignore_then(fn_signature_parser())
        .then(block.clone())
        .map_with(|_, e| Spanned {
            node: StmtNode::Expr(Spanned {
                node: ExprNode::Literal(Literal::Unit),
                span: e.span(),
            }),
            span: e.span(),
        })
        .labelled("local function");
    let stmt_p = choice((
        local_fn_stmt,
        let_decl,
        typed_init,
        assign,
        ret_stmt,
        expr_stmt,
    ))
    .labelled("statement")
    .boxed();

    expr.define(expr_p.clone());
    stmt.define(stmt_p);
    (expr, stmt, block)
}

fn import_item<'src>()
-> impl Parser<'src, &'src [Token<'src>], ImportPath<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let alias = just(Token::As).ignore_then(ident()).or_not();
    path()
        .then(alias)
        .map(|(path, alias)| ImportPath { path, alias })
}

fn import_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::Import)
        .ignore_then(
            just(Token::LBrace)
                .ignore_then(trivia())
                .ignore_then(
                    import_item()
                        .padded_by(trivia())
                        .repeated()
                        .collect::<Vec<_>>(),
                )
                .then_ignore(trivia())
                .then_ignore(just(Token::RBrace)),
        )
        .map_with(|imports, e| Spanned {
            node: ItemNode::ImportBlock { imports },
            span: e.span(),
        })
}

fn fn_def_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (_expr, _stmt, block) = expression_bundle();
    let vis = just(Token::Pub).or_not().map(|m| m.is_some());
    let body = block.clone().map(|b| match &b.node {
        ExprNode::Block { stmts, last_expr } if stmts.is_empty() && last_expr.is_none() => {
            Spanned {
                node: ExprNode::Literal(Literal::Unit),
                span: b.span,
            }
        }
        _ => b,
    });
    let extern_body = empty().map_with(|_, e| Spanned {
        node: ExprNode::Literal(Literal::Unit),
        span: e.span(),
    });

    let normal_fn = vis
        .clone()
        .then_ignore(just(Token::Fn))
        .then(fn_signature_parser())
        .then(body)
        .map(|((is_public, signature), body)| (is_public, false, signature, body));

    let extern_fn = vis
        .then_ignore(just(Token::Extern))
        .then_ignore(just(Token::Fn))
        .then(fn_signature_parser())
        .then(extern_body)
        .map(|((is_public, signature), body)| (is_public, true, signature, body));

    choice((extern_fn, normal_fn))
        .map(
            |(is_public, is_extern, (name, generics, params, ret_type, effects), body)| Function {
                name,
                generics,
                params,
                ret_type,
                effects,
                body,
                is_public,
                is_extern,
            },
        )
        .labelled("function definition")
}

fn effect_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], EffectDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let vis = just(Token::Pub).or_not().map(|m| m.is_some()).boxed();
    let op = vis
        .clone()
        .then(ident())
        .then(params_parser())
        .then(just(Token::Arrow).ignore_then(type_parser()))
        .map(|(((is_public, name), params), ret_type)| EffectOp {
            name,
            params: params.into_iter().map(|(_, _, t)| t).collect(),
            ret_type,
            is_public,
        })
        .boxed();
    vis.clone()
        .then_ignore(just(Token::Effect))
        .then(
            ident()
                .then(
                    select! { Token::Op(op) if op == "<" => () }
                        .ignore_then(
                            ident()
                                .separated_by(just(Token::Comma))
                                .allow_trailing()
                                .collect::<Vec<_>>(),
                        )
                        .then_ignore(select! { Token::Op(op) if op == ">" => () })
                        .or_not()
                        .ignored(),
                )
                .map(|(name, _)| name),
        )
        .then(
            op.clone()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((is_public, name), operations)| EffectDef {
            name,
            operations,
            is_public,
        })
}

fn handler_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], HandlerDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let effects = just(Token::Colon)
        .ignore_then(type_parser())
        .then(effects_types(|| type_parser()))
        .map(|(primary, mut rest)| {
            let mut v = vec![primary];
            v.append(&mut rest);
            v
        });
    let vis = just(Token::Pub).or_not().map(|m| m.is_some()).boxed();
    // Handler supports either '= { ... }' or inline '{ ... }'
    let make_block = || {
        just(Token::LBrace)
            .ignore_then(trivia())
            .ignore_then(
                fn_def_parser()
                    .padded_by(trivia())
                    .repeated()
                    .collect::<Vec<_>>(),
            )
            .then_ignore(trivia())
            .then_ignore(just(Token::RBrace))
    };
    vis.clone()
        .then_ignore(just(Token::Handler))
        .then(ident())
        .then(effects)
        .then(make_block())
        .map(|(((is_public, name), effects), functions)| HandlerDef {
            name,
            effects,
            functions,
            is_public,
        })
}

fn fn_signature_parser<'src>() -> impl Parser<
    'src,
    &'src [Token<'src>],
    (
        &'src str,
        Vec<&'src str>,
        Vec<(bool, Option<&'src str>, Type<'src>)>,
        Option<Type<'src>>,
        Vec<Type<'src>>,
    ),
    extra::Err<Rich<'src, Token<'src>>>,
> {
    let generics = ident()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(
            select! { Token::Op(op) if op == "<" => () },
            select! { Token::Op(op) if op == ">" => () },
        )
        .or_not()
        .map(|g| g.unwrap_or_default());

    ident()
        .then(generics)
        .then(params_parser())
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(effects_types(|| type_parser()))
        .map(|((((name, generics), params), ret_type), effects)| {
            (name, generics, params, ret_type, effects)
        })
        .labelled("function signature")
}

fn type_alias_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], TypeAliasDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let vis = just(Token::Pub).or_not().map(|m| m.is_some()).boxed();
    let generics = ident()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(
            select! { Token::Op(op) if op == "<" => () },
            select! { Token::Op(op) if op == ">" => () },
        )
        .or_not()
        .map(|g| g.unwrap_or_default());

    // Tagged union: Tag: Type (| Tag: Type)*
    // Use `type_atom_parser` here to avoid consuming the outer '|' as an anonymous union inside the payload.
    let tagged_variant = ident()
        .then_ignore(just(Token::Colon))
        .then(type_atom_parser());
    let tagged_union_body = tagged_variant
        .separated_by(select! { Token::Op(op) if op == "|" => () })
        .at_least(1)
        .collect::<Vec<_>>()
        .map(|vs| TypeAliasBody::Union { variants: vs });

    let body = choice((tagged_union_body, type_parser().map(TypeAliasBody::Type)));
    vis.then(just(Token::Type).ignore_then(ident()))
        .then(generics)
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(body)
        .map(|(((is_public, name), generics), aliased)| TypeAliasDef {
            name,
            generics,
            aliased,
            is_public,
        })
}

fn struct_def_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], StructDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let vis = just(Token::Pub).or_not().map(|m| m.is_some());
    let field = just(Token::Pub)
        .or_not()
        .map(|m| m.is_some())
        .then(ident())
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .map(|((_is_public, name), ty)| (name, ty));
    vis.then_ignore(just(Token::Struct))
        .then(ident())
        .then(
            field
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((is_public, name), fields)| StructDef {
            name,
            generics: vec![],
            fields,
            is_public,
        })
}

fn memory_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], MemoryDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let usize_literal = select! {
        Token::I32(n) if n >= 0 => n as usize,
        Token::I64(n) if n >= 0 => n as usize,
        Token::U8(n) => n as usize,
        Token::U16(n) => n as usize,
        Token::U32(n) => n as usize,
        Token::U64(n) => n as usize,
    };

    let chunk_args = usize_literal
        .then(just(Token::Comma).ignore_then(usize_literal).or_not())
        .delimited_by(just(Token::LParen), just(Token::RParen));

    just(Token::Memory)
        .ignore_then(ident())
        .then_ignore(just(Token::Colon))
        .then_ignore(select! { Token::Ident(s) if s == "chunk" => () })
        .then(chunk_args)
        .map(|(name, (byte_limit, object_limit))| MemoryDef {
            name,
            byte_limit,
            object_limit,
        })
}

fn item_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (_expr, stmt, _block) = expression_bundle();
    choice((
        import_parser(),
        spanned(memory_parser().map(ItemNode::Memory)),
        spanned(fn_def_parser().map(ItemNode::Fn)),
        spanned(effect_parser().map(ItemNode::Effect)),
        spanned(handler_parser().map(ItemNode::Handler)),
        spanned(type_alias_parser().map(ItemNode::TypeAlias)),
        spanned(struct_def_parser().map(ItemNode::Struct)),
        stmt.map_with(|s, e| Spanned {
            node: ItemNode::Stmt(s),
            span: e.span(),
        }),
    ))
    .padded_by(trivia())
}

pub fn file_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Vec<Item<'src>>, extra::Err<Rich<'src, Token<'src>>>> {
    trivia()
        .ignore_then(
            item_parser()
                .padded_by(trivia())
                .repeated()
                .collect::<Vec<_>>(),
        )
        .then_ignore(trivia())
        .then_ignore(end())
}
