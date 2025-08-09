use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use parking_lot::RwLock;
use chumsky::Parser;
use chumsky::span::Span;
use tokio::sync::Mutex;
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;
use tower_lsp::{Client, LanguageServer, LspService, Server};

// Bring compiler modules into this binary. These module files live in src/ and
// are included into this compilation unit just like in main.rs.
use basalt::ast;
use basalt::ast_owned;
use basalt::hir;
use basalt::lexer;
use basalt::parser;
use basalt::token;
use basalt::type_unifier;
use basalt::typechecker;

use ast_owned::{OwnedItem, OwnedItemWithSpan, OwnedTypeAliasBody, Spanned as OwnedSpanned};
use hir::{HirFunctionSignature, Item as HirItem};
use lexer::lexer;
use parser::file_parser;
use token::{SimpleSpan, Token};
use typechecker::{TypeError, Typechecker};

#[derive(Clone, Debug)]
struct DefInfo {
    path: PathBuf,
    span: SimpleSpan,
    signature: Option<HirFunctionSignature>,
}

#[derive(Default, Clone)]
struct TopLevelIndex {
    functions: HashMap<String, DefInfo>,
    type_aliases: HashMap<String, (PathBuf, SimpleSpan)>,
    union_variants: HashMap<String, (PathBuf, SimpleSpan)>,
}

#[derive(Default, Clone)]
struct AnalysisResult {
    sources: HashMap<PathBuf, String>,
    ast: HashMap<PathBuf, Vec<OwnedSpanned<OwnedItem>>>,
    hir: Vec<HirItem>,
    diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>>,
    index: TopLevelIndex,
    // Per-file token spans from the lexer (character ranges), used to translate
    // token-index spans produced by the parser/typechecker into character ranges
    token_spans: HashMap<PathBuf, Vec<SimpleSpan>>, 
}

struct Backend {
    client: Client,
    // Latest open text per file
    open_files: RwLock<HashMap<PathBuf, String>>,
    // Last analysis per root file
    analysis: RwLock<HashMap<PathBuf, AnalysisResult>>,
    // Serialize analyses to avoid races
    analyze_lock: Mutex<()>,
}

impl Backend {
    fn new(client: Client) -> Self {
        Self {
            client,
            open_files: RwLock::new(HashMap::new()),
            analysis: RwLock::new(HashMap::new()),
            analyze_lock: Mutex::new(()),
        }
    }

    fn url_to_path(url: &lsp::Url) -> Option<PathBuf> {
        url.to_file_path().ok()
    }

    fn simple_span_to_range(text: &str, span: SimpleSpan) -> lsp::Range {
        let start = Self::offset_to_position(text, span.start);
        let end = Self::offset_to_position(text, span.end);
        lsp::Range { start, end }
    }

    fn token_index_span_to_range(
        text: &str,
        token_spans: &[SimpleSpan],
        token_span: SimpleSpan,
    ) -> lsp::Range {
        // token_span.start/end are token indices; map to char offsets via token_spans
        if token_spans.is_empty() {
            return lsp::Range {
                start: lsp::Position { line: 0, character: 0 },
                end: lsp::Position { line: 0, character: 0 },
            };
        }
        let last_idx = token_spans.len().saturating_sub(1);
        let start_idx = token_span.start.min(last_idx);
        let mut end_idx = if token_span.end == 0 { 0 } else { token_span.end.saturating_sub(1) };
        end_idx = end_idx.min(last_idx);

        let start_char = token_spans.get(start_idx).map(|s| s.start).unwrap_or(0);
        let mut end_char = token_spans.get(end_idx).map(|s| s.end).unwrap_or(start_char);

        if end_char < start_char {
            end_char = start_char;
        }
        let start = Self::offset_to_position(text, start_char);
        let end = Self::offset_to_position(text, end_char);
        lsp::Range { start, end }
    }

    fn offset_to_position(text: &str, offset_chars: usize) -> lsp::Position {
        // Map a character offset into UTF-16 line/character for LSP
        // We approximate by using Unicode scalar values for both line and character.
        // VSCode uses UTF-16, but this suffices for a minimal server.
        let mut remaining = offset_chars;
        let mut line: u32 = 0;
        for l in text.split_inclusive('\n') {
            let len = l.chars().count();
            if remaining <= len {
                let col = l.chars().take(remaining).map(|c| c.len_utf16() as u32).sum();
                return lsp::Position { line, character: col };
            }
            remaining -= len;
            line += 1;
        }
        lsp::Position { line, character: 0 }
    }

    fn position_to_offset(text: &str, pos: lsp::Position) -> usize {
        let mut line_idx = 0u32;
        let mut offset_chars = 0usize;
        for l in text.split_inclusive('\n') {
            if line_idx == pos.line {
                // Convert UTF-16 character to Rust char count approximately
                let mut acc16 = 0u32;
                for ch in l.chars() {
                    if acc16 >= pos.character {
                        break;
                    }
                    acc16 += ch.len_utf16() as u32;
                    offset_chars += 1;
                }
                return offset_chars;
            }
            offset_chars += l.chars().count();
            line_idx += 1;
        }
        offset_chars
    }

    fn ident_at(text: &str, pos: lsp::Position) -> Option<(String, SimpleSpan)> {
        let offset = Self::position_to_offset(text, pos);
        let chars: Vec<char> = text.chars().collect();
        if offset >= chars.len() {
            return None;
        }
        let is_ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_';
        // Expand left and right to capture the word
        let mut start = offset;
        while start > 0 && is_ident_char(chars[start - 1]) {
            start -= 1;
        }
        let mut end = offset;
        while end < chars.len() && is_ident_char(chars[end]) {
            end += 1;
        }
        if start < end {
            let name: String = chars[start..end].iter().collect();
            Some((name, SimpleSpan::new((), start..end)))
        } else {
            None
        }
    }

    fn build_signature_string(sig: &HirFunctionSignature) -> String {
        let params = sig
            .params
            .iter()
            .map(|(n, t)| format!("{}: {:?}", n, t))
            .collect::<Vec<_>>()
            .join(", ");
        format!("fn {}({}) -> {:?}", sig.name, params, sig.ret_type)
    }

    fn resolve_module_dir_path(path: &[String]) -> Option<PathBuf> {
        if path.is_empty() {
            return None;
        }
        let (root_dir, path_segments) = if path[0].eq_ignore_ascii_case("self") {
            ("./src", &path[1..])
        } else {
            ("./modules", &path[..])
        };
        let mut final_path = PathBuf::from(root_dir);
        for segment in path_segments {
            final_path.push(segment.to_lowercase());
        }
        Some(final_path)
    }

    fn diagnostics_push(map: &mut HashMap<PathBuf, Vec<lsp::Diagnostic>>, path: &Path, diag: lsp::Diagnostic) {
        map.entry(path.to_path_buf()).or_default().push(diag);
    }

    fn analyze(&self, entry_path: &Path, entry_text: &str) -> AnalysisResult {
        let mut sources: HashMap<PathBuf, String> = HashMap::new();
        let mut ast: HashMap<PathBuf, Vec<OwnedSpanned<OwnedItem>>> = HashMap::new();
        let mut diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>> = HashMap::new();
        let mut token_cache: HashMap<PathBuf, Vec<(Token<'static>, SimpleSpan)>> = HashMap::new();
        let mut token_spans_map: HashMap<PathBuf, Vec<SimpleSpan>> = HashMap::new();

        fn parse_one_file(
            path: &Path,
            text: &str,
            sources: &mut HashMap<PathBuf, String>,
            ast: &mut HashMap<PathBuf, Vec<OwnedSpanned<OwnedItem>>>,
            diagnostics: &mut HashMap<PathBuf, Vec<lsp::Diagnostic>>,
            token_cache: &mut HashMap<PathBuf, Vec<(Token<'static>, SimpleSpan)>>,
            token_spans_out: &mut HashMap<PathBuf, Vec<SimpleSpan>>,
        ) {
            sources.insert(path.to_path_buf(), text.to_string());
            let (tokens, lex_errs) = lexer().parse(text).into_output_errors();
            for e in lex_errs {
                let range = lsp::Range {
                    start: Backend::offset_to_position(text, e.span().start),
                    end: Backend::offset_to_position(text, e.span().end),
                };
                Backend::diagnostics_push(
                    diagnostics,
                    path,
                    lsp::Diagnostic {
                        range,
                        severity: Some(lsp::DiagnosticSeverity::ERROR),
                        source: Some("basalt-lexer".to_string()),
                        message: format!("Lexing error: {}", e.reason()),
                        ..Default::default()
                    },
                );
            }
            let tokens_with_spans: Vec<(Token, SimpleSpan)> = match tokens {
                Some(t) => t,
                None => Vec::new(),
            };
            // Extend lifetime for parser; safe for LSP process lifetime
            let owned_tokens: Vec<(Token<'static>, SimpleSpan)> = tokens_with_spans
                .into_iter()
                .map(|(tok, sp)| (unsafe { std::mem::transmute::<Token<'_>, Token<'static>>(tok) }, sp))
                .collect();
            let tokens_for_parser: Vec<Token<'static>> = owned_tokens.iter().map(|(t, _)| t.clone()).collect();
            let key = path.to_path_buf();
            let spans_only: Vec<SimpleSpan> = owned_tokens.iter().map(|(_, s)| *s).collect();
            token_spans_out.insert(key.clone(), spans_only);
            token_cache.insert(key, owned_tokens.clone());

            let (items, parse_errs) = file_parser().parse(&tokens_for_parser).into_output_errors();
            for e in parse_errs {
                // Use the token-range that chumsky provides for the error
                let tok_span = SimpleSpan::new((), e.span().start..e.span().end);
                let token_spans = token_cache
                    .get(path)
                    .map(|v| v.iter().map(|(_, s)| *s).collect::<Vec<_>>())
                    .unwrap_or_default();
                let range = Backend::token_index_span_to_range(text, &token_spans, tok_span);
                Backend::diagnostics_push(
                    diagnostics,
                    path,
                    lsp::Diagnostic {
                        range,
                        severity: Some(lsp::DiagnosticSeverity::ERROR),
                        source: Some("basalt-parser".to_string()),
                        message: format!("Parse error: {}", e.to_string()),
                        ..Default::default()
                    },
                );
            }
            if let Some(items) = items {
                let mut owned_items: Vec<OwnedItemWithSpan> = Vec::new();
                for item in items {
                    let owned: OwnedItem = (&item).into();
                    owned_items.push(OwnedSpanned { item: owned, span: item.span });
                }
                ast.insert(path.to_path_buf(), owned_items);
            }
        }

        // Parse entry
        parse_one_file(entry_path, entry_text, &mut sources, &mut ast, &mut diagnostics, &mut token_cache, &mut token_spans_map);

        // Collect imports in a worklist and parse those files from disk
        let mut worklist: Vec<Vec<String>> = Vec::new();
        if let Some(items) = ast.get(entry_path) {
            for it in items {
                if let OwnedItem::ImportBlock { imports } = &it.item {
                    for imp in imports {
                        worklist.push(imp.path.clone());
                    }
                }
            }
        }

        let mut visited_files: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
        while let Some(import_path) = worklist.pop() {
            if let Some(dir) = Self::resolve_module_dir_path(&import_path) {
                if let Ok(read_dir) = fs::read_dir(&dir) {
                    for entry in read_dir.flatten() {
                        let path = entry.path();
                        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("bst") {
                            let canonical = match fs::canonicalize(&path) { Ok(p) => p, Err(_) => continue };
                            if !visited_files.insert(canonical.clone()) {
                                continue;
                            }
                            if let Ok(text) = fs::read_to_string(&canonical) {
                                parse_one_file(&canonical, &text, &mut sources, &mut ast, &mut diagnostics, &mut token_cache, &mut token_spans_map);
                                // Pull nested imports too
                                if let Some(items) = ast.get(&canonical) {
                                    for it in items {
                                        if let OwnedItem::ImportBlock { imports } = &it.item {
                                            for imp in imports {
                                                worklist.push(imp.path.clone());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Resolve diagnostics: report unknown imports and name conflicts with union constructors
        for (file, items) in &ast {
            // Build a set of union constructor names defined in this file
            let mut union_constructors: std::collections::HashSet<String> = std::collections::HashSet::new();
            for it in items {
                if let OwnedItem::TypeAlias(ta) = &it.item {
                    if let OwnedTypeAliasBody::Union(vars) = &ta.aliased {
                        for (vname, _payload) in vars {
                            union_constructors.insert(vname.clone());
                        }
                    }
                }
            }

            // Now scan import blocks for this file
            for it in items {
                if let OwnedItem::ImportBlock { imports } = &it.item {
                    for imp in imports {
                        let import_name = imp
                            .alias
                            .clone()
                            .unwrap_or_else(|| imp.path.last().cloned().unwrap_or_default());

                        // Conflict with union constructor name
                        if union_constructors.contains(&import_name) {
                            if let Some(text) = sources.get(file) {
                                if let Some(tok_spans) = token_spans_map.get(file) {
                                    let range = Self::token_index_span_to_range(text, tok_spans, it.span);
                                    Self::diagnostics_push(
                                        &mut diagnostics,
                                        file,
                                        lsp::Diagnostic {
                                            range,
                                            severity: Some(lsp::DiagnosticSeverity::ERROR),
                                            source: Some("basalt-resolve".to_string()),
                                            message: format!(
                                                "Import name '{}' conflicts with union constructor '{}' in this file",
                                                import_name, import_name
                                            ),
                                            ..Default::default()
                                        },
                                    );
                                }
                            }
                        }

                        // Unknown import path (module directory missing)
                        if let Some(dir) = Self::resolve_module_dir_path(&imp.path) {
                            if !dir.exists() {
                                if let Some(text) = sources.get(file) {
                                    if let Some(tok_spans) = token_spans_map.get(file) {
                                        let range = Self::token_index_span_to_range(text, tok_spans, it.span);
                                        let display_path = imp.path.join("/");
                                        Self::diagnostics_push(
                                            &mut diagnostics,
                                            file,
                                            lsp::Diagnostic {
                                                range,
                                                severity: Some(lsp::DiagnosticSeverity::ERROR),
                                                source: Some("basalt-resolve".to_string()),
                                                message: format!("Unknown import: {}", display_path),
                                                ..Default::default()
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Typecheck
        let mut typechecker = Typechecker::default();
        let hir = match typechecker.check_program(ast.clone()) {
            Ok(hir_items) => {
                // no type errors
                hir_items
            }
            Err(errors) => {
                // Publish diagnostics for each file
                for TypeError { message, context } in errors {
                    if let Some(text) = sources.get(&context.path) {
                        let range = if let Some(tok_spans) = token_spans_map.get(&context.path) {
                            Self::token_index_span_to_range(text, tok_spans, context.span)
                        } else {
                            // Fallback
                            Self::simple_span_to_range(text, context.span)
                        };
                        Self::diagnostics_push(
                            &mut diagnostics,
                            &context.path,
                            lsp::Diagnostic {
                                range,
                                severity: Some(lsp::DiagnosticSeverity::ERROR),
                                source: Some("basalt-typechecker".to_string()),
                                message,
                                ..Default::default()
                            },
                        );
                    }
                }
                Vec::new()
            }
        };

        // Build top-level index: functions, type aliases, union variants
        let mut index = TopLevelIndex::default();
        // Map function name -> signature from HIR
        let mut fn_sigs: HashMap<String, HirFunctionSignature> = HashMap::new();
        for item in &hir {
            if let HirItem::Fn(f) = item {
                fn_sigs.insert(f.signature.name.clone(), f.signature.clone());
            }
        }
        // Helper to find a specific identifier token span in the token list within a given token range
        let mut find_ident_span = |
            tokens: &[(Token<'static>, SimpleSpan)],
            item_span: SimpleSpan,
            ident: &str,
        | -> Option<SimpleSpan> {
            let start = item_span.start;
            let end = item_span.end.min(tokens.len());
            for i in start..end {
                if let Token::Ident(s) = tokens[i].0 {
                    if s == ident {
                        return Some(tokens[i].1);
                    }
                }
            }
            None
        };

        for (file, items) in &ast {
            for it in items {
                match &it.item {
                    OwnedItem::Fn(f) => {
                        let sig = fn_sigs.get(&f.name).cloned();
                        // Prefer the span of the function name token
                        let name_span = token_cache
                            .get(file)
                            .and_then(|toks| find_ident_span(toks, it.span, &f.name))
                            .unwrap_or(it.span);
                        index.functions.insert(
                            f.name.clone(),
                            DefInfo { path: file.clone(), span: name_span, signature: sig },
                        );
                    }
                    OwnedItem::TypeAlias(ta) => {
                        // Prefer the span of the alias name token
                        let name_span = token_cache
                            .get(file)
                            .and_then(|toks| find_ident_span(toks, it.span, &ta.name))
                            .unwrap_or(it.span);
                        index.type_aliases.insert(ta.name.clone(), (file.clone(), name_span));
                        if let OwnedTypeAliasBody::Union(vars) = &ta.aliased {
                            for (vname, _payload) in vars {
                                // Try to find each variant's ident span within the item span
                                let v_span = token_cache
                                    .get(file)
                                    .and_then(|toks| find_ident_span(toks, it.span, vname))
                                    .unwrap_or(it.span);
                                index.union_variants.insert(vname.clone(), (file.clone(), v_span));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        AnalysisResult { sources, ast, hir, diagnostics, index, token_spans: token_spans_map }
    }

    async fn reanalyze_and_publish(&self, root_path: PathBuf, text: String) {
        let _guard = self.analyze_lock.lock().await;
        // Capture previously published files for this root (so we can clear them if needed)
        let previously_published: Vec<PathBuf> = {
            let map = self.analysis.read();
            if let Some(prev) = map.get(&root_path) {
                prev.sources.keys().cloned().collect()
            } else {
                Vec::new()
            }
        };

        let result = self.analyze(&root_path, &text);

        // Cache new analysis
        self.analysis.write().insert(root_path.clone(), result.clone());

        // Union set of paths (current + previous)
        let mut all_paths: std::collections::HashSet<PathBuf> = result.sources.keys().cloned().collect();
        for p in previously_published { all_paths.insert(p); }

        // Publish diagnostics for all, clearing old ones by sending empty arrays
        for path in all_paths {
            let uri = lsp::Url::from_file_path(&path).unwrap_or_else(|_| lsp::Url::parse("file:///").unwrap());
            let diags = result.diagnostics.get(&path).cloned().unwrap_or_default();
            self.client.publish_diagnostics(uri, diags, None).await;
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: lsp::InitializeParams) -> LspResult<lsp::InitializeResult> {
        let server_caps = lsp::ServerCapabilities {
            text_document_sync: Some(lsp::TextDocumentSyncCapability::Kind(lsp::TextDocumentSyncKind::FULL)),
            hover_provider: Some(lsp::HoverProviderCapability::Simple(true)),
            definition_provider: Some(lsp::OneOf::Left(true)),
            ..Default::default()
        };

        Ok(lsp::InitializeResult { capabilities: server_caps, server_info: None })
    }

    async fn initialized(&self, _: lsp::InitializedParams) {
        let _ = self.client.log_message(lsp::MessageType::INFO, "basalt LSP initialized").await;
    }

    async fn shutdown(&self) -> LspResult<()> { Ok(()) }

    async fn did_open(&self, params: lsp::DidOpenTextDocumentParams) {
        if let Some(path) = Self::url_to_path(&params.text_document.uri) {
            self.open_files.write().insert(path.clone(), params.text_document.text.clone());
            self.reanalyze_and_publish(path, params.text_document.text).await;
        }
    }

    async fn did_change(&self, params: lsp::DidChangeTextDocumentParams) {
        if let Some(path) = Self::url_to_path(&params.text_document.uri) {
            // Full sync: take the first content
            if let Some(change) = params.content_changes.into_iter().last() {
                self.open_files.write().insert(path.clone(), change.text.clone());
                self.reanalyze_and_publish(path, change.text).await;
            }
        }
    }

    async fn hover(&self, params: lsp::HoverParams) -> LspResult<Option<lsp::Hover>> {
        let pos_params = match params.text_document_position_params {
            lsp::TextDocumentPositionParams { text_document, position } => (text_document, position),
        };
        let (doc, position) = pos_params;
        let path = match Self::url_to_path(&doc.uri) { Some(p) => p, None => return Ok(None) };
        let text = match self.open_files.read().get(&path) { Some(t) => t.clone(), None => return Ok(None) };

        let analysis_map = self.analysis.read();
        let analysis = match analysis_map.get(&path) { Some(a) => a.clone(), None => return Ok(None) };
        drop(analysis_map);

        if let Some((name, _span)) = Self::ident_at(&text, position) {
            if let Some(def) = analysis.index.functions.get(&name) {
                if let Some(sig) = &def.signature {
                    let contents = lsp::HoverContents::Markup(lsp::MarkupContent {
                        kind: lsp::MarkupKind::PlainText,
                        value: Self::build_signature_string(sig),
                    });
                    return Ok(Some(lsp::Hover { contents, range: None }));
                } else {
                    let contents = lsp::HoverContents::Markup(lsp::MarkupContent {
                        kind: lsp::MarkupKind::PlainText,
                        value: format!("fn {}(..)", name),
                    });
                    return Ok(Some(lsp::Hover { contents, range: None }));
                }
            }
            if let Some((_p, _s)) = analysis.index.type_aliases.get(&name) {
                let contents = lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::PlainText,
                    value: format!("type {}", name),
                });
                return Ok(Some(lsp::Hover { contents, range: None }));
            }
            if let Some((_p, _s)) = analysis.index.union_variants.get(&name) {
                let contents = lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::PlainText,
                    value: format!("variant {}", name),
                });
                return Ok(Some(lsp::Hover { contents, range: None }));
            }
        }

        Ok(None)
    }

    async fn goto_definition(&self, params: lsp::GotoDefinitionParams) -> LspResult<Option<lsp::GotoDefinitionResponse>> {
        let pos_params = params.text_document_position_params;
        let path = match Self::url_to_path(&pos_params.text_document.uri) { Some(p) => p, None => return Ok(None) };
        let text = match self.open_files.read().get(&path) { Some(t) => t.clone(), None => return Ok(None) };

        let analysis_map = self.analysis.read();
        let analysis = match analysis_map.get(&path) { Some(a) => a.clone(), None => return Ok(None) };
        drop(analysis_map);

        if let Some((name, _span)) = Self::ident_at(&text, pos_params.position) {
            if let Some(def) = analysis.index.functions.get(&name) {
                if let Some(src_text) = analysis.sources.get(&def.path) {
                    let range = Self::simple_span_to_range(src_text, def.span);
                    let loc = lsp::Location { uri: lsp::Url::from_file_path(&def.path).unwrap(), range };
                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                }
            }
            if let Some((p, s)) = analysis.index.type_aliases.get(&name) {
                if let Some(src_text) = analysis.sources.get(p) {
                    let range = Self::simple_span_to_range(src_text, *s);
                    let loc = lsp::Location { uri: lsp::Url::from_file_path(p).unwrap(), range };
                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                }
            }
            if let Some((p, s)) = analysis.index.union_variants.get(&name) {
                if let Some(src_text) = analysis.sources.get(p) {
                    let range = Self::simple_span_to_range(src_text, *s);
                    let loc = lsp::Location { uri: lsp::Url::from_file_path(p).unwrap(), range };
                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                }
            }
        }

        Ok(None)
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(|client| Backend::new(client));
    Server::new(stdin, stdout, socket).serve(service).await;
}


