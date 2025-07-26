use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::prelude::*;
use clap::Parser as ClapParser;
use std::fs;
use std::io::{self, Read};

// --- Module Declarations ---
mod ast;
mod codegen;
mod hir;
mod lexer;
mod mir;
mod parser;
mod project;
mod token;
mod typechecker;

use crate::{
    codegen::compile_program_to_wasm,
    lexer::lexer,
    mir::MirLowerer,
    parser::file_parser,
    project::ProjectLoader,
    token::Token,
    typechecker::{TypeChecker, TypeError},
};

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
    /// Type-check and generate HIR
    Hir {
        /// The path to the file to process. If not provided, reads from stdin.
        path: Option<String>,
    },
    /// Generate MIR from HIR
    Mir {
        /// The path to the file to process. If not provided, reads from stdin.
        path: Option<String>,
    },
    /// Compile to WebAssembly (.wasm file)
    Build {
        /// The path to the file to process. If not provided, reads from stdin.
        path: Option<String>,
        /// Output file path for the .wasm file (defaults to dist/output.wasm)
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    match cli.action {
        Action::Parse { path } => {
            let (source_id, source_code) = read_source(path)?;
            let (tokens, lex_errs) = lexer().parse(&source_code).into_output_errors();

            if report_errors(&source_code, &source_id, &lex_errs, |e| {
                (
                    e.span().into_range(),
                    format!("Unexpected character: {}", e.reason()),
                )
            }) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Lexing errors occurred",
                ));
            }

            if let Some(tokens) = tokens {
                let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();
                let (ast, parse_errs) = file_parser().parse(&token_slice).into_output_errors();

                if report_parser_errors(&source_code, &source_id, &parse_errs, &tokens) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Parsing errors occurred",
                    ));
                }

                if let Some(ast) = ast {
                    println!("{:#?}", ast);
                }
            }
        }
        Action::Hir { path } => {
            let (source_id, source_code) = read_source(path)?;
            run_type_checker(&source_code, &source_id)?;
        }
        Action::Mir { path } => {
            let (source_id, source_code) = read_source(path)?;
            run_mir_lowering(&source_code, &source_id)?;
        }
        Action::Build { path, output } => {
            let entry_path = path.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Build command requires a file path",
                )
            })?;
            run_wasm_compilation(&entry_path, output)?;
        }
    }

    Ok(())
}

/// A helper function to encapsulate the full lex, parse, and type-check pipeline.
fn run_type_checker(source_code: &str, source_id: &str) -> io::Result<()> {
    // --- Lexing ---
    let (tokens, lex_errs) = lexer().parse(source_code).into_output_errors();
    if report_errors(source_code, source_id, &lex_errs, |e| {
        (
            e.span().into_range(),
            format!("Unexpected character: {}", e.reason()),
        )
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Lexing errors occurred",
        ));
    }

    // --- Parsing ---
    let tokens = tokens.unwrap(); // Safe to unwrap due to check above
    let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();
    let (ast, parse_errs) = file_parser().parse(&token_slice).into_output_errors();
    if report_parser_errors(source_code, source_id, &parse_errs, &tokens) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Parsing errors occurred",
        ));
    }

    // --- Type Checking ---
    if let Some(ast) = ast {
        match TypeChecker::with_token_spans(tokens).check_file(&ast) {
            Ok(hir_items) => {
                // Always print the typed HIR for snapshot testing compatibility
                for item in hir_items {
                    println!("{:#?}", item);
                }
            }
            Err(type_errors) => {
                report_type_errors(source_code, source_id, &type_errors);
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Type checking errors occurred",
                ));
            }
        }
    }

    Ok(())
}

/// Runs the MIR lowering process on the given source code.
fn run_mir_lowering(source_code: &str, source_id: &str) -> io::Result<()> {
    // First, lex and parse the source code
    let (tokens, lex_errs) = lexer().parse(source_code).into_output_errors();

    if report_errors(source_code, source_id, &lex_errs, |e| {
        (
            e.span().into_range(),
            format!("Unexpected character: {}", e.reason()),
        )
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Lexing errors occurred",
        ));
    }

    if let Some(tokens) = tokens {
        let token_slice: Vec<_> = tokens.iter().map(|(tok, _)| tok.clone()).collect();
        let (ast, parse_errs) = file_parser().parse(&token_slice).into_output_errors();

        if report_parser_errors(source_code, source_id, &parse_errs, &tokens) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Parsing errors occurred",
            ));
        }

        if let Some(ast) = ast {
            // Type check to get HIR
            let type_checker = TypeChecker::with_token_spans(tokens);
            match type_checker.check_file(&ast) {
                Ok(hir_items) => {
                    // Lower HIR to MIR
                    let mir_lowerer = MirLowerer::new(&hir_items);
                    let mir_program = mir_lowerer.lower_to_mir();

                    // Print the MIR representation
                    println!("{:#?}", mir_program);
                }
                Err(type_errors) => {
                    report_type_errors(source_code, source_id, &type_errors);
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Type checking errors occurred",
                    ));
                }
            }
        }
    }

    Ok(())
}

/// Runs the WebAssembly compilation process on the given source code.
fn run_wasm_compilation(entry_path: &str, output: Option<String>) -> io::Result<()> {
    // Load the complete project starting from the entry point
    let project = ProjectLoader::load(entry_path)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    println!(
        "✓ Loaded project with {} items from {} files",
        project.items.len(),
        project.file_items.len()
    );

    // Type check to get HIR
    let type_checker = TypeChecker::new();
    match type_checker.check_file(&project.items) {
        Ok(hir_items) => {
            // Lower HIR to MIR
            let mir_lowerer = MirLowerer::new(&hir_items);
            let mir_program = mir_lowerer.lower_to_mir();

            // Compile MIR directly to Wasm using our backend
            match compile_program_to_wasm(&mir_program) {
                Ok(wasm_bytes) => {
                    println!(
                        "✓ Generated WebAssembly module ({} bytes)",
                        wasm_bytes.len()
                    );

                    // Determine output file path (default to dist/output.wasm)
                    let output_path = output.unwrap_or_else(|| "dist/output.wasm".to_string());

                    // Ensure dist directory exists
                    if let Some(parent) = std::path::Path::new(&output_path).parent() {
                        if !parent.exists() {
                            std::fs::create_dir_all(parent)?;
                        }
                    }

                    // Write Wasm file to disk
                    match std::fs::write(&output_path, wasm_bytes) {
                        Ok(_) => println!("✓ Wrote WebAssembly file to: {}", output_path),
                        Err(e) => {
                            eprintln!("Error writing WebAssembly file: {}", e);
                            return Err(io::Error::new(
                                io::ErrorKind::Other,
                                format!("Failed to write WebAssembly file: {}", e),
                            ));
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error generating WebAssembly: {}", e);
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        format!("Failed to generate WebAssembly: {}", e),
                    ));
                }
            }
        }
        Err(type_errors) => {
            // For now, we'll report errors without source context since we have multiple files
            eprintln!("Type checking errors occurred:");
            for error in &type_errors {
                eprintln!("  {}", error);
            }
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Type checking errors occurred",
            ));
        }
    }

    Ok(())
}

/// Reads source code from a file path or from stdin.
fn read_source(path: Option<String>) -> io::Result<(String, String)> {
    match path {
        Some(path) => Ok((path.clone(), fs::read_to_string(&path)?)),
        None => {
            let mut buf = String::new();
            io::stdin().read_to_string(&mut buf)?;
            Ok(("stdin".to_string(), buf))
        }
    }
}

/// A generic error reporting function for lexer errors.
fn report_errors<T: std::fmt::Display>(
    source_code: &str,
    source_id: &str,
    errors: &[Rich<T>],
    map_fn: impl Fn(&Rich<T>) -> (std::ops::Range<usize>, String),
) -> bool {
    for e in errors {
        let (span, msg) = map_fn(e);
        Report::build(ReportKind::Error, source_id, span.start)
            .with_message("Lexing error")
            .with_label(
                Label::new((source_id, span))
                    .with_message(msg)
                    .with_color(Color::Red),
            )
            .finish()
            .print((source_id, Source::from(source_code)))
            .unwrap();
    }
    !errors.is_empty()
}

/// A dedicated error reporting function for parser errors.
fn report_parser_errors(
    source_code: &str,
    source_id: &str,
    errors: &[Rich<Token>],
    tokens: &[(Token, SimpleSpan)],
) -> bool {
    for e in errors {
        let report_span = if let Some((_, span)) = tokens.get(e.span().start) {
            span.into_range()
        } else {
            let end = source_code.chars().count();
            end..end + 1
        };

        let report = Report::build(ReportKind::Error, source_id, report_span.start);
        let report = match e.reason() {
            chumsky::error::RichReason::ExpectedFound { expected, found } => {
                let expected_str = expected
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(" or ");
                let found_str = found
                    .as_ref()
                    .map(|f| f.to_string())
                    .unwrap_or_else(|| "end of input".to_string());
                report
                    .with_message(format!("Unexpected token, expected {}", expected_str))
                    .with_label(
                        Label::new((source_id, report_span))
                            .with_message(format!(
                                "Found {} but expected {}",
                                found_str.fg(Color::Red),
                                expected_str.fg(Color::Green)
                            ))
                            .with_color(Color::Red),
                    )
            }
            chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                Label::new((source_id, report_span))
                    .with_message(format!("{}", msg.fg(Color::Red)))
                    .with_color(Color::Red),
            ),
        };
        report
            .finish()
            .print((source_id, Source::from(source_code)))
            .unwrap();
    }
    !errors.is_empty()
}

/// A new, dedicated error reporting function for our structured type errors.
fn report_type_errors(source_code: &str, source_id: &str, errors: &[TypeError]) {
    for e in errors {
        // Try to find a better location based on the error content
        let span = find_error_location(source_code, e);

        let msg = match e {
            &TypeError::MismatchedTypes {
                ref expected,
                ref found,
            } => {
                format!(
                    "Mismatched types: expected `{}`, found `{}`",
                    expected, found
                )
            }
            &TypeError::UnknownVariable(name) => {
                format!("Unknown variable: `{}`", name)
            }
            &TypeError::UnknownFunction(name) => {
                format!("Unknown function: `{}`", name)
            }
            &TypeError::UnknownStruct(name) => {
                format!("Unknown struct: `{}`", name)
            }
            &TypeError::UnknownEnum(name) => {
                format!("Unknown enum: `{}`", name)
            }
            &TypeError::UnknownEnumVariant {
                enum_name,
                variant_name,
            } => {
                format!("Unknown variant `{}` in enum `{}`", variant_name, enum_name)
            }
            &TypeError::WrongArgumentCount { expected, found } => {
                format!(
                    "Wrong number of arguments: expected {}, found {}",
                    expected, found
                )
            }
            &TypeError::WrongNumberOfArguments { expected, found } => {
                format!(
                    "Wrong number of arguments: expected {}, found {}",
                    expected, found
                )
            }
            &TypeError::WrongArgumentType {
                ref expected,
                ref found,
            } => {
                format!(
                    "Wrong argument type: expected `{}`, found `{}`",
                    expected, found
                )
            }
            &TypeError::UnknownStructField {
                struct_name,
                field_name,
            } => {
                format!("Unknown struct field: `{}.{}`", struct_name, field_name)
            }
            &TypeError::MissingStructField {
                struct_name,
                field_name,
            } => {
                format!("Missing struct field: `{}.{}`", struct_name, field_name)
            }
            &TypeError::InvalidOperator { ref op, ref ty } => {
                format!("Cannot apply operator `{}` to type `{}`", op, ty)
            }
            &TypeError::InvalidPattern { ref pattern } => {
                format!("Invalid pattern: {}", pattern)
            }
            &TypeError::UnificationError(ref ty1, ref ty2) => {
                format!("Cannot unify types `{}` and `{}`", ty1, ty2)
            }
            &TypeError::UnknownModule { namespace, module } => {
                format!("Unknown module: `{}::{}`", namespace, module)
            }
            &TypeError::UnknownModuleSymbol {
                namespace,
                module,
                symbol,
            } => {
                format!(
                    "Unknown symbol `{}` in module `{}::{}`",
                    symbol, namespace, module
                )
            }
            &TypeError::MissingImport {
                symbol,
                ref suggested_import,
            } => {
                if let Some(suggestion) = suggested_import {
                    format!(
                        "Unknown symbol `{}`. Try importing it: {}",
                        symbol, suggestion
                    )
                } else {
                    format!("Unknown symbol `{}`", symbol)
                }
            }
            &TypeError::LiteralOverflow {
                value,
                ref target_type,
            } => {
                format!(
                    "Literal `{}` overflows target type `{}`",
                    value, target_type
                )
            }
        };

        Report::build(ReportKind::Error, source_id, span.start)
            .with_message("Type Error")
            .with_label(
                Label::new((source_id, span))
                    .with_message(msg.fg(Color::Red))
                    .with_color(Color::Red),
            )
            .finish()
            .print((source_id, Source::from(source_code)))
            .unwrap();
    }
}

/// Find a better location for type errors by searching for relevant tokens
fn find_error_location(source_code: &str, error: &TypeError) -> std::ops::Range<usize> {
    match error {
        TypeError::UnknownVariable(name) => {
            // Find the variable name in the source code
            if let Some(pos) = source_code.find(name) {
                pos..pos + name.len()
            } else {
                // Fallback to middle of file
                let source_len = source_code.chars().count();
                let default_pos = source_len / 2;
                default_pos..default_pos + 1
            }
        }
        TypeError::MissingImport { symbol, .. } => {
            // Look for the symbol name in the source code
            if let Some(pos) = source_code.find(symbol) {
                pos..pos + symbol.len()
            } else {
                // Fallback to middle of file
                let source_len = source_code.chars().count();
                let default_pos = source_len / 2;
                default_pos..default_pos + 1
            }
        }
        TypeError::UnknownModule { namespace, module } => {
            // Look for the module path in the source code
            let module_path = format!("{}::{}", namespace, module);
            if let Some(pos) = source_code.find(&module_path) {
                pos..pos + module_path.len()
            } else {
                // Fallback to middle of file
                let source_len = source_code.chars().count();
                let default_pos = source_len / 2;
                default_pos..default_pos + 1
            }
        }
        TypeError::UnknownModuleSymbol {
            namespace,
            module,
            symbol,
        } => {
            // Look for the full path in the source code
            let full_path = format!("{}::{}::{}", namespace, module, symbol);
            if let Some(pos) = source_code.find(&full_path) {
                pos..pos + full_path.len()
            } else {
                // Fallback to middle of file
                let source_len = source_code.chars().count();
                let default_pos = source_len / 2;
                default_pos..default_pos + 1
            }
        }
        TypeError::MismatchedTypes { .. } => {
            // Look for common type error patterns
            if let Some(pos) = source_code.find("=") {
                pos..pos + 1
            } else if let Some(pos) = source_code.find("+") {
                pos..pos + 1
            } else if let Some(pos) = source_code.find("-") {
                pos..pos + 1
            } else {
                // Fallback to middle of file
                let source_len = source_code.chars().count();
                let default_pos = source_len / 2;
                default_pos..default_pos + 1
            }
        }
        _ => {
            // For other errors, try to find relevant tokens
            let source_len = source_code.chars().count();
            let default_pos = source_len / 2;
            default_pos..default_pos + 1
        }
    }
}
