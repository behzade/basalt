use ariadne::{Color, Fmt, Label, Report, ReportKind, Source};
use chumsky::{Parser, error::Rich};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::PathBuf,
};

use crate::{
    ast::ItemNode,
    ast_owned::{OwnedItem, OwnedItemWithSpan, OwnedTypeAliasBody, Spanned},
    hir,
    lexer::lexer,
    parser::file_parser,
    token::{OwnedTokenWithSpan, SimpleSpan, Token},
    typechecker::checker::ModuleCapability,
    typechecker::{TypeError, Typechecker},
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
    // Optional in-memory source overrides (e.g., from LSP unsaved buffers)
    source_overrides: HashMap<PathBuf, String>,
}

#[derive(Default)]
pub struct Workspace {
    tokens: Vec<OwnedTokenWithSpan>,
    // Keep import blocks along with their source file path and span
    imports: Vec<(PathBuf, Spanned<OwnedItem>)>,
    pub sources: HashMap<PathBuf, String>,
    pub ast: HashMap<PathBuf, Vec<Spanned<OwnedItem>>>,
    pub hir: Vec<hir::Item>,
    resolved_modules: HashSet<PathBuf>, // Add this field
    module_capabilities: HashMap<PathBuf, HashSet<ModuleCapability>>,
    pub last_run_result: Option<crate::interpreter::Value>,
}

impl Compiler {
    /// Creates a new Compiler instance.
    pub fn new(path: String, output_path: Option<String>) -> Self {
        Compiler {
            path,
            output_path,
            workspace: Default::default(),
            source_overrides: HashMap::new(),
        }
    }

    /// Provide an in-memory source override for a file path (used by LSP).
    pub fn set_source_override(&mut self, path: PathBuf, text: String) {
        self.source_overrides.insert(path.clone(), text.clone());
        // Keep workspace sources consistent so later stages can read from here if desired
        self.workspace.sources.insert(path, text);
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
        let path_buf: PathBuf = file_to_parse.clone().into();
        // Use override if present
        let source_code = if let Some(override_text) = self.source_overrides.get(&path_buf) {
            override_text.clone()
        } else {
            fs::read_to_string(&file_to_parse)?
        };

        self.workspace
            .sources
            .insert(path_buf.clone(), source_code.clone());

        let (tokens, lex_errs) = lexer().parse(&source_code).into_output_errors();

        // Assuming your error reporter needs the source code
        if self.report_lex_errors(&lex_errs, &source_code, file_to_parse.clone()) {
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

        // Parser now returns Vec<Item> with spans inside
        let (items, parse_errs) = file_parser().parse(&tokens_for_parser).into_output_errors();

        if self.report_parser_errors(
            &parse_errs,
            &tokens_with_spans,
            &source_code,
            file_to_parse.clone(),
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Compilation failed due to parsing errors.",
            ));
        }

        if let Some(items) = items {
            let mut owned_items: Vec<OwnedItemWithSpan> = Vec::new();
            for item in items {
                match &item.node {
                    ItemNode::ImportBlock { .. } => {
                        // Store import block with its source path and span for diagnostics
                        self.workspace.imports.push((
                            PathBuf::from(file_to_parse.clone()),
                            Spanned {
                                item: (&item).into(),
                                span: item.span,
                            },
                        ));
                        // Also keep it in the AST so later passes can see import blocks if needed
                        owned_items.push(Spanned {
                            item: (&item).into(),
                            span: item.span,
                        });
                    }
                    _ => {
                        owned_items.push(Spanned {
                            item: (&item).into(),
                            span: item.span,
                        });
                    }
                }
            }
            let file_ast = self
                .workspace
                .ast
                .entry(file_to_parse.clone().into())
                .or_default();
            file_ast.extend(owned_items);
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

        // Collect resolve-time diagnostics (unresolved imports, name conflicts)
        let mut resolve_errors: Vec<TypeError> = Vec::new();

        while let Some((item_path, item)) = worklist.pop() {
            if let OwnedItem::ImportBlock { imports } = item.item {
                // Precompute union constructor names defined in the same file for conflict checks
                let mut local_union_constructors: HashSet<String> = HashSet::new();
                if let Some(items_in_file) = self.workspace.ast.get(&item_path) {
                    for owned in items_in_file {
                        if let OwnedItem::TypeAlias(ta) = &owned.item {
                            if let OwnedTypeAliasBody::Union(variants) = &ta.aliased {
                                for (vname, _) in variants {
                                    local_union_constructors.insert(vname.clone());
                                }
                            }
                        }
                    }
                }

                for import in imports {
                    let import_name = import
                        .alias
                        .clone()
                        .unwrap_or_else(|| import.path.last().cloned().unwrap_or_default());

                    // Name conflict: import name collides with a local union constructor
                    if local_union_constructors.contains(&import_name) {
                        resolve_errors.push(TypeError {
                            message: format!(
                                "Import name '{}' conflicts with union constructor '{}' in this file",
                                import_name, import_name
                            ),
                            context: crate::typechecker::ItemContext {
                                span: item.span,
                                path: item_path.clone(),
                            },
                        });
                    }

                    if let Some(dir_path) = Compiler::resolve_module_dir_path(&import.path) {
                        if !dir_path.exists() {
                            // Unresolved import path -> diagnostic at the import block span
                            let display_path = import.path.join("/");
                            resolve_errors.push(TypeError {
                                message: format!("Unknown import: {}", display_path),
                                context: crate::typechecker::ItemContext {
                                    span: item.span,
                                    path: item_path.clone(),
                                },
                            });
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

                                let capabilities = self
                                    .workspace
                                    .module_capabilities
                                    .entry(path.clone())
                                    .or_default();
                                if import.path == ["std", "runtime"] {
                                    capabilities.insert(ModuleCapability::MemoryInternals);
                                    capabilities.insert(ModuleCapability::RuntimeInternals);
                                } else if import.path == ["std", "buffer"] {
                                    capabilities.insert(ModuleCapability::MemoryInternals);
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

        // If we collected any errors, report them but do not fail the resolve stage
        if !resolve_errors.is_empty() {
            self.report_type_errors(&resolve_errors);
        }
        Ok(())
    }

    fn run_hir_gen(&mut self) -> io::Result<()> {
        let mut typechecker =
            Typechecker::with_module_capabilities(self.workspace.module_capabilities.clone());
        match typechecker.check_program(self.workspace.ast.clone()) {
            Ok(hir_items) => {
                self.workspace.hir = hir_items;
                Ok(())
            }
            Err(errors) => {
                self.report_type_errors(&errors);
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
        let output = self.output_path.as_deref().unwrap_or("a.out");
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!("build not implemented for output `{}`", output),
        ))
    }

    pub fn run_interpreter(&mut self) -> io::Result<()> {
        let result = crate::interpreter::run_program(&self.workspace.hir)
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e.0))?;
        self.workspace.last_run_result = Some(result);
        Ok(())
    }

    // --- Error Reporting ---

    fn report_lex_errors(
        &self,
        errors: &[Rich<char>],
        source_code: &str,
        filepath: String,
    ) -> bool {
        for e in errors {
            Report::build(ReportKind::Error, &filepath, e.span().start)
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
        filepath: String,
    ) -> bool {
        for e in errors {
            let report_span = if let Some((_, span)) = tokens.get(e.span().start) {
                span.into_range()
            } else {
                let end = source_code.chars().count();
                end..end + 1 // Point to end of file if span is out of bounds
            };

            let report = Report::build(ReportKind::Error, &filepath, report_span.start);
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

    fn report_type_errors(&self, errors: &[TypeError]) -> bool {
        for error in errors {
            // Get the path for this specific error
            let error_path_str = error.context.path.to_str().unwrap_or("?");

            // Find the source code for this error's file in our workspace map
            let source_code = match self.workspace.sources.get(&error.context.path) {
                Some(s) => s,
                None => {
                    // Fallback for the unlikely case we can't find the source
                    eprintln!(
                        "Internal error: Could not find source for path {:?}",
                        error.context.path
                    );
                    continue;
                }
            };

            Report::build(ReportKind::Error, error_path_str, error.context.span.start)
                .with_message("Type error")
                .with_label(
                    Label::new((error_path_str, error.context.span.clone().into_range())) // Use clone here
                        .with_message(&error.message)
                        .with_color(Color::Red),
                )
                .finish()
                // The magic moment! ✨ We provide the correct source code for this specific error.
                .print((error_path_str, Source::from(source_code)))
                .unwrap();
        }
        !errors.is_empty()
    }
}
