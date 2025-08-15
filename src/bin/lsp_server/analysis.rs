use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use chumsky::Parser;
use chumsky::span::Span;
use tower_lsp::lsp_types as lsp;

use basalt::ast;
use basalt::ast_owned;
use basalt::hir;
use basalt::compiler::{Compiler, CompilerStage};
use basalt::lexer;
use basalt::parser;
use basalt::token;
use basalt::typechecker::{TypeError, Typechecker};

use basalt::parser::file_parser;
use basalt::lexer::lexer;
use basalt::token::{SimpleSpan, Token};

#[derive(Default, Clone)]
pub struct AnalysisResult {
    pub sources: HashMap<PathBuf, String>,
    pub hir: Vec<hir::Item>,
    pub diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>>,
    pub token_spans: HashMap<PathBuf, Vec<SimpleSpan>>, // per-file token spans
    pub contexts: Vec<hir::HirContext>,
}

pub struct Analyzer;

impl Analyzer {
    pub fn analyze(entry_path: &Path, entry_text: &str) -> AnalysisResult {
        let mut sources: HashMap<PathBuf, String> = HashMap::new();
        let mut diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>> = HashMap::new();
        let mut token_spans_map: HashMap<PathBuf, Vec<SimpleSpan>> = HashMap::new();

        let entry_str = entry_path.to_string_lossy().to_string();
        let mut compiler = Compiler::new(entry_str.clone(), None);
        compiler.set_source_override(entry_path.to_path_buf(), entry_text.to_string());
        let _ = compiler.run_until(CompilerStage::Resolve);

        for (p, text) in compiler.workspace.sources.clone() { sources.insert(p.clone(), text.clone()); }

        for (path, text) in &sources {
            let (tokens, lex_errs) = lexer().parse(text).into_output_errors();
            for e in lex_errs {
                let range = lsp::Range { start: super::symbols::offset_to_position(text, e.span().start), end: super::symbols::offset_to_position(text, e.span().end) };
                super::symbols::diagnostics_push(&mut diagnostics, path, lsp::Diagnostic { range, severity: Some(lsp::DiagnosticSeverity::ERROR), source: Some("basalt-lexer".to_string()), message: format!("Lexing error: {}", e.reason()), ..Default::default() });
            }
            let tokens_with_spans: Vec<(Token, SimpleSpan)> = tokens.unwrap_or_default();
            let owned_tokens: Vec<(Token<'static>, SimpleSpan)> = tokens_with_spans
                .into_iter()
                .map(|(tok, sp)| (unsafe { std::mem::transmute::<Token<'_>, Token<'static>>(tok) }, sp))
                .collect();
            let tokens_for_parser: Vec<Token<'static>> = owned_tokens.iter().map(|(t, _)| t.clone()).collect();
            let spans_only: Vec<SimpleSpan> = owned_tokens.iter().map(|(_, s)| *s).collect();
            token_spans_map.insert(path.clone(), spans_only.clone());
            let (_items, parse_errs) = file_parser().parse(&tokens_for_parser).into_output_errors();
            for e in parse_errs {
                let tok_span = SimpleSpan::new((), e.span().start..e.span().end);
                let range = super::symbols::token_index_span_to_range(text, &spans_only, tok_span);
                super::symbols::diagnostics_push(&mut diagnostics, path, lsp::Diagnostic { range, severity: Some(lsp::DiagnosticSeverity::ERROR), source: Some("basalt-parser".to_string()), message: format!("Parse error: {}", e.to_string()), ..Default::default() });
            }
        }

        let mut typechecker = Typechecker::default();
        let hir = match typechecker.check_program(compiler.workspace.ast.clone()) {
            Ok(hir_items) => hir_items,
            Err(errors) => {
                for TypeError { message, context } in errors {
                    if let Some(text) = sources.get(&context.path) {
                        let range = if let Some(tok_spans) = token_spans_map.get(&context.path) {
                            super::symbols::token_index_span_to_range(text, tok_spans, context.span)
                        } else {
                            super::symbols::simple_span_to_range(text, context.span)
                        };
                        super::symbols::diagnostics_push(&mut diagnostics, &context.path, lsp::Diagnostic { range, severity: Some(lsp::DiagnosticSeverity::ERROR), source: Some("basalt-typechecker".to_string()), message, ..Default::default() });
                    }
                }
                Vec::new()
            }
        };

        AnalysisResult { sources, hir, diagnostics, token_spans: token_spans_map, contexts: typechecker.contexts.clone() }
    }
}


