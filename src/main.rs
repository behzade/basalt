use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::prelude::*;
use chumsky::input::Stream;
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
            report_lexer_errors(&source_code, &source_id, lex_errs);

            if let Some(tokens) = tokens {
                // --- Parsing ---
                let stream = Stream::from_iter(tokens.into_iter().map(|(tok, span)| (tok, span.into_range())));
                let (ast, parse_errs) = file_parser().parse(stream).into_output_errors();

                report_parser_errors(&source_code, &source_id, parse_errs);

                if let Some(ast) = ast {
                    println!("{:#?}", ast);
                }
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

fn report_lexer_errors(source_code: &str, source_id: &str, errors: Vec<Rich<char>>) {
    for e in errors {
        let report = Report::build(ReportKind::Error, source_id, e.span().start);
        let report = match e.reason() {
            chumsky::error::RichReason::Unexpected => report
                .with_message("Unexpected character in input")
                .with_label(
                    Label::new((source_id, e.span()))
                        .with_message(format!(
                            "Unexpected character {}",
                            e.found().unwrap().fg(Color::Red)
                        ))
                        .with_color(Color::Red),
                ),
            chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                Label::new((source_id, e.span()))
                    .with_message(format!("{}", msg.fg(Color::Red)))
                    .with_color(Color::Red),
            ),
            _ => report.with_message("Unknown lexer error"), // Should not happen with current lexer
        };
        report.finish().print((source_id, Source::from(source_code))).unwrap();
    }
}

fn report_parser_errors(source_code: &str, source_id: &str, errors: Vec<Rich<Token>>) {
    for e in errors {
        let report = Report::build(ReportKind::Error, source_id, e.span().start);
        let report = match e.reason() {
            chumsky::error::RichReason::ExpectedFound { expected, found } => {
                let expected_str = expected.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                let found_str = found.as_ref().map(|f| f.to_string()).unwrap_or_else(|| "end of input".to_string());
                report
                    .with_message(format!("Unexpected token, expected {}", expected_str))
                    .with_label(
                        Label::new((source_id, e.span()))
                            .with_message(format!("Found {} but expected {}", found_str.fg(Color::Red), expected_str.fg(Color::Green)))
                            .with_color(Color::Red),
                    )
            },
            chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                Label::new((source_id, e.span()))
                    .with_message(format!("{}", msg.fg(Color::Red)))
                    .with_color(Color::Red),
            ),
        };
        report.finish().print((source_id, Source::from(source_code))).unwrap();
    }
}
