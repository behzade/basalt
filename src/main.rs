use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::prelude::*;
use clap::Parser as ClapParser;
use std::fs;
use std::io::{self, Read};

mod ast;
mod lexer;
mod parser;
mod token;
// This assumes your lib is named 'basalt'. If it's different, change this line.
use crate::{lexer::lexer, parser::file_parser, token::Token};

#[derive(ClapParser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand)]
enum Action {
    /// Parse a file and print the AST
    Parse {
        /// The path to the file to parse. If not provided, reads from stdin.
        path: Option<String>,
    },
    /// (Coming soon) Type-check a file
    TypeCheck { path: String },
    /// (Coming soon) Compile a file
    Compile { path: String },
}

fn main() -> io::Result<()> {
    // FIX: Changed `Cli.parse()` to the correct `Cli::parse()` for the associated function.
    let cli = Cli::parse();

    match cli.action {
        Action::Parse { path } => {
            let (source_id, source_code) = match path {
                Some(path) => (path.clone(), fs::read_to_string(&path)?),
                None => {
                    let mut buf = String::new();
                    io::stdin().read_to_string(&mut buf)?;
                    ("stdin".to_string(), buf)
                }
            };

            // --- Lexing ---
            let (tokens, lex_errs) = lexer().parse(&source_code).into_output_errors();
            report_lexer_errors(&source_code, &source_id, &lex_errs);

            // If there are lexing errors, exit with error code
            if !lex_errs.is_empty() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Lexing errors occurred"));
            }

            if let Some(tokens) = tokens {
                // --- Parsing ---
                // FIX: The parser expects a slice `&[Token]`, so we pass that directly
                // instead of creating a `Stream`.
                let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();
                let (ast, parse_errs) = file_parser().parse(&token_slice).into_output_errors();

                report_parser_errors(&source_code, &source_id, &parse_errs, &tokens);

                // If there are parsing errors, exit with error code
                if !parse_errs.is_empty() {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Parsing errors occurred"));
                }

                if let Some(ast) = ast {
                    println!("{:#?}", ast);
                } else {
                    // If no AST was produced, exit with error code
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "Failed to parse input"));
                }
            } else {
                // If no tokens were produced, exit with error code
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Failed to tokenize input"));
            }
        }
        Action::TypeCheck { .. } => {
            println!("Type checking is not yet implemented.");
        }
        Action::Compile { .. } => {
            println!("Compilation is not yet implemented.");
        }
    }

    Ok(())
}

fn report_lexer_errors(source_code: &str, source_id: &str, errors: &[Rich<char>]) {
    for e in errors {
        let report = Report::build(ReportKind::Error, source_id, e.span().start);
        
        let report = match e.reason() {
            chumsky::error::RichReason::ExpectedFound { found, .. } => report
                .with_message("Unexpected character in input")
                .with_label(
                    Label::new((source_id, e.span().into_range()))
                        .with_message(format!(
                            "Unexpected character {}",
                            found.map(|c| c.to_string()).unwrap_or_else(|| "end of file".to_string()).fg(Color::Red)
                        ))
                        .with_color(Color::Red),
                ),
            chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                Label::new((source_id, e.span().into_range()))
                    .with_message(format!("{}", msg.fg(Color::Red)))
                    .with_color(Color::Red),
            ),
            // FIX: Removed unreachable `_` pattern. The two patterns above cover all variants of `RichReason`.
        };
        report.finish().print((source_id, Source::from(source_code))).unwrap();
    }
}

fn report_parser_errors(source_code: &str, source_id: &str, errors: &[Rich<Token>], tokens_with_spans: &[(Token, chumsky::span::SimpleSpan)]) {
    for e in errors {
        let report_span = if let Some((_, span)) = tokens_with_spans.get(e.span().start) {
            span.into_range()
        } else {
            let end = source_code.chars().count();
            end..end+1
        };

        let report = Report::build(ReportKind::Error, source_id, report_span.start);
        
        let report = match e.reason() {
            chumsky::error::RichReason::ExpectedFound { expected, found } => {
                let expected_str = expected.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(" or ");
                let found_str = found.as_ref().map(|f| f.to_string()).unwrap_or_else(|| "end of input".to_string());
                report
                    .with_message(format!("Unexpected token, expected {}", expected_str))
                    .with_label(
                        Label::new((source_id, report_span))
                            .with_message(format!("Found {} but expected {}", found_str.fg(Color::Red), expected_str.fg(Color::Green)))
                            .with_color(Color::Red),
                    )
            },
            chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                Label::new((source_id, report_span))
                    .with_message(format!("{}", msg.fg(Color::Red)))
                    .with_color(Color::Red),
            ),
            // FIX: Removed unreachable `_` pattern.
        };
        report.finish().print((source_id, Source::from(source_code))).unwrap();
    }
}
