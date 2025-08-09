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
    // allow multiple functions with same simple name (different modules)
    functions: HashMap<String, Vec<DefInfo>>,
    type_aliases: HashMap<String, (PathBuf, SimpleSpan)>,
    union_variants: HashMap<String, (PathBuf, SimpleSpan)>,
    // module name (alias or last segment) -> (file path, name span in import block, full path segments)
    modules: HashMap<String, (PathBuf, SimpleSpan, Vec<String>)>,
    // trait/interface names
    traits: HashMap<String, (PathBuf, SimpleSpan)>,
    // method name -> possibly multiple definitions (impls on different types)
    methods: HashMap<String, Vec<DefInfo>>,
    // trait method name -> possibly multiple definitions (interface declarations)
    trait_methods: HashMap<String, Vec<DefInfo>>,
    // per-file local variable definitions: name -> span
    locals_by_file: HashMap<PathBuf, HashMap<String, SimpleSpan>>,
    // per-file locals with owning item span for disambiguation: name -> list of (ident span, owner item span)
    locals_detailed_by_file: HashMap<PathBuf, HashMap<String, Vec<(SimpleSpan, SimpleSpan)>>>,
    // per-file local variables with annotated types: name -> pretty type
    local_annotated_types_by_file: HashMap<PathBuf, HashMap<String, String>>,
    // per-file local variables with inferred types: name -> hir::Ty
    local_inferred_types_by_file: HashMap<PathBuf, HashMap<String, hir::Ty>>,
    // record fields index: field name -> list of (file, span, parent type name, field type)
    record_fields: HashMap<String, Vec<(PathBuf, SimpleSpan, String, ast_owned::OwnedType)>>,
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
    // HIR function and method bodies for span-based lookup
    hir_fn_bodies: HashMap<String, hir::HirBlock>,
    hir_method_bodies: HashMap<String, Vec<hir::HirBlock>>,
    hir_method_sigs: HashMap<String, Vec<HirFunctionSignature>>,
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

    // Attempt to find the smallest OwnedExpr that contains the given token index
    fn find_enclosing_expr_in_items(
        items: &[OwnedSpanned<OwnedItem>],
        tok_idx: usize,
    ) -> Option<ast_owned::Spanned<ast_owned::OwnedExpr>> {
        use ast_owned::OwnedExpr as OE;

        fn descend(expr: &ast_owned::Spanned<ast_owned::OwnedExpr>, tok_idx: usize) -> Option<ast_owned::Spanned<ast_owned::OwnedExpr>> {
            if !(expr.span.start <= tok_idx && tok_idx < expr.span.end) {
                return None;
            }
            let mut best: Option<ast_owned::Spanned<ast_owned::OwnedExpr>> = None;
            match &expr.item {
                OE::Array(elems) => {
                    for e in elems { if let Some(d) = descend(e, tok_idx) { best = Some(d); } }
                }
                OE::Map(entries) => {
                    for (k, v) in entries {
                        if let Some(d) = descend(k, tok_idx) { best = Some(d); }
                        if let Some(d) = descend(v, tok_idx) { best = Some(d); }
                    }
                }
                OE::FieldAccess { receiver, .. } => {
                    if let Some(d) = descend(receiver, tok_idx) { best = Some(d); }
                }
                OE::Unary { rhs, .. } => { if let Some(d) = descend(rhs, tok_idx) { best = Some(d); } }
                OE::Binary { lhs, rhs, .. } => {
                    if let Some(d) = descend(lhs, tok_idx) { best = Some(d); }
                    if let Some(d) = descend(rhs, tok_idx) { best = Some(d); }
                }
                OE::Call { fun, args } => {
                    if let Some(d) = descend(fun, tok_idx) { best = Some(d); }
                    for a in args { if let Some(d) = descend(a, tok_idx) { best = Some(d); } }
                }
                OE::StructInit { fields, .. } => {
                    for (_n, e) in fields { if let Some(d) = descend(e, tok_idx) { best = Some(d); } }
                }
                OE::Block { stmts, last_expr } => {
                    use ast_owned::OwnedStmt as OS;
                    for s in stmts {
                        if !(s.span.start <= tok_idx && tok_idx < s.span.end) { continue; }
                        match &s.item {
                            OS::Let { value, .. } => { if let Some(val) = value { if let Some(d) = descend(val, tok_idx) { best = Some(d); } } }
                            OS::Assign(l, r) => {
                                if let Some(d) = descend(l, tok_idx) { best = Some(d); }
                                if let Some(d) = descend(r, tok_idx) { best = Some(d); }
                            }
                            OS::Return(e) => { if let Some(e) = e { if let Some(d) = descend(e, tok_idx) { best = Some(d); } } }
                            OS::Expr(e) => { if let Some(d) = descend(e, tok_idx) { best = Some(d); } }
                            OS::Error => {}
                        }
                    }
                    if let Some(le) = last_expr { if let Some(d) = descend(le, tok_idx) { best = Some(d); } }
                }
                OE::If { cond, then_block, else_block } => {
                    if let Some(d) = descend(cond, tok_idx) { best = Some(d); }
                    if let Some(d) = descend(then_block, tok_idx) { best = Some(d); }
                    if let Some(e) = else_block { if let Some(d) = descend(e, tok_idx) { best = Some(d); } }
                }
                OE::Match { scrutinee, arms } => {
                    if let Some(d) = descend(scrutinee, tok_idx) { best = Some(d); }
                    for (_p, arm) in arms { if let Some(d) = descend(arm, tok_idx) { best = Some(d); } }
                }
                OE::While { cond, body } => {
                    if let Some(d) = descend(cond, tok_idx) { best = Some(d); }
                    if let Some(d) = descend(body, tok_idx) { best = Some(d); }
                }
                OE::Handle { body, .. } => { if let Some(d) = descend(body, tok_idx) { best = Some(d); } }
                OE::Cast { expr, .. } => { if let Some(d) = descend(expr, tok_idx) { best = Some(d); } }
                OE::Path(_) | OE::Literal(_) | OE::Perform { .. } | OE::Error => {}
            }
            // If no deeper match, current expr is the smallest containing
            Some(best.unwrap_or_else(|| expr.clone()))
        }

        for it in items {
            match &it.item {
                OwnedItem::Fn(f) => {
                    if let Some(found) = descend(&f.body, tok_idx) { return Some(found); }
                }
                OwnedItem::Impl(imp) => {
                    for m in &imp.methods {
                        if let Some(found) = descend(&m.body, tok_idx) { return Some(found); }
                    }
                }
                _ => {}
            }
        }
        None
    }

    // Given a token index, try to find a precise HIR-based location for goto
    fn goto_via_hir(
        analysis: &AnalysisResult,
        file: &PathBuf,
        tok_idx: usize,
        ident: &str,
        text: &str,
    ) -> Option<lsp::GotoDefinitionResponse> {
        // Resolve locals/params provided by typechecker scope mapping under current owning span
        if let Some(det) = analysis.index.locals_detailed_by_file.get(file).and_then(|m| m.get(ident)) {
            if let Some((sp, _owner)) = det.iter().find(|(_lsp, owner_sp)| owner_sp.start <= tok_idx && tok_idx < owner_sp.end) {
                let range = Backend::simple_span_to_range(text, *sp);
                let loc = lsp::Location { uri: lsp::Url::from_file_path(file).ok()?, range };
                return Some(lsp::GotoDefinitionResponse::Scalar(loc));
            }
        }

        // Find HIR expr at position by scanning function/method bodies for this file
        let mut candidate_blocks: Vec<&hir::HirBlock> = Vec::new();
        // Top-level functions in this file
        if let Some(funcs) = analysis.index.functions.iter().filter(|(_n, defs)| defs.iter().any(|d| &d.path == file)).map(|(n, _)| n.clone()).collect::<Vec<_>>().into_iter().next() { let _ = funcs; }
        for (name, defs) in &analysis.index.functions {
            if defs.iter().any(|d| &d.path == file) {
                if let Some(b) = analysis.hir_fn_bodies.get(name) { candidate_blocks.push(b); }
            }
        }
        // Methods in this file
        for (name, defs) in &analysis.index.methods {
            if defs.iter().any(|d| &d.path == file) {
                if let Some(v) = analysis.hir_method_bodies.get(name) {
                    for b in v { candidate_blocks.push(b); }
                }
            }
        }

        // Search the smallest expr containing tok_idx
        let mut best: Option<hir::Expr> = None;
        for b in candidate_blocks {
            if let Some(e) = Self::find_hir_expr_in_block(b, tok_idx) {
                best = Some(e);
                break;
            }
        }
        if let Some(expr) = best {
            use hir::ExprKind as EK;
            match &expr.kind {
                EK::Path(p) => {
                    if p.len() == 1 {
                        // Function vs variable: decide by type
                        match &expr.ty {
                            hir::Ty::Function { param_types, .. } => {
                                // If method-like (first param Adt), prefer method definitions
                                if let Some(first) = param_types.get(0) {
                                    if let Some(owner_last) = Self::hir_type_to_path(first).and_then(|v| v.last().cloned()) {
                                        if let Some(loc) = Self::locate_method_impl(analysis, &expr, &owner_last, text) { return Some(lsp::GotoDefinitionResponse::Scalar(loc)); }
                                    }
                                }
                                // Else top-level function
                                if let Some((pfile, sp)) = analysis.index.functions.get(&ident.to_string()).and_then(|v| v.first()).map(|d| (d.path.clone(), d.span)) {
                                    if let Some(src_text) = analysis.sources.get(&pfile) {
                                        let range = Backend::simple_span_to_range(src_text, sp);
                                        let uri = lsp::Url::from_file_path(&pfile).ok()?;
                                        return Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location { uri, range }));
                                    }
                                }
                            }
                            _ => {
                                // Already tried locals; no further HIR mapping needed here
                            }
                        }
                    }
                }
                EK::FieldAccess { receiver, field } => {
                    if let Some(owner) = Self::hir_type_to_path(&receiver.ty).and_then(|v| v.last().cloned()) {
                        if let Some(defs) = analysis.index.record_fields.get(field) {
                            for (p, sp, own, _ty) in defs {
                                if own == &owner {
                                    if let Some(src_text) = analysis.sources.get(p) {
                                        let range = Backend::simple_span_to_range(src_text, *sp);
                                        let uri = lsp::Url::from_file_path(p).ok()?;
                                        return Some(lsp::GotoDefinitionResponse::Scalar(lsp::Location { uri, range }));
                                    }
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

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

    fn locate_method_impl(analysis: &AnalysisResult, expr: &hir::Expr, owner_last: &str, _text: &str) -> Option<lsp::Location> {
        let name = if let hir::ExprKind::Path(p) = &expr.kind { p.last()?.clone() } else { return None };
        if let Some(defs) = analysis.index.methods.get(&name) {
            for d in defs {
                if let Some(src_text) = analysis.sources.get(&d.path) {
                    // We do not have per-def signature binding; return first match
                    let range = Backend::simple_span_to_range(src_text, d.span);
                    let uri = lsp::Url::from_file_path(&d.path).ok()?;
                    return Some(lsp::Location { uri, range });
                }
            }
        }
        None
    }

    // Extract a best-effort owner type path for a receiver expression, using inferred or annotated local types
    fn infer_receiver_type_path(
        analysis: &AnalysisResult,
        file: &PathBuf,
        recv: &ast_owned::Spanned<ast_owned::OwnedExpr>,
    ) -> Option<Vec<String>> {
        use ast_owned::OwnedExpr as OE;
        match &recv.item {
            OE::Path(segs) => {
                if segs.len() == 1 {
                    let var = &segs[0];
                    if let Some(map) = analysis.index.local_inferred_types_by_file.get(file) {
                        if let Some(ty) = map.get(var) { return Self::hir_type_to_path(ty); }
                    }
                    if let Some(map) = analysis.index.local_annotated_types_by_file.get(file) {
                        if let Some(ty_str) = map.get(var) {
                            let parts: Vec<String> = ty_str.split("::").map(|s| s.to_string()).collect();
                            if !parts.is_empty() { return Some(parts); }
                        }
                    }
                }
                None
            }
            OE::StructInit { path, .. } => Some(path.clone()),
            _ => None,
        }
    }

    fn hir_type_to_path(ty: &hir::Ty) -> Option<Vec<String>> {
        match ty {
            hir::Ty::Adt(hir::AdtTy::Struct { name, .. }) => Some(name.clone()),
            hir::Ty::Adt(hir::AdtTy::Enum { name, .. }) => Some(name.clone()),
            hir::Ty::Adt(hir::AdtTy::Effect { name, .. }) => Some(name.clone()),
            _ => None,
        }
    }

    // Try to infer an owner type name from an enclosing let statement's annotated type
    fn infer_owner_from_enclosing_let(
        items: &[OwnedSpanned<OwnedItem>],
        tok_idx: usize,
    ) -> Option<String> {
        use ast_owned::OwnedExpr as OE;
        use ast_owned::OwnedStmt as OS;

        fn visit_expr(expr: &ast_owned::Spanned<ast_owned::OwnedExpr>, tok_idx: usize) -> Option<String> {
            if !(expr.span.start <= tok_idx && tok_idx < expr.span.end) { return None; }
            match &expr.item {
                OE::Block { stmts, last_expr } => {
                    for s in stmts {
                        if !(s.span.start <= tok_idx && tok_idx < s.span.end) { continue; }
                        match &s.item {
                            OS::Let { ty, value, .. } => {
                                if let Some(val) = value {
                                    if val.span.start <= tok_idx && tok_idx < val.span.end {
                                        if let Some(t) = ty {
                                            if let Some(last) = t.path.last() { return Some(last.clone()); }
                                        }
                                    }
                                }
                            }
                            OS::Assign(l, r) => {
                                if let Some(o) = visit_expr(l, tok_idx) { return Some(o); }
                                if let Some(o) = visit_expr(r, tok_idx) { return Some(o); }
                            }
                            OS::Return(e) => { if let Some(e) = e { if let Some(o) = visit_expr(e, tok_idx) { return Some(o); } } }
                            OS::Expr(e) => { if let Some(o) = visit_expr(e, tok_idx) { return Some(o); } }
                            OS::Error => {}
                        }
                    }
                    if let Some(le) = last_expr { return visit_expr(le, tok_idx); }
                }
                OE::Unary { rhs, .. } => return visit_expr(rhs, tok_idx),
                OE::Binary { lhs, rhs, .. } => {
                    if let Some(o) = visit_expr(lhs, tok_idx) { return Some(o); }
                    if let Some(o) = visit_expr(rhs, tok_idx) { return Some(o); }
                }
                OE::FieldAccess { receiver, .. } => return visit_expr(receiver, tok_idx),
                OE::Call { fun, args } => {
                    if let Some(o) = visit_expr(fun, tok_idx) { return Some(o); }
                    for a in args { if let Some(o) = visit_expr(a, tok_idx) { return Some(o); } }
                }
                OE::If { cond, then_block, else_block } => {
                    if let Some(o) = visit_expr(cond, tok_idx) { return Some(o); }
                    if let Some(o) = visit_expr(then_block, tok_idx) { return Some(o); }
                    if let Some(e) = else_block { if let Some(o) = visit_expr(e, tok_idx) { return Some(o); } }
                }
                OE::While { cond, body } => {
                    if let Some(o) = visit_expr(cond, tok_idx) { return Some(o); }
                    if let Some(o) = visit_expr(body, tok_idx) { return Some(o); }
                }
                OE::Match { scrutinee, arms } => {
                    if let Some(o) = visit_expr(scrutinee, tok_idx) { return Some(o); }
                    for (_p, arm) in arms { if let Some(o) = visit_expr(arm, tok_idx) { return Some(o); } }
                }
                OE::Handle { body, .. } => return visit_expr(body, tok_idx),
                OE::Cast { expr, .. } => return visit_expr(expr, tok_idx),
                OE::StructInit { .. } | OE::Array(_) | OE::Map(_) | OE::Path(_) | OE::Literal(_) | OE::Perform { .. } | OE::Error => {}
            }
            None
        }

        for it in items {
            match &it.item {
                OwnedItem::Fn(f) => {
                    if let Some(o) = visit_expr(&f.body, tok_idx) { return Some(o); }
                }
                OwnedItem::Impl(imp) => {
                    for m in &imp.methods {
                        if let Some(o) = visit_expr(&m.body, tok_idx) { return Some(o); }
                    }
                }
                _ => {}
            }
        }
        None
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

    fn list_module_targets(dir: &Path) -> Vec<PathBuf> {
        let mut targets: Vec<PathBuf> = Vec::new();
        if !dir.exists() || !dir.is_dir() {
            return targets;
        }
        let mut bst_files: Vec<PathBuf> = fs::read_dir(dir)
            .ok()
            .into_iter()
            .flat_map(|rd| rd.filter_map(|e| e.ok()))
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("bst"))
            .collect();
        bst_files.sort();
        let prefer = ["mod.bst", "index.bst", "main.bst"];
        for name in &prefer {
            let candidate = dir.join(name);
            if bst_files.iter().any(|p| p == &candidate) {
                // Return canonical path if possible
                return vec![fs::canonicalize(&candidate).unwrap_or(candidate)];
            }
        }
        if bst_files.len() == 1 {
            let only = bst_files.remove(0);
            return vec![fs::canonicalize(&only).unwrap_or(only)];
        }
        // Canonicalize all; if any fail, keep as-is
        bst_files
            .into_iter()
            .map(|p| fs::canonicalize(&p).unwrap_or(p))
            .collect()
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

        // Build top-level index: functions, type aliases, union variants, modules
        let mut index = TopLevelIndex::default();
        // Map function name -> signature from HIR
        let mut fn_sigs: HashMap<String, HirFunctionSignature> = HashMap::new();
        // Map method name -> signatures (there can be multiple)
        let mut method_sigs: HashMap<String, Vec<HirFunctionSignature>> = HashMap::new();
        // Bodies for inference
        let mut fn_bodies: HashMap<String, hir::HirBlock> = HashMap::new();
        let mut method_bodies: HashMap<String, Vec<hir::HirBlock>> = HashMap::new();
        for item in &hir {
            if let HirItem::Fn(f) = item {
                fn_sigs.insert(f.signature.name.clone(), f.signature.clone());
                fn_bodies.insert(f.signature.name.clone(), f.body.clone());
            } else if let HirItem::Impl(imp) = item {
                for m in &imp.methods {
                    method_sigs.entry(m.signature.name.clone()).or_default().push(m.signature.clone());
                    method_bodies.entry(m.signature.name.clone()).or_default().push(m.body.clone());
                }
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
                        index.functions.entry(f.name.clone()).or_default().push(DefInfo { path: file.clone(), span: name_span, signature: sig });
                        // No local/type collection here; rely on typechecker/HIR upstream
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
                        } else if let OwnedTypeAliasBody::Record(fields) = &ta.aliased {
                            // Index record fields for hover/definition
                            for (field_name, field_ty) in fields {
                                let f_span = token_cache
                                    .get(file)
                                    .and_then(|toks| find_ident_span(toks, it.span, field_name))
                                    .unwrap_or(it.span);
                                index
                                    .record_fields
                                    .entry(field_name.clone())
                                    .or_default()
                                    .push((file.clone(), f_span, ta.name.clone(), field_ty.clone()));
                            }
                        }
                    }
                    OwnedItem::Trait(tr) => {
                        // Trait/interface indexing
                        let name_span = token_cache
                            .get(file)
                            .and_then(|toks| find_ident_span(toks, it.span, &tr.name))
                            .unwrap_or(it.span);
                        index.traits.insert(tr.name.clone(), (file.clone(), name_span));
                        // Collect trait methods as declarations for hover/definition
                        for m in &tr.methods {
                            let m_span = token_cache
                                .get(file)
                                .and_then(|toks| find_ident_span(toks, it.span, &m.name))
                                .unwrap_or(it.span);
                            index
                                .trait_methods
                                .entry(m.name.clone())
                                .or_default()
                                .push(DefInfo { path: file.clone(), span: m_span, signature: Some(HirFunctionSignature { name: m.name.clone(), params: m.params.iter().map(|(n, t)| (n.clone().unwrap_or_default(), Self::owned_type_to_hir_ty(t))).collect(), ret_type: m.ret_type.as_ref().map(Self::owned_type_to_hir_ty).unwrap_or(hir::Ty::Special(hir::SpecialTy::Unit)), effects: vec![] }) });
                        }
                    }
                    OwnedItem::Impl(imp) => {
                        // Index methods inside impl blocks
                        for m in &imp.methods {
                            let sig_opt = method_sigs.get(&m.name).and_then(|v| v.first()).cloned();
                            let name_span = token_cache
                                .get(file)
                                .and_then(|toks| find_ident_span(toks, it.span, &m.name))
                                .unwrap_or(it.span);
                            index
                                .methods
                                .entry(m.name.clone())
                                .or_default()
                                .push(DefInfo { path: file.clone(), span: name_span, signature: sig_opt });

                            // No local/type collection; rely on typechecker/HIR upstream
                        }
                    }
                    OwnedItem::ImportBlock { imports } => {
                        // Collect module names and their spans
                        for imp in imports {
                            let mod_name = imp
                                .alias
                                .clone()
                                .unwrap_or_else(|| imp.path.last().cloned().unwrap_or_default());
                            if mod_name.is_empty() { continue; }
                            let name_span = token_cache
                                .get(file)
                                .and_then(|toks| find_ident_span(toks, it.span, &mod_name))
                                .unwrap_or(it.span);
                            index.modules.insert(mod_name, (file.clone(), name_span, imp.path.clone()));
                        }
                    }
                    _ => {}
                }
            }
        }

        AnalysisResult { sources, ast, hir, diagnostics, index, token_spans: token_spans_map, hir_fn_bodies: fn_bodies, hir_method_bodies: method_bodies, hir_method_sigs: method_sigs }
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
            // Collect candidate HIR blocks from this file
            let mut candidate_blocks: Vec<&hir::HirBlock> = Vec::new();
            for (name, defs) in &analysis.index.functions {
                if defs.iter().any(|d| &d.path == &path) {
                    if let Some(b) = analysis.hir_fn_bodies.get(name) { candidate_blocks.push(b); }
                }
            }
            for (name, defs) in &analysis.index.methods {
                if defs.iter().any(|d| &d.path == &path) {
                    if let Some(v) = analysis.hir_method_bodies.get(name) { for b in v { candidate_blocks.push(b); } }
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

        if let Some((name, _id_span)) = Self::ident_at(&text, pos_params.position) {
            // Determine context of the identifier (function call vs method call vs field access)
            let mut is_method_ctx = false;
            let mut method_owner_last: Option<String> = None;
            let mut field_owner_last: Option<String> = None;
            let mut on_field_name_token_owner: Option<String> = None;
            // Exact struct field name token?
            if let Some(defs) = analysis.index.record_fields.get(&name) {
                for (p, sp, owner, _ty) in defs {
                    if p == &path {
                        if let Some(text_src) = analysis.sources.get(p) {
                            let char_offset_here = char_offset; // already computed above
                            if sp.start <= char_offset_here && char_offset_here < sp.end {
                                on_field_name_token_owner = Some(owner.clone());
                                break;
                            }
                        }
                    }
                }
            }
            if let Some(tok_idx) = token_index_opt {
                if let Some(items) = analysis.ast.get(&path) {
                    if let Some(expr) = Self::find_enclosing_expr_in_items(items, tok_idx) {
                        use ast_owned::OwnedExpr as OE;
                        match &expr.item {
                            OE::Call { fun, .. } => {
                                if let OE::FieldAccess { receiver, field } = &fun.item {
                                    if field == &name {
                                        is_method_ctx = true;
                                        if let Some(p) = Self::infer_receiver_type_path(&analysis, &path, receiver) {
                                            method_owner_last = p.last().cloned();
                                        }
                                    }
                                }
                            }
                            OE::FieldAccess { receiver, field } => {
                                if field == &name {
                                    if let Some(p) = Self::infer_receiver_type_path(&analysis, &path, receiver) {
                                        field_owner_last = p.last().cloned();
                                    }
                                }
                            }
                            _ => {}
                        }
                        if field_owner_last.is_none() {
                            if let Some(owner) = Self::infer_owner_from_enclosing_let(items, tok_idx) {
                                field_owner_last = Some(owner);
                            }
                        }
                    }
                }
            }
            // If exactly on a record field name, jump to that field first
            if let Some(owner_name) = on_field_name_token_owner.clone() {
                if let Some(defs) = analysis.index.record_fields.get(&name) {
                    let mut locs: Vec<lsp::Location> = Vec::new();
                    for (p, sp, own, _ty) in defs {
                        if own != &owner_name { continue; }
                        if let Some(src_text) = analysis.sources.get(p) {
                            let range = Self::simple_span_to_range(src_text, *sp);
                            let uri = match lsp::Url::from_file_path(p) { Ok(u) => u, Err(_) => continue };
                            locs.push(lsp::Location { uri, range });
                        }
                    }
                    if !locs.is_empty() {
                        return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
                    }
                }
            }
            // Prefer locals (including params) before fields/functions if not on a field name token
            if on_field_name_token_owner.is_none() {
                let selected_local_span = if let Some(det) = analysis.index.locals_detailed_by_file.get(&path).and_then(|m| m.get(&name)) {
                    det.iter().find_map(|(lsp, owner_sp)| {
                        if owner_sp.start <= char_offset && char_offset < owner_sp.end { Some(*lsp) } else { None }
                    })
                } else {
                    analysis.index.locals_by_file.get(&path).and_then(|m| m.get(&name)).copied()
                };
                if let Some(sp) = selected_local_span {
                    if let Some(src_text) = analysis.sources.get(&path) {
                        let range = Self::simple_span_to_range(src_text, sp);
                        let loc = lsp::Location { uri: lsp::Url::from_file_path(&path).unwrap(), range };
                        return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                    }
                }
            }
            // If identifier is a module name, jump to its import declaration span
            if let Some((p, s, _full)) = analysis.index.modules.get(&name) {
                if let Some(src_text) = analysis.sources.get(p) {
                    let range = Self::simple_span_to_range(src_text, *s);
                    let loc = lsp::Location { uri: lsp::Url::from_file_path(p).unwrap(), range };
                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                }
            }
            // First, handle import-context navigation
            if let Some(tok_idx) = token_index_opt {
                if let Some(items) = analysis.ast.get(&path) {
                    // Find import block containing this token index
                    if let Some(import_item) = items.iter().find(|it| matches!(it.item, OwnedItem::ImportBlock { .. }) && it.span.start <= tok_idx && tok_idx < it.span.end) {
                        if let OwnedItem::ImportBlock { imports } = &import_item.item {
                            let mut locations: Vec<lsp::Location> = Vec::new();
                            for imp in imports {
                                if let Some(pos_in_path) = imp.path.iter().position(|seg| seg == &name) {
                                    let prefix: Vec<String> = imp.path.iter().take(pos_in_path + 1).cloned().collect();
                                    if let Some(dir) = Self::resolve_module_dir_path(&prefix) {
                                        let targets = Self::list_module_targets(&dir);
                                        for t in targets {
                                            // Ensure absolute file URI; skip if cannot construct
                                            let uri = match lsp::Url::from_file_path(&t) {
                                                Ok(u) => u,
                                                Err(_) => continue,
                                            };
                                            // Jump to start of file
                                            let range = lsp::Range { start: lsp::Position { line: 0, character: 0 }, end: lsp::Position { line: 0, character: 0 } };
                                            locations.push(lsp::Location { uri, range });
                                        }
                                    }
                                }
                            }
                            if !locations.is_empty() {
                                return Ok(Some(if locations.len() == 1 {
                                    lsp::GotoDefinitionResponse::Scalar(locations.remove(0))
                                } else {
                                    lsp::GotoDefinitionResponse::Array(locations)
                                }));
                            }
                        }
                    }
                }
            }

            // HIR-first: try to use HIR to resolve precisely
            if let Some(tok_idx) = token_index_opt {
                if let Some(loc) = Self::goto_via_hir(&analysis, &path, tok_idx, &name, &text) { return Ok(Some(loc)); }
            }

            // Fallback to symbol-based navigation
            // Bare call identifiers should resolve to free functions only; avoid jumping to methods here
            if let Some(defs) = analysis.index.functions.get(&name) {
                let mut locs: Vec<lsp::Location> = Vec::new();
                for def in defs {
                    if let Some(src_text) = analysis.sources.get(&def.path) {
                        let range = Self::simple_span_to_range(src_text, def.span);
                        let uri = match lsp::Url::from_file_path(&def.path) { Ok(u) => u, Err(_) => continue };
                        locs.push(lsp::Location { uri, range });
                    }
                }
                if !locs.is_empty() {
                    return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
                }
            }
            if let Some((p, s)) = analysis.index.traits.get(&name) {
                if let Some(src_text) = analysis.sources.get(p) {
                    let range = Self::simple_span_to_range(src_text, *s);
                    let loc = lsp::Location { uri: lsp::Url::from_file_path(p).unwrap(), range };
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
            if is_method_ctx {
                // Prefer concrete impl when receiver type known; else trait
                if let Some(defs) = analysis.index.methods.get(&name) {
                    let mut locs: Vec<lsp::Location> = Vec::new();
                    for d in defs {
                        if let Some(sig) = &d.signature {
                            let matches_owner = match sig.params.first() {
                                Some((_n, t)) => Self::hir_type_to_path(t).and_then(|p| p.last().cloned()) == method_owner_last,
                                None => false,
                            };
                            if !matches_owner { continue; }
                        }
                        if let Some(src_text) = analysis.sources.get(&d.path) {
                            let range = Self::simple_span_to_range(src_text, d.span);
                            let uri = match lsp::Url::from_file_path(&d.path) { Ok(u) => u, Err(_) => continue };
                            locs.push(lsp::Location { uri, range });
                        }
                    }
                    if !locs.is_empty() {
                        return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
                    }
                }
                if let Some(defs) = analysis.index.trait_methods.get(&name) {
                    let mut locs: Vec<lsp::Location> = Vec::new();
                    for d in defs {
                        if let Some(src_text) = analysis.sources.get(&d.path) {
                            let range = Self::simple_span_to_range(src_text, d.span);
                            let uri = match lsp::Url::from_file_path(&d.path) { Ok(u) => u, Err(_) => continue };
                            locs.push(lsp::Location { uri, range });
                        }
                    }
                    if !locs.is_empty() {
                        return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
                    }
                }
            }
            if let Some(defs) = analysis.index.trait_methods.get(&name) {
                let mut locs: Vec<lsp::Location> = Vec::new();
                for d in defs {
                    if let Some(src_text) = analysis.sources.get(&d.path) {
                        let range = Self::simple_span_to_range(src_text, d.span);
                        let uri = match lsp::Url::from_file_path(&d.path) { Ok(u) => u, Err(_) => continue };
                        locs.push(lsp::Location { uri, range });
                    }
                }
                if !locs.is_empty() {
                    return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
                }
            }
            // Record field go-to-definition: only when we are in a field context or exactly at the field name
            if (field_owner_last.is_some() || on_field_name_token_owner.is_some()) &&
               (analysis.index.record_fields.get(&name).is_some()) {
                let defs = analysis.index.record_fields.get(&name).unwrap();
                let mut locs: Vec<lsp::Location> = Vec::new();
                for (p, sp, owner, _ty) in defs {
                    if let Some(ref want) = field_owner_last { if owner != want { continue; } }
                    if let Some(ref want) = on_field_name_token_owner { if owner != want { continue; } }
                    if let Some(src_text) = analysis.sources.get(p) {
                        let range = Self::simple_span_to_range(src_text, *sp);
                        let uri = match lsp::Url::from_file_path(p) { Ok(u) => u, Err(_) => continue };
                        locs.push(lsp::Location { uri, range });
                    }
                }
                if !locs.is_empty() {
                    return Ok(Some(if locs.len() == 1 { lsp::GotoDefinitionResponse::Scalar(locs.remove(0)) } else { lsp::GotoDefinitionResponse::Array(locs) }));
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


