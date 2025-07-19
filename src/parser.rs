//! This module contains the parser, which is responsible for turning a stream of tokens
//! into a structured Abstract Syntax Tree (AST). It uses a combination of recursive
//! descent and Pratt parsing to handle the language's syntax, including expressions,
//! statements, and top-level items.

use chumsky::prelude::*;
use chumsky::pratt::{self, infix, left, prefix, postfix};
use chumsky::select;
use either::Either;
use std::vec;

// These are assumed to be defined in `ast.rs` and `token.rs` respectively.
// Make sure these paths are correct in your project structure.
use crate::ast::*;
use crate::token::Token;

// --- Main Parser ---

/// The main parser function for the entire language.
/// It parses a sequence of top-level items, ignoring comments, until the end of input.
pub fn file_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Vec<Item<'src>>, extra::Err<Rich<'src, Token<'src>>>> {
    // A parser for any comments, which are ignored.
    let comment = select! { Token::Comment(_) => () }.padded();

    // The core item parser, surrounded by ignored comments.
    item_parser()
        .padded_by(comment.repeated())
        .repeated()
        .collect()
        .then_ignore(end())
}

/// Parses a single top-level item.
fn item_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    // An item can be one of many constructs. We use `choice` to try each in order.
    choice((
        import_parser(),
        extern_fn_parser(),
        fn_parser().map(Item::Fn),
        effect_parser(),
        handler_parser(),
        struct_parser(),
        enum_parser(),
        trait_parser(),
        impl_parser(),
        let_decl_parser().map(Item::Stmt), // Top-level let statements
    ))
    // Basic recovery strategy: skip tokens until the next likely start of an item.
    .recover_with(skip_then_retry_until(
        any().ignored(),
        one_of(&[
            Token::Let,
            Token::Fn,
            Token::Struct,
            Token::Enum,
            Token::Trait,
            Token::Impl,
            Token::Effect,
            Token::Handler,
            Token::Extern,
            Token::Import,
        ])
        .ignored(),
    ))
}

// --- Helper Parsers (Identifiers, Paths, Types) ---

/// Parses an identifier token.
fn ident_parser<'src>() -> impl Parser<'src, &'src [Token<'src>], &'src str, extra::Err<Rich<'src, Token<'src>>>>
{
    select! { Token::Ident(ident) => ident }.labelled("identifier")
}

/// Parses a path, e.g., `myEnum::A` or `Std::Collections::Map`
fn path_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Path<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    ident_parser()
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>()
        .labelled("path")
}

/// Parses a type annotation, e.g., `i64`, `Array<i64>`, `Map<string, i64>`
fn type_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|type_p| {
        path_parser()
            .then(
                type_p
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::Op("<".to_string())), just(Token::Op(">".to_string())))
                    .or_not(),
            )
            .map(|(path, generics)| Type {
                path,
                generics: generics.unwrap_or_default(),
            })
            .labelled("type")
    })
}

// --- Item Parsers ---

/// Parses an import statement, e.g., `import Std::Collections::Map;` or `import Std::Collections::Map as MyMap;`
fn import_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::Import)
        .ignore_then(path_parser())
        .then(just(Token::As).ignore_then(ident_parser()).or_not())
        .then_ignore(just(Token::Semi))
        .map(|(path, alias)| Item::Import { path, alias })
        .labelled("import statement")
}

/// Parses a function declaration.
fn fn_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let params = ident_parser()
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .map(|(name, ty)| (Some(name), ty))
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    let effects = just(Token::Op("/".to_string()))
        .ignore_then(
            ident_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .or_not();

    just(Token::Fn)
        .ignore_then(ident_parser())
        .then(params)
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(effects)
        .then(block_parser())
        .map(|((((name, params), ret_type), effects), body)| Function {
            name,
            params,
            ret_type,
            effects: effects.unwrap_or_default(),
            body,
        })
        .labelled("function declaration")
}

/// Parses an `extern fn` declaration.
fn extern_fn_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let params = ident_parser()
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .map(|(name, ty)| (Some(name), ty))
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    just(Token::Extern)
        .ignore_then(just(Token::Fn))
        .ignore_then(ident_parser())
        .then(params)
        .then_ignore(just(Token::Arrow))
        .then(type_parser())
        .then_ignore(just(Token::Semi))
        .map(|((name, params), ret_type)| Item::ExternFn {
            name,
            params,
            ret_type,
        })
        .labelled("external function declaration")
}

/// Parses a struct definition.
fn struct_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let field = ident_parser()
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .then_ignore(just(Token::Comma).or_not());

    let generics = ident_parser()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::Op("<".to_string())), just(Token::Op(">".to_string())))
        .or_not();

    just(Token::Struct)
        .ignore_then(ident_parser())
        .then(generics)
        .then(
            field
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((name, generics), fields)| {
            Item::Struct(StructDef {
                name,
                generics: generics.unwrap_or_default(),
                fields,
            })
        })
        .labelled("struct definition")
}

/// Parses an enum definition.
fn enum_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let variant = ident_parser()
        .then(
            type_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not(),
        )
        .then_ignore(just(Token::Comma).or_not());

    just(Token::Enum)
        .ignore_then(ident_parser().or_not()) // Optional name for anonymous enums
        .then(
            variant
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, variants)| Item::Enum(EnumDef { name, variants }))
        .labelled("enum definition")
}

/// Parses a trait definition.
fn trait_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    // A trait method is just a function signature without a body.
    let method_sig = just(Token::Fn)
        .ignore_then(ident_parser())
        .then(
            ident_parser()
                .then_ignore(just(Token::Colon))
                .then(type_parser())
                .map(|(name, ty)| (Some(name), ty))
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then_ignore(just(Token::Semi))
        .map(|((name, params), ret_type)| TraitMethod { name, params, ret_type });

    just(Token::Trait)
        .ignore_then(ident_parser())
        .then(
            method_sig
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, methods)| Item::Trait(TraitDef { name, methods }))
        .labelled("trait definition")
}

/// Parses an `impl` block.
fn impl_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::Impl)
        .ignore_then(ident_parser()) // Trait name
        .then_ignore(just(Token::For))
        .then(type_parser()) // Target type
        .then(
            fn_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((trait_name, target_type), methods)| {
            Item::Impl(ImplBlock {
                trait_name,
                target_type,
                methods,
            })
        })
        .labelled("impl block")
}

/// Parses an effect definition.
fn effect_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let operation = ident_parser()
        .then(
            type_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .then_ignore(just(Token::Arrow))
        .then(type_parser())
        .then_ignore(just(Token::Comma).or_not())
        .map(|((name, params), ret_type)| EffectOp { name, params, ret_type });

    just(Token::Effect)
        .ignore_then(ident_parser())
        .then(
            operation
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(name, operations)| Item::Effect(EffectDef { name, operations }))
        .labelled("effect definition")
}

/// Parses a handler definition.
fn handler_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let effect_list = ident_parser()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    let handler_fn = fn_parser(); // Re-use the full function parser

    just(Token::Handler)
        .ignore_then(ident_parser())
        .then(effect_list)
        .then(
            handler_fn
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((name, effects), functions)| {
            Item::Handler(HandlerDef {
                name,
                effects,
                functions,
            })
        })
        .labelled("handler definition")
}

// --- Statement Parsers ---

/// Parses a statement.
fn stmt_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Stmt<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    choice((
        let_decl_parser(),
        just(Token::Return)
            .ignore_then(expr_parser().or_not())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Return),
        // Assignment or expression statement
        expr_parser()
            .then(
                just(Token::Op("=".to_string()))
                    .ignore_then(expr_parser())
                    .or_not(),
            )
            .then_ignore(just(Token::Semi))
            .map(|(expr, rhs)| {
                if let Some(rhs) = rhs {
                    Stmt::Assign(expr, rhs)
                } else {
                    Stmt::Expr(expr)
                }
            }),
    ))
}

/// Parses a `let` declaration.
fn let_decl_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Stmt<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::Let)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident_parser())
        .then(just(Token::Colon).ignore_then(type_parser()).or_not())
        .then_ignore(just(Token::Op("=".to_string())))
        .then(expr_parser())
        .then_ignore(just(Token::Semi))
        .map(|(((is_mut, name), opt_type), value)| Stmt::Let {
            is_mut: is_mut.is_some(),
            name,
            ty: opt_type,
            value,
        })
        .labelled("let declaration")
}

/// Parses a block of statements, which is also an expression.
fn block_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    stmt_parser()
        .repeated()
        .collect::<Vec<_>>()
        .then(expr_parser().or_not()) // Optional trailing expression for the block's return value
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|(stmts, last_expr)| Expr::Block {
            stmts,
            last_expr: last_expr.map(Box::new),
        })
        .labelled("block")
}

// --- Expression Parsers ---

/// Parses expressions, handling operator precedence with a Pratt parser.
fn expr_parser<'src>(
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    recursive(|expr| {
        let atom = choice((
            // Literals
            select! { Token::I64(n) => Expr::Literal(Literal::I64(n)) },
            select! { Token::F64(n) => Expr::Literal(Literal::F64(n)) },
            select! { Token::Bool(b) => Expr::Literal(Literal::Bool(b)) },
            select! { Token::Str(s) => Expr::Literal(Literal::Str(s)) },
            // Array literal: `[a, b, c]`
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBracket), just(Token::RBracket))
                .map(Expr::Array),
            // Map literal: `{"a": 1, "b": 2}`
            expr.clone()
                .then_ignore(just(Token::Colon))
                .then(expr.clone())
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace))
                .map(Expr::Map),
            // Struct instantiation: `MyStruct<i64> { field: value }`
            path_parser()
                .then(
                    type_parser()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .delimited_by(just(Token::Op("<".to_string())), just(Token::Op(">".to_string())))
                        .or_not(),
                )
                .then(
                    ident_parser()
                        .then_ignore(just(Token::Colon))
                        .then(expr.clone())
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                )
                .map(|((path, generics), fields)| Expr::StructInit {
                    path,
                    generics: generics.unwrap_or_default(),
                    fields,
                }),
            // Grouped expression or block
            expr.clone().delimited_by(just(Token::LParen), just(Token::RParen)),
            block_parser(),
            // Control flow
            if_parser(expr.clone()),
            match_parser(expr.clone()),
            while_parser(expr.clone()),
            handle_parser(expr.clone()),
            // Other constructs
            just(Token::Perform).ignore_then(path_parser()).map(Expr::Perform),
            // Variable/path
            path_parser().map(Expr::Path),
        ))
        .padded();

        // Operator precedence parser using Pratt's algorithm
        atom.pratt((
            // Postfix operators
            postfix(
                8,
                just(Token::LParen)
                    .ignore_then(
                        expr.clone()
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(just(Token::RParen)),
                |lhs, args| Expr::Call { fun: Box::new(lhs), args },
            ),
            // Infix operators (note that field access `.` is often handled here too)
            infix(
                left(6),
                just(Token::Op("*".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Mul, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(6),
                just(Token::Op("/".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Div, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(5),
                just(Token::Op("+".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Add, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(5),
                just(Token::Op("-".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Sub, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(4),
                just(Token::Op("<".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Lt, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(4),
                just(Token::Op(">".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Gt, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            infix(
                left(3),
                just(Token::Op("==".to_string())),
                |l, _, r| Expr::Binary { op: BinaryOp::Eq, lhs: Box::new(l), rhs: Box::new(r) },
            ),
            // Prefix operators
             prefix(
                7,
                just(Token::Op("-".to_string())),
                |_, rhs| Expr::Unary { op: UnaryOp::Neg, rhs: Box::new(rhs) },
            ),
        ))
        .labelled("expression")
    })
}

fn if_parser<'src>(
    expr: impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::If)
        .ignore_then(expr.clone())
        .then(block_parser())
        .then(
            just(Token::Else)
                .ignore_then(block_parser().or(if_parser(expr)))
                .or_not(),
        )
        .map(|((cond, then_block), else_block)| Expr::If {
            cond: Box::new(cond),
            then_block: Box::new(then_block),
            else_block: else_block.map(Box::new),
        })
        .labelled("if expression")
}

fn match_parser<'src>(
    expr: impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> + Clone,
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let pattern = path_parser()
        .then(
            ident_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not(),
        )
        .map(|(path, args)| Pattern {
            path,
            args: args.unwrap_or_default(),
        });

    let arm = pattern
        .then_ignore(just(Token::FatArrow))
        .then(expr)
        .then_ignore(just(Token::Comma).or_not());

    just(Token::Match)
        .ignore_then(expr)
        .then(
            arm.repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(scrutinee, arms)| Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
        .labelled("match expression")
}

fn while_parser<'src>(
    expr: impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>>,
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    just(Token::While)
        .ignore_then(expr)
        .then(block_parser())
        .map(|(cond, body)| Expr::While {
            cond: Box::new(cond),
            body: Box::new(body),
        })
        .labelled("while loop")
}

fn handle_parser<'src>(
    expr: impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>>,
) -> impl Parser<'src, &'src [Token<'src>], Expr<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let handler_body = fn_parser()
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Handle)
        .ignore_then(expr)
        .then_ignore(just(Token::With))
        .then(
            path_parser()
                .map(HandlerBody::Path)
                .or(handler_body.map(HandlerBody::Inline)),
        )
        .map(|(body, handler)| Expr::Handle {
            body: Box::new(body),
            handler,
        })
        .labelled("handle expression")
}
