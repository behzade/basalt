use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich};
use std::{fs, io};

use crate::{
    ast::{Item, OwnedItem}, codegen::compile_program_to_wasm, hir::HirItem, lexer::lexer, mir::{data::MirItem, MirLowerer}, parser::file_parser, token::{OwnedTokenWithSpan, SimpleSpan, Token}, typechecker::{TypeChecker, TypeError}
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum CompilerStage {
    Parse,
    Hir,
    Mir,
    Build,
}

pub struct Compiler {
    path: String,
    source_code: String,
    output_path: Option<String>,

    workspace: Workspace,
}

#[derive(Default)]
pub struct Workspace {
    tokens: Option<Vec<OwnedTokenWithSpan>>,
    ast: Option<Vec<OwnedItem>>,
    hir: Option<Vec<HirItem>>,
    mir: Option<Vec<MirItem>>,
}

impl Compiler {
    /// Creates a new Compiler instance.
    pub fn new(path: String, source_code: String, output_path: Option<String>) -> Self {
        Compiler {
            path,
            source_code,
            output_path,
            workspace: Default::default(),
        }
    }

    /// Runs the pipeline up to and including the specified target stage.
    pub fn run_until(&mut self, target_stage: CompilerStage) -> io::Result<()> {
        let full_pipeline = [
            CompilerStage::Parse,
            CompilerStage::Hir,
            CompilerStage::Mir,
            CompilerStage::Build,
        ];

        for &stage in &full_pipeline {
            if stage > target_stage {
                break;
            }

            println!("🚀 Executing stage: {:?}", stage);

            match stage {
                CompilerStage::Parse => self.run_parse()?,
                CompilerStage::Hir => self.run_hir_gen()?,
                CompilerStage::Mir => self.run_mir_gen()?,
                CompilerStage::Build => self.run_build()?,
            };
        }
        Ok(())
    }

    // --- Pipeline Stage Implementations ---

    fn run_parse(&mut self) -> io::Result<()> {
        let (tokens, lex_errs) = lexer().parse(&self.source_code).into_output_errors();

        if self.report_lex_errors(&lex_errs) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to lexing errors.",
            ));
        }

        // Safely get the tokens from the Option, or return if none exist.
        let tokens_with_spans = if let Some(t) = tokens {
            t
        } else {
            // No tokens were produced, so we can't parse.
            return Ok(());
        };

        // Create a new Vec for the parser by cloning tokens.
        // This requires `Token` to implement `Clone`.
        let tokens_for_parser: Vec<Token> = tokens_with_spans
            .iter()
            .map(|(tok, _)| tok.clone())
            .collect();

        let (ast, parse_errs) = file_parser()
            .parse(&tokens_for_parser) // Use the new Vec
            .into_output_errors();

        // Report parser errors using the original vector that still has the spans.
        if self.report_parser_errors(&parse_errs, &tokens_with_spans) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to parsing errors.",
            ));
        }

        // Safely print the AST for debugging, only if it exists.
        if let Some(ref ast_val) = ast {
            println!("AST: {:#?}", ast_val);
        }

        // Store the results.
        self.ast = ast;
        self.tokens = Some(tokens_with_spans); // Move the original vector into self.

        Ok(())
    }

    fn run_hir_gen(&mut self) -> io::Result<()> {
        if self.ast.is_none() {
            self.run_parse()?;
        }

        let ast = self.ast.as_ref().unwrap();
        let tokens = self.tokens.as_ref().unwrap();

        match TypeChecker::with_token_spans(tokens.clone()).check_file(ast) {
            Ok(hir_items) => {
                println!("HIR: {:#?}", hir_items);
                self.hir = Some(hir_items);
                Ok(())
            }
            Err(type_errors) => {
                self.report_type_errors(&type_errors);
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Type checking failed.",
                ))
            }
        }
    }

    fn run_mir_gen(&mut self) -> io::Result<()> {
        if self.hir.is_none() {
            self.run_hir_gen()?;
        }

        let hir_items = self.hir.as_ref().unwrap();
        // Assuming MirLowerer::new takes a slice of HirItems.
        let mir_lowerer = MirLowerer::new(hir_items);
        let mir_program = mir_lowerer.lower_to_mir();

        println!("MIR: {:#?}", mir_program);
        self.mir = Some(mir_program);
        Ok(())
    }

    fn run_build(&mut self) -> io::Result<()> {
        if self.mir.is_none() {
            self.run_mir_gen()?;
        }

        let mir_program = self.mir.as_ref().unwrap();
        let hir_items = self.hir.as_ref().unwrap(); // Needed for context in codegen

        let wasm_bytes = compile_program_to_wasm(mir_program, hir_items)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

        println!(
            "✓ Generated WebAssembly module ({} bytes)",
            wasm_bytes.len()
        );

        let output_path = self
            .output_path
            .clone()
            .unwrap_or_else(|| "dist/output.wasm".to_string());
        if let Some(parent) = std::path::Path::new(&output_path).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&output_path, wasm_bytes)?;
        println!("✓ Wrote WebAssembly file to: {}", output_path);

        Ok(())
    }

    // --- Error Reporting ---

    fn report_lex_errors(&self, errors: &[Rich<char>]) -> bool {
        for e in errors {
            Report::build(ReportKind::Error, &self.path, e.span().start)
                .with_message("Lexing error")
                .with_label(
                    Label::new((&self.path, e.span().into_range()))
                        .with_message(format!("Unexpected character: {}", e.reason()))
                        .with_color(Color::Red),
                )
                .finish()
                .print((&self.path, Source::from(&self.source_code)))
                .unwrap();
        }
        !errors.is_empty()
    }

    fn report_parser_errors(&self, errors: &[Rich<Token>], tokens: &[(Token, SimpleSpan)]) -> bool {
        for e in errors {
            let report_span = if let Some((_, span)) = tokens.get(e.span().start) {
                span.into_range()
            } else {
                let end = self.source_code.chars().count();
                end..end + 1 // Point to end of file if span is out of bounds
            };

            let report = Report::build(ReportKind::Error, &self.path, report_span.start);
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
                            Label::new((&self.path, report_span))
                                .with_message(format!(
                                    "Found {} but expected {}",
                                    found_str.fg(Color::Red),
                                    expected_str.fg(Color::Green)
                                ))
                                .with_color(Color::Red),
                        )
                }
                chumsky::error::RichReason::Custom(msg) => report.with_message(msg).with_label(
                    Label::new((&self.path, report_span))
                        .with_message(format!("{}", msg.fg(Color::Red)))
                        .with_color(Color::Red),
                ),
            };
            report
                .finish()
                .print((&self.path, Source::from(&self.source_code)))
                .unwrap();
        }
        !errors.is_empty()
    }

    fn report_type_errors(&self, errors: &[TypeError]) {
        for e in errors {
            let msg = e.to_string(); // Assuming TypeError implements Display
            let span = self.find_error_location(e); // Heuristic to find error location

            Report::build(ReportKind::Error, &self.path, span.start)
                .with_message("Type Error")
                .with_label(
                    Label::new((&self.path, span))
                        .with_message(msg.fg(Color::Red))
                        .with_color(Color::Red),
                )
                .finish()
                .print((&self.path, Source::from(&self.source_code)))
                .unwrap();
        }
    }

    /// Heuristic to find a better location for type errors.
    fn find_error_location(&self, error: &TypeError) -> std::ops::Range<usize> {
        // This is a simplified heuristic. A better approach would involve
        // attaching spans to all AST/HIR nodes during parsing/type-checking.
        let name_to_find = match error {
            TypeError::UnknownVariable(name) => Some(name),
            TypeError::UnknownFunction(name) => Some(name),
            TypeError::UnknownStruct(name) => Some(name),
            TypeError::UnknownEnum(name) => Some(name),
            TypeError::MissingImport { symbol, .. } => Some(symbol),
            _ => None,
        };

        if let Some(name) = name_to_find {
            if let Some(pos) = self.source_code.find(name.to_owned()) {
                return pos..pos + name.len();
            }
        }

        // Fallback to the end of the file.
        let end = self.source_code.chars().count();
        end..end
    }
}
