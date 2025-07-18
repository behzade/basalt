// main.rs

// Declare the modules that make up our compiler front-end.
mod token;
mod lexer;
mod ast;
mod parser;

use chumsky::{Parser, Stream};
use std::env;
use std::fs;
use std::io::{self, Read};

fn main() {
    // --- Argument Parsing ---
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [filepath]", args[0]);
        eprintln!("Commands: parse, check, compile");
        return;
    }

    let command = &args[1];
    let filepath = args.get(2);

    // --- Input Reading ---
    let source_code = match filepath {
        Some(path) => match fs::read_to_string(path) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("Error reading file '{}': {}", path, e);
                return;
            }
        },
        None => {
            let mut buffer = String::new();
            if let Err(e) = io::stdin().read_to_string(&mut buffer) {
                eprintln!("Error reading from stdin: {}", e);
                return;
            }
            buffer
        }
    };

    // --- Command Dispatch ---
    match command.as_str() {
        "parse" => parse_code(&source_code),
        "check" => println!("Type checking is not yet implemented."),
        "compile" => println!("Compilation is not yet implemented."),
        _ => {
            eprintln!("Unknown command: '{}'. Use 'parse', 'check', or 'compile'.", command);
        }
    }
}

/// Tokenizes and parses the source code, then prints the resulting AST or errors.
fn parse_code(source: &str) {
    // --- Lexing ---
    let lexer = lexer::Lexer::new(source.to_string());
    let tokens: Vec<_> = lexer.collect(); // Using an iterator implementation on Lexer
    
    // It's useful to print tokens for debugging
    // println!("Tokens: {:?}", tokens);

    // --- Parsing ---
    let token_stream = Stream::from_iter(tokens.into_iter().map(|t| (t, SimpleSpan::new(0,0)))); // Spans are dummy for now
    let parser = parser::parser();
    
    match parser.parse(token_stream) {
        Ok(ast) => {
            println!("Parsing successful!");
            // Pretty-print the AST
            println!("{:#?}", ast); 
        }
        Err(errors) => {
            println!("Parsing failed with {} errors:", errors.len());
            for error in errors {
                println!("- {:?}", error);
            }
        }
    }
}

// To make the lexer an iterator, you would add this to `lexer.rs`:
/*
impl Iterator for Lexer {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        let token = self.next_token();
        if token == Token::Eof {
            None
        } else {
            Some(token)
        }
    }
}
*/
// I've commented this out here so you can add it to the correct file.
// For now, I've changed the code above to use a simple loop.
fn parse_code_without_iterator(source: &str) {
    let mut lexer = lexer::Lexer::new(source.to_string());
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token();
        if token == token::Token::Eof {
            tokens.push((token, SimpleSpan::new(0,0))); // Add EOF for the parser
            break;
        }
        if token == token::Token::Illegal {
            eprintln!("Illegal token found during lexing.");
            // Decide if you want to stop on illegal tokens
        }
        tokens.push((token, SimpleSpan::new(0,0))); // Spans are dummy for now
    }

    let token_stream = Stream::from_iter(tokens.into_iter());
    let parser = parser::parser();

    match parser.parse(token_stream) {
        Ok(ast) => {
            println!("Parsing successful!");
            println!("{:#?}", ast);
        }
        Err(errors) => {
            eprintln!("Parsing failed with {} errors:", errors.len());
            for error in errors {
                eprintln!("- {:?}", error);
            }
        }
    }
}

// Let's adjust `parse_code` to not require the Iterator trait, making this file self-contained.
fn parse_code_final(source: &str) {
    use chumsky::span::SimpleSpan;

    let mut lexer = lexer::Lexer::new(source.to_string());
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token();
        let is_eof = token == token::Token::Eof;
        // We don't care about spans yet, so we'll just use a dummy span.
        tokens.push((token, SimpleSpan::new(0, 0)));
        if is_eof {
            break;
        }
    }

    let token_stream = Stream::from_iter(tokens.into_iter());
    let parser = parser::parser();

    match parser.parse(token_stream) {
        Ok(ast) => {
            println!("--- PARSE SUCCESS ---");
            println!("{:#?}", ast);
        }
        Err(errors) => {
            eprintln!("--- PARSE FAILED ---");
            errors.into_iter().for_each(|e| {
                eprintln!("{}", e);
            });
        }
    }
}

// Re-defining main to use the final version of parse_code
fn main_final() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <command> [filepath]", args[0]);
        eprintln!("Commands: parse, check, compile");
        return;
    }

    let command = &args[1];
    let filepath = args.get(2);

    let source_code = match filepath {
        Some(path) => fs::read_to_string(path).expect(&format!("Failed to read file: {}", path)),
        None => {
            let mut buffer = String::new();
            io::stdin().read_to_string(&mut buffer).expect("Failed to read from stdin");
            buffer
        }
    };

    match command.as_str() {
        "parse" => parse_code_final(&source_code),
        "check" => println!("Type checking is not yet implemented."),
        "compile" => println!("Compilation is not yet implemented."),
        _ => eprintln!("Unknown command: '{}'.", command),
    }
}
