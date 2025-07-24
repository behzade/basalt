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
                // Custom generic parameter parser that handles nested generics
                select! { Token::Op(op) if op == "<" => () }
                    .ignore_then(
                        type_p
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(select! { Token::Op(op) if op == ">" => () })
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

    // Array literal parser
    let array_literal = expr
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBracket), just(Token::RBracket))
        .map(Expr::Array)
        .labelled("array literal")
        .boxed();

    // Typed map literal parser (e.g., Map<string, i64> {"a": 1, "b": 2})
    let typed_map_literal = select! { Token::Ident("Map") => vec!["Map"] }
        .then(
            // Generic parameters
            ident
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(
                    select! { Token::Op(op) if op == "<" => () },
                    select! { Token::Op(op) if op == ">" => () },
                )
                .or_not()
                .map(|g| {
                    g.unwrap_or_default()
                        .into_iter()
                        .map(|id| Type {
                            path: vec![id],
                            generics: vec![],
                        })
                        .collect::<Vec<_>>()
                }),
        )
        .then(
            // Map content - must be a literal or path as key, not just any expression
            choice((
                select! { Token::Str(s) => Expr::Literal(Literal::Str(s)) },
                select! { Token::I64(n) => Expr::Literal(Literal::I64(n)) },
                select! { Token::F64(n) => Expr::Literal(Literal::F64(n)) },
                select! { Token::Bool(b) => Expr::Literal(Literal::Bool(b)) },
                path.clone().map(Expr::Path),
            ))
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((_path, _generics), pairs)| {
            let map_pairs = pairs.into_iter().collect();
            Expr::Map(map_pairs)
        })
        .labelled("typed map literal")
        .boxed();

    // Regular map literal parser
    let map_literal = expr
        .clone()
        .then_ignore(just(Token::Colon))
        .then(expr.clone())
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|pairs| {
            let map_pairs = pairs.into_iter().collect();
            Expr::Map(map_pairs)
        })
        .labelled("map literal")
        .boxed();

    // Struct instantiation parser
    let struct_init = path
        .clone()
        .then(
            // Generic parameters - convert identifiers to types
            ident
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(
                    select! { Token::Op(op) if op == "<" => () },
                    select! { Token::Op(op) if op == ">" => () },
                )
                .or_not()
                .map(|g| {
                    g.unwrap_or_default()
                        .into_iter()
                        .map(|id| Type {
                            path: vec![id],
                            generics: vec![],
                        })
                        .collect()
                }),
        )
        .then(
            // Field initializers: name: value
            ident
                .then_ignore(just(Token::Colon))
                .then(expr.clone())
                .map(|(name, value)| (name, value))
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|((path, generics), fields)| Expr::StructInit {
            path,
            generics,
            fields,
        })
        .labelled("struct instantiation")
        .boxed();

    // Pattern parser for match expressions
    let mut pattern = Recursive::declare();

    // Literal pattern parser
    let literal_pattern = choice((
        select! { Token::I64(n) => Pattern::Literal(Literal::I64(n)) },
        select! { Token::F64(n) => Pattern::Literal(Literal::F64(n)) },
        select! { Token::Bool(b) => Pattern::Literal(Literal::Bool(b)) },
        select! { Token::Str(s) => Pattern::Literal(Literal::Str(s)) },
    ))
    .labelled("literal pattern");

    // Wildcard pattern parser
    let wildcard_pattern = just(Token::Ident("_"))
        .map(|_| Pattern::Wildcard)
        .labelled("wildcard pattern");

    // Enum variant pattern parser (with nested patterns as arguments)
    let enum_variant_pattern = path
        .clone()
        .then(
            // Optional arguments with nested patterns
            pattern
                .clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not(),
        )
        .map(|(path, args)| Pattern::Path {
            path,
            args: args.unwrap_or_default(),
        })
        .labelled("enum variant pattern");

    // Identifier pattern parser (for variable bindings)
    let identifier_pattern = select! { Token::Ident(name) => name }
        .filter(|name| name != &"_") // Exclude wildcard
        .map(Pattern::Identifier)
        .labelled("identifier pattern");

    // Combine all pattern types with proper precedence
    let pattern_definition = choice((
        literal_pattern,
        wildcard_pattern,
        enum_variant_pattern,
        identifier_pattern,
    ))
    .labelled("pattern");

    pattern.define(pattern_definition);

    // If expression parser
    let if_expr = just(Token::If)
        .ignore_then(expr.clone())
        .then(block.clone())
        .then(
            just(Token::Else)
                .ignore_then(choice((block.clone(), expr.clone())))
                .or_not(),
        )
        .map(|((condition, then_block), else_expr)| Expr::If {
            cond: Box::new(condition),
            then_block: Box::new(then_block),
            else_block: else_expr.map(Box::new),
        })
        .labelled("if expression")
        .boxed();

    // While loop parser
    let while_expr = just(Token::While)
        .ignore_then(expr.clone())
        .then(block.clone())
        .map(|(condition, body)| Expr::While {
            cond: Box::new(condition),
            body: Box::new(body),
        })
        .labelled("while loop")
        .boxed();

    // Match expression parser
    let match_arm = pattern
        .then_ignore(just(Token::FatArrow))
        .then(expr.clone())
        .map(|(pattern, body)| (pattern, body));

    let match_expr = just(Token::Match)
        .ignore_then(expr.clone())
        .then(
            match_arm
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .map(|(value, arms)| Expr::Match {
            scrutinee: Box::new(value),
            arms,
        })
        .labelled("match expression")
        .boxed();

    let _atom = choice((
        // Literals
        select! { Token::I64(n) => Expr::Literal(Literal::I64(n)) },
        select! { Token::F64(n) => Expr::Literal(Literal::F64(n)) },
        select! { Token::Bool(b) => Expr::Literal(Literal::Bool(b)) },
        select! { Token::Str(s) => Expr::Literal(Literal::Str(s)) },
        // Unit literal
        just(Token::LParen)
            .then(just(Token::RParen))
            .map(|_| Expr::Literal(Literal::Unit)),
        // Control flow expressions
        if_expr.clone(),
        while_expr.clone(),
        match_expr.clone(),
        // Typed map literal (must come before struct instantiation to avoid conflicts)
        typed_map_literal.clone(),
        // Map literal
        map_literal.clone(),
        // Struct instantiation
        struct_init.clone(),
        // Variable/path
        path.clone().map(Expr::Path),
        // Grouped expression
        expr.clone()
            .delimited_by(just(Token::LParen), just(Token::RParen)),
        // Block
        block.clone(),
        // Array literal
        array_literal.clone(),
    ))
    .boxed();

    let call_args = expr
        .clone()
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>();

    // Perform expression (must be defined after call_args)
    // Support both :: and . separators for effect operations
    let effect_path = ident
        .then(just(Token::Op(".".to_string())).ignore_then(ident))
        .map(|(effect_name, operation_name)| vec![effect_name, operation_name])
        .labelled("effect path");

    let perform_expr = just(Token::Perform)
        .ignore_then(effect_path)
        .then(
            call_args
                .clone()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .map(|(path, args)| Expr::Perform { path, args })
        .labelled("perform expression");

    // Handle expression parser
    let handle_expr = just(Token::Handle)
        .ignore_then(expr.clone())
        .then_ignore(just(Token::With))
        .then(path.clone())
        .then_ignore(just(Token::Semi))
        .map(|(body, handler_path)| Expr::Handle {
            body: Box::new(body),
            handler: HandlerBody::Path(handler_path),
        })
        .labelled("handle expression");

    // Add perform and handle expressions to atom choices
    let atom_with_perform_and_handle = choice((
        // Literals
        select! { Token::I64(n) => Expr::Literal(Literal::I64(n)) },
        select! { Token::F64(n) => Expr::Literal(Literal::F64(n)) },
        select! { Token::Bool(b) => Expr::Literal(Literal::Bool(b)) },
        select! { Token::Str(s) => Expr::Literal(Literal::Str(s)) },
        // Unit literal
        just(Token::LParen)
            .then(just(Token::RParen))
            .map(|_| Expr::Literal(Literal::Unit)),
        // Control flow expressions
        if_expr.clone(),
        while_expr.clone(),
        match_expr.clone(),
        // Typed map literal (must come before struct instantiation to avoid conflicts)
        typed_map_literal.clone(),
        // Map literal
        map_literal.clone(),
        // Struct instantiation
        struct_init.clone(),
        // Variable/path
        path.clone().map(Expr::Path),
        // Grouped expression
        expr.clone()
            .delimited_by(just(Token::LParen), just(Token::RParen)),
        // Block
        block.clone(),
        // Perform expression
        perform_expr.clone(),
        // Handle expression
        handle_expr.clone(),
        // Array literal
        array_literal.clone(),
    ))
    .boxed();

    let expr_parser = atom_with_perform_and_handle
        .pratt((
            postfix(
                9,
                just(Token::Op(".".to_string()))
                    .ignore_then(ident)
                    .then(
                        call_args
                            .clone()
                            .delimited_by(just(Token::LParen), just(Token::RParen)),
                    )
                    .map(|(method_name, args)| (method_name, args)),
                |lhs, (method_name, args), _extra| {
                    // Method call - create a path with the method name and include receiver as first arg
                    let method_path = vec![method_name];
                    let method_expr = Expr::Path(method_path);
                    let mut all_args = vec![lhs];
                    all_args.extend(args);
                    Expr::Call {
                        fun: Box::new(method_expr),
                        args: all_args,
                    }
                },
            ),
            postfix(
                8,
                call_args
                    .clone()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
                |lhs, args, _extra| Expr::Call {
                    fun: Box::new(lhs),
                    args,
                },
            ),
            postfix(
                8,
                expr.clone()
                    .delimited_by(just(Token::LBracket), just(Token::RBracket)),
                |lhs, index, _extra| Expr::Call {
                    fun: Box::new(Expr::Path(vec!["get"])),
                    args: vec![lhs, index],
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
            prefix(
                7,
                select! { Token::Op(op) if op == "!" => () },
                |_op, rhs, _extra| Expr::Unary {
                    op: UnaryOp::Not,
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
            infix(
                left(3),
                select! { Token::Op(op) if op == "!=" => () },
                |l, _op, r, _extra| Expr::Binary {
                    op: BinaryOp::Ne,
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

    // Assignment statement parser
    let assign_stmt = path
        .clone()
        .then_ignore(select! { Token::Op(op) if op == "=" => () })
        .then(expr.clone())
        .then_ignore(just(Token::Semi))
        .map(|(lhs, rhs)| Stmt::Assign(Expr::Path(lhs), rhs));

    // Control flow expressions that don't need semicolons (but can have them)
    let control_flow_stmt = choice((if_expr.clone(), while_expr.clone()))
        .then(just(Token::Semi).or_not())
        .map(|(expr, _semi)| Stmt::Expr(expr));

    // Regular expressions that need semicolons
    let expr_stmt = expr.clone().then_ignore(just(Token::Semi)).map(Stmt::Expr);

    let stmt_parser = choice((
        let_decl,
        assign_stmt,
        just(Token::Return)
            .ignore_then(expr.clone().or_not())
            .then_ignore(just(Token::Semi))
            .map(Stmt::Return),
        control_flow_stmt,
        expr_stmt,
    ))
    .labelled("statement");

    stmt.define(stmt_parser);

    (expr, stmt)
}

fn fn_decl_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    let ty = recursive(|type_p| {
        ident
            .clone()
            .separated_by(just(Token::DoubleColon))
            .at_least(1)
            .collect::<Vec<_>>()
            .then(
                // Custom generic parameter parser that handles nested generics
                select! { Token::Op(op) if op == "<" => () }
                    .ignore_then(
                        type_p
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(select! { Token::Op(op) if op == ">" => () })
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

    // Optional pub keyword
    let pub_keyword = just(Token::Pub).or_not().map(|opt| opt.is_some());

    pub_keyword
        .then(just(Token::Fn))
        .map(|(is_public, _)| is_public)
        .then(ident)
        .then(params)
        .then(just(Token::Arrow).ignore_then(ty).or_not())
        .then_ignore(just(Token::Semi))
        .map(|(((is_public, name), params), ret_type)| Function {
            name,
            generics: Vec::new(), // Function declarations don't have generics
            params,
            ret_type,
            effects: Vec::new(), // Function declarations don't have effects
            body: Expr::Block {
                stmts: vec![],
                last_expr: None,
            }, // Empty body for declarations
            is_public,
        })
        .labelled("function declaration")
}

fn fn_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Function<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, stmt) = expression_parsers();

    let ident = select! { Token::Ident(ident) => ident };

    let path = ident
        .clone()
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>();

    let ty = recursive(|type_p| {
        path.clone()
            .then(
                // Custom generic parameter parser that handles nested generics
                select! { Token::Op(op) if op == "<" => () }
                    .ignore_then(
                        type_p
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(select! { Token::Op(op) if op == ">" => () })
                    .or_not(),
            )
            .map(|(path, generics)| Type {
                path,
                generics: generics.unwrap_or_default(),
            })
    })
    .boxed();

    // Generic type parameters for function (e.g., <T, U>)
    let function_generics = ident
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(
            select! { Token::Op(op) if op == "<" => () },
            select! { Token::Op(op) if op == ">" => () },
        )
        .or_not()
        .map(|g| g.unwrap_or_default());

    let params = ident
        .clone()
        .then_ignore(just(Token::Colon))
        .then(ty.clone())
        .map(|(name, ty)| (Some(name), ty))
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    // Use the same block parser from expression_parsers, but handle comments
    let block = stmt
        .clone()
        .or(select! { Token::Comment(_) => Stmt::Error })
        .repeated()
        .collect::<Vec<_>>()
        .then(expr.clone().or_not())
        .delimited_by(just(Token::LBrace), just(Token::RBrace))
        .map(|(stmts, last_expr)| {
            // Filter out comment statements (Error statements)
            let filtered_stmts: Vec<_> = stmts
                .into_iter()
                .filter(|s| !matches!(s, Stmt::Error))
                .collect();
            Expr::Block {
                stmts: filtered_stmts,
                last_expr: last_expr.map(Box::new),
            }
        });

    // Parse effects list: / {effect1, effect2}
    let effects = just(Token::Op("/".to_string()))
        .ignore_then(
            ident
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace)),
        )
        .or_not()
        .map(|e| e.unwrap_or_default());

    // Optional pub keyword
    let pub_keyword = just(Token::Pub).or_not().map(|opt| opt.is_some());

    pub_keyword
        .then(just(Token::Fn))
        .map(|(is_public, _)| is_public)
        .then(ident)
        .then(function_generics)
        .then(params)
        .then(just(Token::Arrow).ignore_then(ty).or_not())
        .then(effects)
        .then(block)
        .map(
            |((((((is_public, name), generics), params), ret_type), effects), body)| Function {
                name,
                generics,
                params,
                ret_type,
                effects,
                body,
                is_public,
            },
        )
        .labelled("function declaration")
}

fn struct_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], StructDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    // Generic parameters
    let generics = ident
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(
            select! { Token::Op(op) if op == "<" => () },
            select! { Token::Op(op) if op == ">" => () },
        )
        .or_not()
        .map(|g| g.unwrap_or_default());

    // Field definitions: name: type
    let field = ident
        .then_ignore(just(Token::Colon))
        .then(type_parser())
        .map(|(name, ty)| (name, ty));

    let fields = field
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    // Optional pub keyword
    let pub_keyword = just(Token::Pub).or_not().map(|opt| opt.is_some());

    just(Token::Struct)
        .ignore_then(pub_keyword)
        .then(ident)
        .then(generics)
        .then(fields)
        .map(|(((is_public, name), generics), fields)| StructDef {
            name,
            generics,
            fields,
            is_public,
        })
        .labelled("struct declaration")
}

fn import_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    let path = ident
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>();

    let alias = just(Token::As).ignore_then(ident).or_not();

    just(Token::Import)
        .ignore_then(path)
        .then(alias)
        .then_ignore(just(Token::Semi))
        .map(|(path, alias)| Item::Import { path, alias })
        .labelled("import declaration")
}

fn type_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Type<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    let path = ident
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>();

    recursive(|type_p| {
        path.clone()
            .then(
                // Custom generic parameter parser that handles nested generics
                select! { Token::Op(op) if op == "<" => () }
                    .ignore_then(
                        type_p
                            .separated_by(just(Token::Comma))
                            .allow_trailing()
                            .collect::<Vec<_>>(),
                    )
                    .then_ignore(select! { Token::Op(op) if op == ">" => () })
                    .or_not(),
            )
            .map(|(path, generics)| Type {
                path,
                generics: generics.unwrap_or_default(),
            })
    })
    .labelled("type")
    .boxed()
}

fn trait_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], TraitDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    // Trait method parser - methods end with semicolon, not comma
    let method = ident
        .then_ignore(just(Token::LParen))
        .then(
            ident
                .then_ignore(just(Token::Colon))
                .then(type_parser())
                .map(|(name, ty)| (Some(name), ty))
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::RParen))
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then_ignore(just(Token::Semi))
        .map(|((name, params), ret_type)| TraitMethod {
            name,
            params,
            ret_type,
            is_public: true, // Trait methods are always public
        });

    let methods = method
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Trait)
        .ignore_then(ident)
        .then(methods)
        .map(|(name, methods)| TraitDef {
            name,
            methods,
            is_public: true, // Traits are always public
        })
        .labelled("trait declaration")
}

fn enum_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], EnumDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    // Generic parameters for enum
    let generics = ident
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(
            select! { Token::Op(op) if op == "<" => () },
            select! { Token::Op(op) if op == ">" => () },
        )
        .or_not()
        .map(|g| g.unwrap_or_default());

    // Enum variant parser
    let variant = ident
        .then(
            // Optional payload types
            type_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .or_not(),
        )
        .map(|(name, payload)| (name, payload));

    let variants = variant
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Enum)
        .ignore_then(ident)
        .then(generics)
        .then(variants)
        .map(|((name, generics), variants)| EnumDef {
            name: Some(name),
            generics,
            variants,
            is_public: true, // Enums are always public
        })
        .labelled("enum declaration")
}

fn effect_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], EffectDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    // Effect operation parser
    let effect_op = ident
        .then_ignore(just(Token::LParen))
        .then(
            type_parser()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::RParen))
        .then(just(Token::Arrow).ignore_then(type_parser()))
        .map(|((name, params), ret_type)| EffectOp {
            name,
            params,
            ret_type,
            is_public: true, // Effect operations are always public
        });

    let operations = effect_op
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Effect)
        .ignore_then(ident)
        .then(operations)
        .map(|(name, operations)| EffectDef {
            name,
            operations,
            is_public: true, // Effects are always public
        })
        .labelled("effect definition")
}

fn handler_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], HandlerDef<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, stmt) = expression_parsers();
    let ident = select! { Token::Ident(ident) => ident };

    // Handler method parser (similar to impl methods)
    let handler_method = ident
        .then_ignore(just(Token::LParen))
        .then(
            ident
                .then_ignore(just(Token::Colon))
                .then(type_parser())
                .map(|(name, ty)| (Some(name), ty))
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::RParen))
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(
            // Function body (block) - can be empty
            just(Token::LBrace)
                .ignore_then(
                    stmt.or(select! { Token::Comment(_) => Stmt::Error })
                        .repeated()
                        .collect::<Vec<_>>()
                        .then(expr.or_not()),
                )
                .then_ignore(just(Token::RBrace))
                .map(|(stmts, last_expr)| {
                    // Filter out comment statements (Error statements)
                    let filtered_stmts: Vec<_> = stmts
                        .into_iter()
                        .filter(|s| !matches!(s, Stmt::Error))
                        .collect();
                    // If no statements and no expression, create an empty block
                    if filtered_stmts.is_empty() && last_expr.is_none() {
                        Expr::Block {
                            stmts: vec![],
                            last_expr: None,
                        }
                    } else {
                        Expr::Block {
                            stmts: filtered_stmts,
                            last_expr: last_expr.map(Box::new),
                        }
                    }
                }),
        )
        .map(|(((name, params), ret_type), body)| Function {
            name,
            generics: Vec::new(), // Handler methods don't have generics
            params,
            ret_type,
            effects: Vec::new(),
            body,
            is_public: true, // Handler methods are always public
        });

    let methods = handler_method
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    // Parse handler name and effects
    let handler_name = ident;
    let effects = ident
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Handler)
        .ignore_then(handler_name)
        .then(effects)
        .then(methods)
        .map(|((name, effects), functions)| HandlerDef {
            name,
            effects,
            functions,
            is_public: true, // Handlers are always public
        })
        .labelled("handler definition")
}

fn extern_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let ident = select! { Token::Ident(ident) => ident };

    let param = choice((
        // Regular parameter: name: type
        ident
            .then_ignore(just(Token::Colon))
            .then(type_parser())
            .map(|(name, ty)| (Some(name), ty)),
        // Variadic parameter: ...
        just(Token::Op("...".to_string())).map(|_| {
            (
                None,
                Type {
                    path: vec!["..."],
                    generics: vec![],
                },
            )
        }),
    ));

    let params = param
        .separated_by(just(Token::Comma))
        .allow_trailing()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LParen), just(Token::RParen));

    // Parse extern block: extern "module_name" { ... }
    just(Token::Extern)
        .ignore_then(
            select! { Token::Str(module_name) => module_name }
        )
        .then(
            fn_decl_parser()
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LBrace), just(Token::RBrace))
        )
        .map(|(module_name, functions)| Item::ExternBlock {
            module_name,
            functions,
        })
        .labelled("extern block declaration")
}

fn impl_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], ImplBlock<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (expr, stmt) = expression_parsers();
    let ident = select! { Token::Ident(ident) => ident };

    // Parse trait name and target type
    let trait_name = ident;
    let target_type = type_parser();

    // Parse impl methods (without fn keyword)
    let impl_method = ident
        .then_ignore(just(Token::LParen))
        .then(
            ident
                .then_ignore(just(Token::Colon))
                .then(type_parser())
                .map(|(name, ty)| (Some(name), ty))
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>(),
        )
        .then_ignore(just(Token::RParen))
        .then(just(Token::Arrow).ignore_then(type_parser()).or_not())
        .then(
            // Function body (block) - can be empty
            just(Token::LBrace)
                .ignore_then(
                    stmt.or(select! { Token::Comment(_) => Stmt::Error })
                        .repeated()
                        .collect::<Vec<_>>()
                        .then(expr.or_not()),
                )
                .then_ignore(just(Token::RBrace))
                .map(|(stmts, last_expr)| {
                    // Filter out comment statements (Error statements)
                    let filtered_stmts: Vec<_> = stmts
                        .into_iter()
                        .filter(|s| !matches!(s, Stmt::Error))
                        .collect();
                    // If no statements and no expression, create an empty block
                    if filtered_stmts.is_empty() && last_expr.is_none() {
                        Expr::Block {
                            stmts: vec![],
                            last_expr: None,
                        }
                    } else {
                        Expr::Block {
                            stmts: filtered_stmts,
                            last_expr: last_expr.map(Box::new),
                        }
                    }
                }),
        )
        .map(|(((name, params), ret_type), body)| Function {
            name,
            generics: Vec::new(), // Impl methods don't have generics
            params,
            ret_type,
            effects: Vec::new(),
            body,
            is_public: true, // Impl methods are always public
        });

    let methods = impl_method
        .repeated()
        .collect::<Vec<_>>()
        .delimited_by(just(Token::LBrace), just(Token::RBrace));

    just(Token::Impl)
        .ignore_then(trait_name)
        .then_ignore(just(Token::For))
        .then(target_type)
        .then(methods)
        .map(|((trait_name, target_type), methods)| ImplBlock {
            trait_name,
            target_type,
            methods,
        })
        .labelled("impl block")
}

/// The main parser function for the entire language.
pub fn file_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Vec<Item<'src>>, extra::Err<Rich<'src, Token<'src>>>> {
    let comment = select! { Token::Comment(_) => () }.repeated();

    let item = item_parser().padded_by(comment);

    comment
        .ignore_then(item.repeated().collect::<Vec<_>>())
        .then_ignore(end())
}

/// Parses a single top-level item.
fn item_parser<'src>()
-> impl Parser<'src, &'src [Token<'src>], Item<'src>, extra::Err<Rich<'src, Token<'src>>>> {
    let (_expr, stmt) = expression_parsers();
    let fn_decl = fn_parser().map(Item::Fn);
    let stmt_item = stmt.map(Item::Stmt);

    // Struct parser
    let struct_parser = struct_parser().map(Item::Struct);

    // Import parser
    let import_parser = import_parser();

    // Trait parser
    let trait_parser = trait_parser().map(Item::Trait);

    // Enum parser
    let enum_parser = enum_parser().map(Item::Enum);

    // Effect parser
    let effect_parser = effect_parser().map(Item::Effect);

    // Handler parser
    let handler_parser = handler_parser().map(Item::Handler);

    // Extern parser
    let extern_parser = extern_parser();

    // Impl parser
    let impl_parser = impl_parser().map(Item::Impl);

    choice((
        fn_decl,
        struct_parser,
        trait_parser,
        enum_parser,
        impl_parser,
        effect_parser,
        handler_parser,
        extern_parser,
        import_parser,
        stmt_item,
    ))
    .recover_with(skip_then_retry_until(
        any().ignored(),
        one_of([
            Token::Fn,
            Token::Struct,
            Token::Trait,
            Token::Enum,
            Token::Impl,
            Token::Effect,
            Token::Handler,
            Token::Extern,
            Token::Import,
            Token::Let,
        ])
        .ignored(),
    ))
}
