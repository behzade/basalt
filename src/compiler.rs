use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich, input::Input};
use std::{fs, io, path::PathBuf};

use crate::{
    ast::Item,
    ast_owned::OwnedItem,
    hir::{self, Item as HirItem},
    lexer::lexer,
    parser::file_parser,
    token::{OwnedTokenWithSpan, SimpleSpan, Token},
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum CompilerStage {
    Parse,
    Resolve,
    Hir,
    Mir,
    Build,
}

pub struct Compiler {
    path: String,
    output_path: Option<String>,

    workspace: Workspace,
}

#[derive(Default)]
pub struct Workspace {
    tokens: Vec<OwnedTokenWithSpan>,
    imports: Vec<OwnedItem>,
    ast: Vec<OwnedItem>,
    hir: Vec<hir::Item>,
}

impl Compiler {
    /// Creates a new Compiler instance.
    pub fn new(path: String, output_path: Option<String>) -> Self {
        Compiler {
            path,
            output_path,
            workspace: Default::default(),
        }
    }

    /// Runs the pipeline up to and including the specified target stage.
    pub fn run_until(&mut self, target_stage: CompilerStage) -> io::Result<()> {
        let full_pipeline = [
            CompilerStage::Parse,
            CompilerStage::Resolve,
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
                CompilerStage::Parse => self.run_parse(None)?,
                CompilerStage::Resolve => self.run_resolve()?,
                CompilerStage::Hir => self.run_hir_gen()?,
                CompilerStage::Mir => self.run_mir_gen()?,
                CompilerStage::Build => self.run_build()?,
            };
        }
        Ok(())
    }

    // --- Pipeline Stage Implementations ---

    // if path is Some, it is the path to the file to be parsed
    fn run_parse(&mut self, path: Option<String>) -> io::Result<()> {
        let source_code: String;
        if let Some(path) = path {
            source_code = fs::read_to_string(path)?;
        } else {
            source_code = fs::read_to_string(&self.path)?;
        }

        let (tokens, lex_errs) = lexer().parse(&source_code).into_output_errors();

        if self.report_lex_errors(&lex_errs, &source_code) {
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
        if self.report_parser_errors(&parse_errs, &tokens_with_spans, &source_code) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to parsing errors.",
            ));
        }

        if let Some(new_ast_items) = ast {
            let owned_items = new_ast_items.iter().map(OwnedItem::from);
            self.workspace.ast.extend(owned_items);
        }

        // Append the new tokens to the workspace.
        let owned_tokens = tokens_with_spans
            .into_iter()
            .map(|(tok, span)| (tok.into(), span.into()));
        self.workspace.tokens.extend(owned_tokens);

        Ok(())
    }

    /// Resolves an import path like `["Self", "Util"]` to a directory path like `./src/util`.
    fn resolve_module_dir_path(path: &[String]) -> Option<PathBuf> {
        if path.is_empty() {
            return None;
        }

        // Determine the root directory and the parts of the path to join.
        let (root_dir, path_segments) = if path[0].eq_ignore_ascii_case("self") {
            // If it starts with "self", the root is "./src" and we skip "self".
            ("./src", &path[1..])
        } else {
            // Otherwise, the root is "./modules" and we use the full path.
            ("./modules", &path[..])
        };

        let mut final_path = PathBuf::from(root_dir);

        // Append each segment, lowercased, to the path.
        for segment in path_segments {
            final_path.push(segment.to_lowercase());
        }

        Some(final_path)
    }

    fn run_resolve(&mut self) -> io::Result<()> {
        // 1. Collect import items into a new owned Vec, dropping the borrow on `self.workspace`
        let import_blocks: Vec<OwnedItem> = self
            .workspace
            .ast
            .iter()
            .filter(|item| matches!(item, OwnedItem::ImportBlock { .. }))
            .cloned() // Use cloned() to get owned OwnedItem values
            .collect();

        // 2. Iterate over the new Vec. Now you can safely call `&mut self` methods.
        for item in &import_blocks {
            if let OwnedItem::ImportBlock { imports } = item {
                for import in imports {
                    if let Some(dir_path) = Compiler::resolve_module_dir_path(&import.path) {
                        for entry in fs::read_dir(dir_path)? {
                            let entry = entry?;
                            let path = entry.path();
                            if path.is_file()
                                && path.extension().and_then(|s| s.to_str()) == Some("bst")
                            {
                                if let Some(path_str) = path.to_str() {
                                    self.run_parse(Some(path_str.to_string()))?;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Remove the processed import blocks from the workspace AST
        self.workspace
            .ast
            .retain(|item| !matches!(item, OwnedItem::ImportBlock { .. }));

        Ok(())
    }

    fn run_hir_gen(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "hir_gen not implemented",
        ))
    }

    fn run_mir_gen(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "mir_gen not implemented",
        ))
    }

    fn run_build(&mut self) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "build not implemented",
        ))
    }

    // --- Error Reporting ---

    fn report_lex_errors(&self, errors: &[Rich<char>], source_code: &str) -> bool {
        for e in errors {
            Report::build(ReportKind::Error, &self.path, e.span().start)
                .with_message("Lexing error")
                .with_label(
                    Label::new((&self.path, e.span().into_range()))
                        .with_message(format!("Unexpected character: {}", e.reason()))
                        .with_color(Color::Red),
                )
                .finish()
                .print((&self.path, Source::from(source_code)))
                .unwrap();
        }
        !errors.is_empty()
    }

    fn report_parser_errors(
        &self,
        errors: &[Rich<Token>],
        tokens: &[(Token, SimpleSpan)],
        source_code: &str,
    ) -> bool {
        for e in errors {
            let report_span = if let Some((_, span)) = tokens.get(e.span().start) {
                span.into_range()
            } else {
                let end = source_code.chars().count();
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
                .print((&self.path, Source::from(source_code)))
                .unwrap();
        }
        !errors.is_empty()
    }
}
