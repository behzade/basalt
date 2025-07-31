use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich, input::Input};
use std::{collections::HashSet, fs, io, path::PathBuf};

use crate::{
    ast::Item,
    ast_owned::OwnedItem,
    hir::{self, Item as HirItem},
    lexer::lexer,
    parser::file_parser,
    token::{OwnedTokenWithSpan, SimpleSpan, Token},
    typechecker::Typechecker,
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

    pub workspace: Workspace,
}

#[derive(Default)]
pub struct Workspace {
    tokens: Vec<OwnedTokenWithSpan>,
    imports: Vec<OwnedItem>,
    pub ast: Vec<OwnedItem>,
    pub hir: Vec<hir::Item>,
    resolved_modules: HashSet<PathBuf>, // Add this field
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
        let file_to_parse = path.unwrap_or_else(|| self.path.clone());
        let source_code = fs::read_to_string(file_to_parse)?;

        let (tokens, lex_errs) = lexer().parse(&source_code).into_output_errors();

        // Assuming your error reporter needs the source code
        if self.report_lex_errors(&lex_errs, &source_code) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to lexing errors.",
            ));
        }

        let tokens_with_spans = match tokens {
            Some(t) => t,
            None => return Ok(()), // No tokens, nothing to parse
        };

        let tokens_for_parser: Vec<Token> = tokens_with_spans
            .iter()
            .map(|(tok, _)| tok.clone())
            .collect();

        // --- Key Change Here ---
        // Assume file_parser() now returns Option<(Vec<Item>, Vec<Item>)>
        let (items, parse_errs) = file_parser().parse(&tokens_for_parser).into_output_errors();

        if self.report_parser_errors(&parse_errs, &tokens_with_spans, &source_code) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to parsing errors.",
            ));
        }

        // Destructure the tuple and populate the separate workspace fields
        if let Some((import_items, other_ast_items)) = items {
            self.workspace
                .imports
                .extend(import_items.iter().map(OwnedItem::from));
            self.workspace
                .ast
                .extend(other_ast_items.iter().map(OwnedItem::from));
        }

        // Append tokens (this logic remains the same)
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
        // Add the main entry point to resolved modules to prevent self-import cycles
        if let Ok(initial_path) = fs::canonicalize(&self.path) {
            self.workspace.resolved_modules.insert(initial_path);
        }

        let mut worklist = std::mem::take(&mut self.workspace.imports);

        while let Some(item) = worklist.pop() {
            if let OwnedItem::ImportBlock { imports } = item {
                for import in imports {
                    if let Some(dir_path) = Compiler::resolve_module_dir_path(&import.path) {
                        if !dir_path.exists() {
                            continue;
                        }

                        for entry in fs::read_dir(dir_path)? {
                            let path = entry?.path();
                            if path.is_file()
                                && path.extension().and_then(|s| s.to_str()) == Some("bst")
                            {
                                // Use the canonical (absolute) path for reliable duplicate detection
                                let canonical_path = match fs::canonicalize(&path) {
                                    Ok(p) => p,
                                    Err(_) => continue, // Skip if path is invalid
                                };

                                if !self.workspace.resolved_modules.insert(canonical_path) {
                                    continue; // Already in the set, so skip.
                                }

                                // Parse the new file
                                if let Some(path_str) = path.to_str() {
                                    self.run_parse(Some(path_str.to_string()))?;
                                }

                                // Add any newly discovered imports to our worklist for processing
                                worklist.extend(std::mem::take(&mut self.workspace.imports));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn run_hir_gen(&mut self) -> io::Result<()> {
        let mut typechecker = Typechecker::default();
        match typechecker.check_program(self.workspace.ast.clone()) {
            Ok(hir_items) => {
                self.workspace.hir = hir_items;
                Ok(())
            }
            Err(errors) => {
                for error in errors {
                    println!("{}", error);
                }
                Err(io::Error::new(io::ErrorKind::Other, "error in typechecker"))
            }
        }
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
