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

use ast_owned::{OwnedItem, OwnedItemWithSpan, Spanned as OwnedSpanned};
use hir::{Item as HirItem};
use lexer::lexer;
use parser::file_parser;
use token::{SimpleSpan, Token};
use typechecker::{TypeError, Typechecker};

#[derive(Default, Clone)]
struct AnalysisResult {
    sources: HashMap<PathBuf, String>,
    hir: Vec<HirItem>,
    diagnostics: HashMap<PathBuf, Vec<lsp::Diagnostic>>,
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

    // removed AST-based enclosing expr search; HIR is source of truth

    // Removed name-based fallbacks: LSP uses only HIR resolution and item metadata

    fn find_hir_expr_in_block(block: &hir::HirBlock, tok_idx: usize) -> Option<hir::Expr> {
        // Search statements first, then last_expr; return the smallest containing expr
        let mut best: Option<hir::Expr> = None;
        for s in &block.stmts {
            if let Some(e) = Self::find_hir_expr_in_stmt(s, tok_idx) {
                best = Some(e);
                break;
            }
        }
        if best.is_none() {
            if let Some(le) = &block.last_expr { if let Some(e) = Self::find_hir_expr(le, tok_idx) { best = Some(e); } }
        }
        best
    }

    fn find_hir_expr_in_stmt(stmt: &hir::Stmt, tok_idx: usize) -> Option<hir::Expr> {
        use hir::Stmt as HS;
        match stmt {
            HS::Let { value, .. } => {
                if let Some(v) = value { return Self::find_hir_expr(v, tok_idx); }
                None
            }
            HS::Assign { lhs, rhs, .. } => {
                if let Some(e) = Self::find_hir_expr(lhs, tok_idx) { return Some(e); }
                if let Some(e) = Self::find_hir_expr(rhs, tok_idx) { return Some(e); }
                None
            }
            HS::Return { value, .. } => {
                if let Some(v) = value { return Self::find_hir_expr(v, tok_idx); }
                None
            }
            HS::Expr { expr, .. } => Self::find_hir_expr(expr, tok_idx),
            HS::Error { .. } => None,
        }
    }

    fn find_hir_expr(expr: &hir::Expr, tok_idx: usize) -> Option<hir::Expr> {
        if !(expr.span.start <= tok_idx && tok_idx < expr.span.end) { return None; }
        use hir::ExprKind as EK;
        match &expr.kind {
            EK::Unary { rhs, .. } => Self::find_hir_expr(rhs, tok_idx).or_else(|| Some(expr.clone())),
            EK::Binary { lhs, rhs, .. } => Self::find_hir_expr(lhs, tok_idx).or_else(|| Self::find_hir_expr(rhs, tok_idx)).or_else(|| Some(expr.clone())),
            EK::FieldAccess { receiver, .. } => Self::find_hir_expr(receiver, tok_idx).or_else(|| Some(expr.clone())),
            EK::Call { fun, args } => {
                if let Some(e) = Self::find_hir_expr(fun, tok_idx) { return Some(e); }
                for a in args { if let Some(e) = Self::find_hir_expr(a, tok_idx) { return Some(e); } }
                Some(expr.clone())
            }
            EK::StructInit { fields, .. } => {
                for (_n, e) in fields { if let Some(found) = Self::find_hir_expr(e, tok_idx) { return Some(found); } }
                Some(expr.clone())
            }
            EK::Array(es) => { for e in es { if let Some(f) = Self::find_hir_expr(e, tok_idx) { return Some(f); } } Some(expr.clone()) }
            EK::Map(kvs) => { for (k, v) in kvs { if let Some(f) = Self::find_hir_expr(k, tok_idx) { return Some(f); } if let Some(f) = Self::find_hir_expr(v, tok_idx) { return Some(f); } } Some(expr.clone()) }
            EK::If { cond, then_block, else_block } => {
                if let Some(e) = Self::find_hir_expr(cond, tok_idx) { return Some(e); }
                if let Some(e) = Self::find_hir_expr_in_block(then_block, tok_idx) { return Some(e); }
                if let Some(eb) = else_block { if let Some(e) = Self::find_hir_expr(eb, tok_idx) { return Some(e); } }
                Some(expr.clone())
            }
            EK::While { cond, body } => {
                if let Some(e) = Self::find_hir_expr(cond, tok_idx) { return Some(e); }
                if let Some(e) = Self::find_hir_expr_in_block(body, tok_idx) { return Some(e); }
                Some(expr.clone())
            }
            EK::Match { scrutinee, arms } => {
                if let Some(e) = Self::find_hir_expr(scrutinee, tok_idx) { return Some(e); }
                for (_p, arm) in arms { if let Some(e) = Self::find_hir_expr(arm, tok_idx) { return Some(e); } }
                Some(expr.clone())
            }
            EK::Handle { body, .. } => Self::find_hir_expr_in_block(body, tok_idx).or_else(|| Some(expr.clone())),
            EK::Cast { expr: inner } => Self::find_hir_expr(inner, tok_idx).or_else(|| Some(expr.clone())),
            _ => Some(expr.clone()),
        }
    }

    fn find_let_decl_spans_in_block(block: &hir::HirBlock, name: &str, out: &mut Vec<token::SimpleSpan>) {
        use hir::Stmt as HS;
        for s in &block.stmts {
            match s {
                HS::Let { name: n, span, name_span, .. } if n == name => {
                    out.push(name_span.unwrap_or(*span));
                }
                HS::Let { .. } => {}
                HS::Assign { lhs, rhs, .. } => {
                    if let hir::ExprKind::Block(b) = &lhs.kind { Self::find_let_decl_spans_in_block(b, name, out); }
                    if let hir::ExprKind::Block(b) = &rhs.kind { Self::find_let_decl_spans_in_block(b, name, out); }
                }
                HS::Return { value, .. } => {
                    if let Some(v) = value {
                        if let hir::ExprKind::Block(b) = &v.kind { Self::find_let_decl_spans_in_block(b, name, out); }
                    }
                }
                HS::Expr { expr, .. } => {
                    Self::recurse_expr_for_lets(expr, name, out);
                }
                HS::Error { .. } => {}
            }
        }
        if let Some(e) = &block.last_expr { Self::recurse_expr_for_lets(e, name, out); }
    }

    fn recurse_expr_for_lets(expr: &hir::Expr, name: &str, out: &mut Vec<token::SimpleSpan>) {
        use hir::ExprKind as EK;
        match &expr.kind {
            EK::Block(b) => Self::find_let_decl_spans_in_block(b, name, out),
            EK::If { cond, then_block, else_block } => {
                Self::recurse_expr_for_lets(cond, name, out);
                Self::find_let_decl_spans_in_block(then_block, name, out);
                if let Some(eb) = else_block { Self::recurse_expr_for_lets(eb, name, out); }
            }
            EK::While { cond, body } => {
                Self::recurse_expr_for_lets(cond, name, out);
                Self::find_let_decl_spans_in_block(body, name, out);
            }
            EK::Match { scrutinee, arms } => {
                Self::recurse_expr_for_lets(scrutinee, name, out);
                for (_p, arm) in arms { Self::recurse_expr_for_lets(arm, name, out); }
            }
            EK::Unary { rhs, .. } => Self::recurse_expr_for_lets(rhs, name, out),
            EK::Binary { lhs, rhs, .. } => { Self::recurse_expr_for_lets(lhs, name, out); Self::recurse_expr_for_lets(rhs, name, out); }
            EK::FieldAccess { receiver, .. } => Self::recurse_expr_for_lets(receiver, name, out),
            EK::Call { fun, args } => { Self::recurse_expr_for_lets(fun, name, out); for a in args { Self::recurse_expr_for_lets(a, name, out); } }
            EK::StructInit { fields, .. } => { for (_n, e) in fields { Self::recurse_expr_for_lets(e, name, out); } }
            EK::Array(es) => { for e in es { Self::recurse_expr_for_lets(e, name, out); } }
            EK::Map(kvs) => { for (k, v) in kvs { Self::recurse_expr_for_lets(k, name, out); Self::recurse_expr_for_lets(v, name, out); } }
            EK::Handle { body, .. } => Self::find_let_decl_spans_in_block(body, name, out),
            EK::Cast { expr: inner } => Self::recurse_expr_for_lets(inner, name, out),
            _ => {}
        }
    }

    fn find_struct_field_location(
        analysis: &AnalysisResult,
        owner: &hir::OwnedPath,
        field: &str,
    ) -> Option<lsp::Location> {
        let owner_name = owner.last().cloned().unwrap_or_default();
        for item in &analysis.hir {
            if let HirItem::Struct(s) = item {
                if s.name == owner_name {
                    if let Some(f) = s.fields.iter().find(|f| f.name == field) {
                        if let Some(src_text) = analysis.sources.get(&s.defined_in) {
                            let range = Backend::simple_span_to_range(src_text, f.name_span.unwrap_or(s.span));
                            let uri = lsp::Url::from_file_path(&s.defined_in).ok()?;
                            return Some(lsp::Location { uri, range });
                        }
                    }
                }
            }
        }
        None
    }

    // removed locate_method_impl

    // Extract a best-effort owner type path for a receiver expression, using inferred or annotated local types
    // removed infer_receiver_type_path

    // removed hir_type_to_path helper

    // Try to infer an owner type name from an enclosing let statement's annotated type
    // removed infer_owner_from_enclosing_let

    // removed build_signature_string

    // removed resolve_module_dir_path

    fn diagnostics_push(map: &mut HashMap<PathBuf, Vec<lsp::Diagnostic>>, path: &Path, diag: lsp::Diagnostic) {
        map.entry(path.to_path_buf()).or_default().push(diag);
    }

    // removed list_module_targets

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

        // Parse just the entry file and do not resolve imports; rely on typechecker
        parse_one_file(entry_path, entry_text, &mut sources, &mut ast, &mut diagnostics, &mut token_cache, &mut token_spans_map);

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

        AnalysisResult { sources, hir, diagnostics, token_spans: token_spans_map }
    }

    fn collect_locals_in_expr(
        expr: &ast_owned::Spanned<ast_owned::OwnedExpr>,
        tokens: &[(Token<'static>, SimpleSpan)],
        locals_out: &mut HashMap<String, SimpleSpan>,
    ) {
        use ast_owned::OwnedExpr as OE;
        use ast_owned::OwnedStmt as OS;
        match &expr.item {
            OE::Block { stmts, last_expr } => {
                for s in stmts {
                    match &s.item {
                        OS::Let { name, .. } => {
                            if let Some(sp) = Self::find_ident_in_span(tokens, s.span, name) {
                                locals_out.insert(name.clone(), sp);
                            }
                        }
                        OS::Assign(l, r) => {
                            Self::collect_locals_in_expr(l, tokens, locals_out);
                            Self::collect_locals_in_expr(r, tokens, locals_out);
                        }
                        OS::Return(eopt) => {
                            if let Some(e) = eopt { Self::collect_locals_in_expr(e, tokens, locals_out); }
                        }
                        OS::Expr(e) => Self::collect_locals_in_expr(e, tokens, locals_out),
                        OS::Error => {}
                    }
                }
                if let Some(le) = last_expr { Self::collect_locals_in_expr(le, tokens, locals_out); }
            }
            OE::Unary { rhs, .. } => Self::collect_locals_in_expr(rhs, tokens, locals_out),
            OE::Binary { lhs, rhs, .. } => {
                Self::collect_locals_in_expr(lhs, tokens, locals_out);
                Self::collect_locals_in_expr(rhs, tokens, locals_out);
            }
            OE::FieldAccess { receiver, .. } => Self::collect_locals_in_expr(receiver, tokens, locals_out),
            OE::Call { fun, args } => {
                Self::collect_locals_in_expr(fun, tokens, locals_out);
                for a in args { Self::collect_locals_in_expr(a, tokens, locals_out); }
            }
            OE::If { cond, then_block, else_block } => {
                Self::collect_locals_in_expr(cond, tokens, locals_out);
                Self::collect_locals_in_expr(then_block, tokens, locals_out);
                if let Some(e) = else_block { Self::collect_locals_in_expr(e, tokens, locals_out); }
            }
            OE::While { cond, body } => {
                Self::collect_locals_in_expr(cond, tokens, locals_out);
                Self::collect_locals_in_expr(body, tokens, locals_out);
            }
            OE::Match { scrutinee, arms } => {
                Self::collect_locals_in_expr(scrutinee, tokens, locals_out);
                for (_pat, arm) in arms { Self::collect_locals_in_expr(arm, tokens, locals_out); }
            }
            OE::Handle { body, .. } => Self::collect_locals_in_expr(body, tokens, locals_out),
            OE::Cast { expr, .. } => Self::collect_locals_in_expr(expr, tokens, locals_out),
            OE::StructInit { fields, .. } => {
                for (_n, e) in fields { Self::collect_locals_in_expr(e, tokens, locals_out); }
            }
            OE::Array(elems) => { for e in elems { Self::collect_locals_in_expr(e, tokens, locals_out); } }
            OE::Map(entries) => { for (_k, v) in entries { Self::collect_locals_in_expr(v, tokens, locals_out); } }
            OE::Path(_) | OE::Literal(_) | OE::Perform { .. } | OE::Error => {}
        }
    }

    fn collect_local_annotated_types_in_fn(
        expr: &ast_owned::Spanned<ast_owned::OwnedExpr>,
        out: &mut HashMap<String, String>,
    ) {
        use ast_owned::OwnedExpr as OE;
        use ast_owned::OwnedStmt as OS;
        match &expr.item {
            OE::Block { stmts, last_expr } => {
                for s in stmts {
                    if let OS::Let { is_mut: _, name, ty, value: _ } = &s.item {
                        if let Some(t) = ty {
                            out.insert(name.clone(), Self::format_type_node(t));
                        }
                    }
                }
                if let Some(le) = last_expr {
                    Self::collect_local_annotated_types_in_fn(le, out);
                }
            }
            OE::Unary { rhs, .. } => Self::collect_local_annotated_types_in_fn(rhs, out),
            OE::Binary { lhs, rhs, .. } => {
                Self::collect_local_annotated_types_in_fn(lhs, out);
                Self::collect_local_annotated_types_in_fn(rhs, out);
            }
            OE::FieldAccess { receiver, .. } => Self::collect_local_annotated_types_in_fn(receiver, out),
            OE::Call { fun, args } => {
                Self::collect_local_annotated_types_in_fn(fun, out);
                for a in args { Self::collect_local_annotated_types_in_fn(a, out); }
            }
            OE::If { cond, then_block, else_block } => {
                Self::collect_local_annotated_types_in_fn(cond, out);
                Self::collect_local_annotated_types_in_fn(then_block, out);
                if let Some(e) = else_block { Self::collect_local_annotated_types_in_fn(e, out); }
            }
            OE::While { cond, body } => {
                Self::collect_local_annotated_types_in_fn(cond, out);
                Self::collect_local_annotated_types_in_fn(body, out);
            }
            OE::Match { scrutinee, arms } => {
                Self::collect_local_annotated_types_in_fn(scrutinee, out);
                for (_pat, arm) in arms { Self::collect_local_annotated_types_in_fn(arm, out); }
            }
            OE::Handle { body, .. } => Self::collect_local_annotated_types_in_fn(body, out),
            OE::Cast { expr, .. } => Self::collect_local_annotated_types_in_fn(expr, out),
            OE::StructInit { fields, .. } => {
                for (_n, e) in fields { Self::collect_local_annotated_types_in_fn(e, out); }
            }
            OE::Array(elems) => { for e in elems { Self::collect_local_annotated_types_in_fn(e, out); } }
            OE::Map(entries) => { for (_k, v) in entries { Self::collect_local_annotated_types_in_fn(v, out); } }
            OE::Path(_) | OE::Literal(_) | OE::Perform { .. } | OE::Error => {}
        }
    }

    fn collect_local_inferred_types_in_hir_block(
        block: &hir::HirBlock,
        out: &mut HashMap<String, hir::Ty>,
    ) {
        use hir::Stmt as HS;
        for s in &block.stmts {
            if let HS::Let { name, ty, .. } = s {
                out.insert(name.clone(), ty.clone());
            }
        }
        if let Some(last) = &block.last_expr {
            let _ = &last.ty; // currently unused; could be wired to special identifier `_` etc.
        }
    }

    fn format_type_node(ty: &ast_owned::OwnedType) -> String {
        let path = ty.path.join("::");
        if ty.generics.is_empty() {
            path
        } else {
            let gens: Vec<String> = ty.generics.iter().map(Self::format_type_node).collect();
            format!("{}<{}>", path, gens.join(", "))
        }
    }

    fn owned_type_to_hir_ty(ty: &ast_owned::OwnedType) -> hir::Ty {
        if ty.path.len() == 1 {
            match ty.path[0].as_str() {
                "bool" => return hir::Ty::Primitive(hir::PrimitiveTy::Bool),
                "byte" => return hir::Ty::Primitive(hir::PrimitiveTy::Byte),
                "i32" => return hir::Ty::Primitive(hir::PrimitiveTy::I32),
                "i64" => return hir::Ty::Primitive(hir::PrimitiveTy::I64),
                "f64" => return hir::Ty::Primitive(hir::PrimitiveTy::F64),
                "str" => return hir::Ty::Primitive(hir::PrimitiveTy::Str),
                "()" => return hir::Ty::Special(hir::SpecialTy::Unit),
                _ => {}
            }
        }
        hir::Ty::Adt(hir::AdtTy::Struct { name: ty.path.clone(), generics: ty.generics.iter().map(Self::owned_type_to_hir_ty).collect() })
    }

    fn find_ident_in_span(
        tokens: &[(Token<'static>, SimpleSpan)],
        item_span: SimpleSpan,
        ident: &str,
    ) -> Option<SimpleSpan> {
        let start = item_span.start;
        let end = item_span.end.min(tokens.len());
        for i in start..end {
            if let Token::Ident(s) = tokens[i].0 {
                if s == ident { return Some(tokens[i].1); }
            }
        }
        None
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
        // HIR-only hover: show the type of the expression under cursor
        let char_offset = Self::position_to_offset(&text, position);
        let token_index_opt = analysis
            .token_spans
            .get(&path)
            .and_then(|spans| spans.iter().position(|s| s.start <= char_offset && char_offset < s.end));
        if let Some(tok_idx) = token_index_opt {
            // Collect candidate HIR blocks from the HIR (no extra indexing)
            let mut candidate_blocks: Vec<&hir::HirBlock> = Vec::new();
            for item in &analysis.hir {
                match item {
                    HirItem::Fn(f) if f.defined_in == path => candidate_blocks.push(&f.body),
                    HirItem::Impl(imp) if imp.defined_in == path => {
                        for m in &imp.methods { candidate_blocks.push(&m.body); }
                    }
                    _ => {}
                }
            }
            for b in candidate_blocks {
                if let Some(expr) = Self::find_hir_expr_in_block(b, tok_idx) {
                    let contents = lsp::HoverContents::Markup(lsp::MarkupContent { kind: lsp::MarkupKind::PlainText, value: format!("{:?}", expr.ty) });
                    return Ok(Some(lsp::Hover { contents, range: None }));
                }
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

        // Map cursor position to token index for containment checks
        let char_offset = Self::position_to_offset(&text, pos_params.position);
        let token_index_opt = analysis
            .token_spans
            .get(&path)
            .and_then(|spans| spans.iter().position(|s| s.start <= char_offset && char_offset < s.end));

        if let Some(tok_idx) = token_index_opt {
            // Search HIR blocks in this file only
            let mut candidate_blocks: Vec<&hir::HirBlock> = Vec::new();
            for item in &analysis.hir {
                match item {
                    HirItem::Fn(f) if f.defined_in == path => candidate_blocks.push(&f.body),
                    HirItem::Impl(imp) if imp.defined_in == path => {
                        for m in &imp.methods { candidate_blocks.push(&m.body); }
                    }
                    _ => {}
                }
            }
            for b in candidate_blocks {
                if let Some(expr) = Self::find_hir_expr_in_block(b, tok_idx) {
                    if let Some(res) = &expr.resolution {
                        match res {
                            hir::Resolution::Local { name, decl_span } => {
                                let target_span = decl_span.unwrap_or(expr.span);
                                if let Some(src_text) = analysis.sources.get(&path) {
                                    let ranges = analysis.token_spans.get(&path).cloned().unwrap_or_default();
                                    let range = Backend::token_index_span_to_range(&text, &ranges, target_span);
                                    let uri = lsp::Url::from_file_path(&path).ok();
                                    if let Some(uri) = uri { return Ok(Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location { uri, range }))); }
                                }
                            }
                            hir::Resolution::Field { owner, field } => {
                                if let Some(loc) = Self::find_struct_field_location(&analysis, owner, field) {
                                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                                }
                            }
                        }
                    }
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


