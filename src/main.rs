// main.rs (or wherever you call the parser)

use chumsky::{Parser};

mod lexer;
mod token;
mod ast;
mod parser;

fn main() {
    let source_code = r#"
        let x: i64 = 5 + (3 * 2);
        let y: bool = true;
    "#;

    // 1. Lexing to create Vec<(Token, Span)>
    let parse_result = lexer::lexer().parse(source_code).unwrap();

    // 2. Create a chumsky::Stream
    // let token_stream = Stream::from_iter(tokens_with_spans.into_iter());

    for token in parse_result.into_iter() {
        println!("{:?}", token);
    }
}
