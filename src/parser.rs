// parser.rs

use chumsky::prelude::*;
use chumsky::Parser;

use crate::ast::{
    BlockStatement, Expression, FunctionDeclaration, Identifier, Path, Program, Statement, Type,
};
use crate::token::Token;

// The type of the input stream to the parser.
// Note: Spans are not fully implemented yet and are placeholders.
type Stream<'a> = chumsky::stream::Stream<'a, Token, SimpleSpan>;
// The type for parser errors.
type ParseError<'a> = Simple<'a, Token>;

/// The main parser function that defines the entire grammar.
pub fn parser<'a>() -> impl Parser<'a, Stream<'a>, Program, extra::Err<ParseError<'a>>> {
    // --- Forward Declarations ---
    // Expressions and statements can be recursive, so we need to declare them upfront.
    let mut expr = Recursive::declare();
    let mut stmt = Recursive::declare();
    let mut block = Recursive::declare();

    // --- Basic Building Blocks ---

    // A parser for identifiers.
    let ident = select! { Token::Ident(s) => Identifier(s) }.labelled("identifier");

    // A parser for paths like `MyModule::MyType`.
    let path = ident
        .clone()
        .separated_by(just(Token::DoubleColon))
        .at_least(1)
        .collect::<Vec<_>>()
        .map(Path)
        .labelled("path");

    // A parser for type annotations.
    let type_parser = recursive(|type_p| {
        let simple_type = ident.clone().map(Type::Ident);

        // A parser for generic types like `Array<i64>`.
        let generic_type = ident.clone().then(
            type_p
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::Lt), just(Token::Gt)),
        )
        .map(|(base, params)| Type::Generic { base, params });

        generic_type.or(simple_type)
    })
    .labelled("type");

    // --- Literals ---
    let literal = select! {
        Token::Int(i) => Expression::IntegerLiteral(i),
        Token::Float(f) => Expression::FloatLiteral(f),
        Token::String(s) => Expression::StringLiteral(s),
        Token::Bool(b) => Expression::BooleanLiteral(b),
    }
    .labelled("literal");

    // --- Expressions ---

    // An atom is the smallest unit of an expression.
    let atom = literal
        .or(ident.clone().map(Expression::Identifier))
        // Parenthesized expression, e.g., `(1 + 2)`.
        .or(expr
            .clone()
            .delimited_by(just(Token::LParen), just(Token::RParen)))
        // An `if-else` expression.
        .or(just(Token::If)
            .ignore_then(expr.clone().delimited_by(just(Token::LParen), just(Token::RParen)))
            .then(block.clone())
            .then(just(Token::Else).ignore_then(block.clone()).or_not())
            .map(|((condition, consequence), alternative)| Expression::If {
                condition: Box::new(condition),
                consequence,
                alternative,
            }))
        // A `while` loop expression.
        .or(just(Token::While)
            .ignore_then(expr.clone().delimited_by(just(Token::LParen), just(Token::RParen)))
            .then(block.clone())
            .map(|(condition, body)| Expression::While {
                condition: Box::new(condition),
                body,
            }))
        // A block expression.
        .or(block.clone().map(Expression::Block))
        // Array literal, e.g., `[1, 2, 3]`.
        .or(expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect::<Vec<_>>()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(|elements| Expression::ArrayLiteral { elements }))
        // Struct literal, e.g., `MyStruct { a: 1, b: 2 }`.
        .or(path
            .clone()
            .then(
                ident
                    .clone()
                    .then_ignore(just(Token::Colon))
                    .then(expr.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace)),
            )
            .map(|(name, fields)| Expression::StructLiteral { name, fields }))
        // Anonymous function literal.
        .or(just(Token::Fn)
            .ignore_then(
                ident
                    .clone()
                    .then_ignore(just(Token::Colon))
                    .then(type_parser.clone())
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect::<Vec<_>>()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then(just(Token::Arrow).ignore_then(type_parser.clone()).or_not())
            .then(
                just(Token::Slash)
                    .ignore_then(
                        path.clone()
                            .separated_by(just(Token::Comma))
                            .collect::<Vec<_>>()
                            .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                    )
                    .or_not(),
            )
            .then(block.clone())
            .map(
                |(((params, return_type), effects), body)| Expression::FunctionLiteral {
                    params,
                    return_type: return_type.unwrap_or(Type::Ident(Identifier("none".to_string()))), // Default to none
                    effects,
                    body,
                },
            ));

    // Postfix operations like function calls and array indexing.
    let call = atom
        .clone()
        .foldl(
            // Function call, e.g., `my_func(a, b)`.
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .map_with(|args, e| (args, e.span()))
                .repeated()
                .collect::<Vec<_>>(),
            |f, (args, _span)| {
                Expression::Call {
                    function: Box::new(f),
                    arguments: args,
                }
            },
        )
        .foldl(
            // Array indexing, e.g., `my_array[i]`.
            expr.clone()
                .delimited_by(just(Token::LBracket), just(Token::RBracket))
                .map_with(|index, e| (index, e.span()))
                .repeated()
                .collect::<Vec<_>>(),
            |arr, (idx, _span)| {
                Expression::Index {
                    left: Box::new(arr),
                    index: Box::new(idx),
                }
            },
        );

    // Prefix operators like `!` and `-`.
    let op = |c| just(c);
    let prefix = op(Token::Not)
        .or(op(Token::Minus))
        .repeated()
        .foldr(call, |op, rhs| Expression::Prefix {
            operator: op,
            right: Box::new(rhs),
        });

    // Infix operators with precedence.
    let infix = prefix
        .clone()
        .foldl(
            // Precedence level 1: `**`
            op(Token::Power).then(prefix.clone()).repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        )
        .foldl(
            // Precedence level 2: `*`, `/`, `%`
            op(Token::Star)
                .or(op(Token::Slash))
                .or(op(Token::Percent))
                .then(prefix.clone())
                .repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        )
        .foldl(
            // Precedence level 3: `+`, `-`
            op(Token::Plus).or(op(Token::Minus)).then(prefix.clone()).repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        )
        .foldl(
            // Precedence level 4: Comparison operators
            op(Token::Eq)
                .or(op(Token::NotEq))
                .or(op(Token::Lt))
                .or(op(Token::LtEq))
                .or(op(Token::Gt))
                .or(op(Token::GtEq))
                .then(prefix.clone())
                .repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        )
        .foldl(
            // Precedence level 5: `&&`
            op(Token::And).then(prefix.clone()).repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        )
        .foldl(
            // Precedence level 6: `||`
            op(Token::Or).then(prefix).repeated().collect::<Vec<_>>(),
            |lhs, (op, rhs)| Expression::Infix {
                left: Box::new(lhs),
                operator: op,
                right: Box::new(rhs),
            },
        );

    expr.define(infix.labelled("expression"));

    // --- Statements ---

    // A block of statements, enclosed in `{}`.
    block.define(
        stmt.clone()
            .repeated()
            .collect()
            .delimited_by(just(Token::LBrace), just(Token::RBrace))
            .map(|statements| BlockStatement { statements }),
    );

    // `let` statement.
    let let_stmt = just(Token::Let)
        .ignore_then(just(Token::Mut).or_not())
        .then(ident.clone())
        .then_ignore(just(Token::Colon))
        .then(type_parser.clone())
        .then_ignore(just(Token::Assign))
        .then(expr.clone())
        .then_ignore(just(Token::Semicolon))
        .map(|(((mutable, name), value_type), value)| Statement::Let {
            name,
            value_type,
            value,
            mutable: mutable.is_some(),
        })
        .labelled("let statement");

    // `return` statement.
    let return_stmt = just(Token::Return)
        .ignore_then(expr.clone())
        .then_ignore(just(Token::Semicolon))
        .map(|return_value| Statement::Return { return_value })
        .labelled("return statement");
        
    // Named `fn` definition.
    let fn_def_stmt = just(Token::Fn)
        .ignore_then(ident.clone())
        .then(
            ident
                .clone()
                .then_ignore(just(Token::Colon))
                .then(type_parser.clone())
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect::<Vec<_>>()
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        )
        .then_ignore(just(Token::Arrow))
        .then(type_parser.clone())
        .then(
            just(Token::Slash)
                .ignore_then(
                    path.clone()
                        .separated_by(just(Token::Comma))
                        .collect::<Vec<_>>()
                        .delimited_by(just(Token::LBrace), just(Token::RBrace)),
                )
                .or_not(),
        )
        .then(block.clone())
        .map(|((((name, params), return_type), effects), body)| {
            Statement::FnDef {
                decl: FunctionDeclaration {
                    name,
                    params,
                    return_type,
                    effects,
                    body,
                }
            }
        })
        .labelled("function definition");

    // An expression statement.
    let expr_stmt = expr
        .clone()
        .then_ignore(just(Token::Semicolon))
        .map(|expression| Statement::Expression { expression })
        .labelled("expression statement");

    stmt.define(
        let_stmt
            .or(return_stmt)
            .or(fn_def_stmt)
            // Add other statement types here
            .or(expr_stmt), // Must be last, as it's a fallback.
    );

    // The full program is a sequence of statements, ending with an EOF token.
    stmt.repeated()
        .collect::<Vec<_>>()
        .map(|statements| Program { statements })
        .then_ignore(just(Token::Eof).recover_with(skip_then_retry_until([])))
}
