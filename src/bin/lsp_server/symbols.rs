use std::fs;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types as lsp;

use basalt::hir;
use basalt::token::{self, SimpleSpan};

use crate::analysis::AnalysisResult;

pub fn simple_span_to_range(text: &str, span: SimpleSpan) -> lsp::Range {
    let start = offset_to_position(text, span.start);
    let end = offset_to_position(text, span.end);
    lsp::Range { start, end }
}

pub fn token_index_span_to_range(
    text: &str,
    token_spans: &[SimpleSpan],
    token_span: SimpleSpan,
) -> lsp::Range {
    let (start_char, end_char) = token_index_span_to_char_offsets(token_spans, token_span);
    let start = offset_to_position(text, start_char);
    let end = offset_to_position(text, end_char);
    lsp::Range { start, end }
}

pub fn token_index_span_to_char_offsets(
    token_spans: &[SimpleSpan],
    token_span: SimpleSpan,
) -> (usize, usize) {
    if token_spans.is_empty() {
        return (0, 0);
    }
    let last_idx = token_spans.len().saturating_sub(1);
    let start_idx = token_span.start.min(last_idx);
    let mut end_idx = if token_span.end == 0 {
        0
    } else {
        token_span.end.saturating_sub(1)
    };
    end_idx = end_idx.min(last_idx);
    let start_char = token_spans.get(start_idx).map(|s| s.start).unwrap_or(0);
    let mut end_char = token_spans
        .get(end_idx)
        .map(|s| s.end)
        .unwrap_or(start_char);
    if end_char < start_char {
        end_char = start_char;
    }
    (start_char, end_char)
}

pub fn find_name_char_offsets_in_span(
    text: &str,
    span_char: (usize, usize),
    name: &str,
) -> Option<(usize, usize)> {
    let (span_start, span_end) = span_char;
    if span_start >= span_end || span_start >= text.len() {
        return None;
    }
    let hay = &text[span_start.min(text.len())..span_end.min(text.len())];
    if name.is_empty() {
        return None;
    }
    if let Some(rel) = hay.find(name) {
        let s = span_start + rel;
        let e = s + name.len();
        Some((s, e))
    } else {
        None
    }
}

pub fn offset_to_position(text: &str, offset_chars: usize) -> lsp::Position {
    let mut remaining = offset_chars;
    let mut line: u32 = 0;
    for l in text.split_inclusive('\n') {
        let len = l.chars().count();
        if remaining < len {
            let col = l
                .chars()
                .take(remaining)
                .map(|c| c.len_utf16() as u32)
                .sum();
            return lsp::Position {
                line,
                character: col,
            };
        }
        remaining -= len;
        line += 1;
    }
    lsp::Position { line, character: 0 }
}

pub fn position_to_offset(text: &str, pos: lsp::Position) -> usize {
    let mut line_idx = 0u32;
    let mut offset_chars = 0usize;
    for l in text.split_inclusive('\n') {
        if line_idx == pos.line {
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

pub fn diagnostics_push(
    map: &mut std::collections::HashMap<PathBuf, Vec<lsp::Diagnostic>>,
    path: &Path,
    diag: lsp::Diagnostic,
) {
    map.entry(path.to_path_buf()).or_default().push(diag);
}

pub fn find_struct_field_location(
    analysis: &AnalysisResult,
    owner: &hir::OwnedPath,
    field: &str,
) -> Option<lsp::Location> {
    let owner_name = owner.last().cloned().unwrap_or_default();
    for item in &analysis.hir {
        if let hir::Item::Struct(s) = item {
            if s.name == owner_name {
                if let Some(f) = s.fields.iter().find(|f| f.name == field) {
                    if let Some(src_text) = analysis.sources.get(&s.defined_in) {
                        let range = simple_span_to_range(src_text, f.name_span.unwrap_or(s.span));
                        let uri = lsp::Url::from_file_path(&s.defined_in).ok()?;
                        return Some(lsp::Location { uri, range });
                    }
                }
            }
        }
    }
    None
}

pub fn make_symbol_range_for(
    text: &str,
    token_spans: &[SimpleSpan],
    span: SimpleSpan,
) -> (lsp::Range, lsp::Range) {
    let range = token_index_span_to_range(text, token_spans, span);
    (range, range)
}

pub fn hir_ty_to_string(ty: &hir::Ty) -> String {
    use hir::{AdtTy, PrimitiveTy, SpecialTy, Ty};
    match ty {
        Ty::Special(SpecialTy::Unit) => "()".to_string(),
        Ty::Special(SpecialTy::Never) => "!".to_string(),
        Ty::Special(SpecialTy::SelfType) => "Self".to_string(),
        Ty::Primitive(PrimitiveTy::Bool) => "bool".to_string(),
        Ty::Primitive(PrimitiveTy::Byte) => "byte".to_string(),
        Ty::Primitive(PrimitiveTy::I8) => "i8".to_string(),
        Ty::Primitive(PrimitiveTy::I16) => "i16".to_string(),
        Ty::Primitive(PrimitiveTy::I32) => "i32".to_string(),
        Ty::Primitive(PrimitiveTy::I64) => "i64".to_string(),
        Ty::Primitive(PrimitiveTy::U8) => "u8".to_string(),
        Ty::Primitive(PrimitiveTy::U16) => "u16".to_string(),
        Ty::Primitive(PrimitiveTy::U32) => "u32".to_string(),
        Ty::Primitive(PrimitiveTy::U64) => "u64".to_string(),
        Ty::Primitive(PrimitiveTy::F32) => "f32".to_string(),
        Ty::Primitive(PrimitiveTy::F64) => "f64".to_string(),
        Ty::Primitive(PrimitiveTy::Str) => "str".to_string(),
        Ty::Adt(AdtTy::Struct { name, generics })
        | Ty::Adt(AdtTy::Enum { name, generics })
        | Ty::Adt(AdtTy::Effect { name, generics }) => {
            let base = name.join("::");
            if generics.is_empty() {
                base
            } else {
                let gens: Vec<String> = generics.iter().map(hir_ty_to_string).collect();
                format!("{}<{}>", base, gens.join(", "))
            }
        }
        Ty::Array(inner) => format!("[{}]", hir_ty_to_string(inner)),
        Ty::Map { key, value } => format!("[{:?}:{}]", key, hir_ty_to_string(value)),
        Ty::Function {
            param_types,
            ret_type,
            effects,
        } => {
            let params = param_types
                .iter()
                .map(hir_ty_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            let ret = hir_ty_to_string(ret_type);
            if effects.is_empty() {
                format!("({}) -> {}", params, ret)
            } else {
                let effs = effects
                    .iter()
                    .map(hir_ty_to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({}) -> {} {{effects: {}}}", params, ret, effs)
            }
        }
        Ty::Handler { effects } => {
            let effects = effects
                .iter()
                .map(hir_ty_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("handler {{ {} }}", effects)
        }
        Ty::Generic(name) => name.clone(),
    }
}

pub fn join_path(path: &hir::OwnedPath) -> String {
    path.join("::")
}

pub fn item_name_and_span<'a>(item: &'a hir::Item) -> (String, &'a PathBuf, SimpleSpan) {
    match item {
        hir::Item::Fn(f) => (f.signature.name.clone(), &f.defined_in, f.span),
        hir::Item::Struct(s) => (s.name.clone(), &s.defined_in, s.span),
        hir::Item::Enum(e) => (e.name.clone(), &e.defined_in, e.span),
        hir::Item::TypeAlias(t) => (t.name.clone(), &t.defined_in, t.span),
        hir::Item::Effect(e) => (e.name.clone(), &e.defined_in, e.span),
        hir::Item::Handler(h) => (h.name.clone(), &h.defined_in, h.span),
    }
}

pub fn symbol_kind_for_item(item: &hir::Item) -> lsp::SymbolKind {
    match item {
        hir::Item::Fn(_) => lsp::SymbolKind::FUNCTION,
        hir::Item::Struct(_) => lsp::SymbolKind::STRUCT,
        hir::Item::Enum(_) => lsp::SymbolKind::ENUM,
        hir::Item::TypeAlias(ta) => symbol_kind_for_type_alias(ta),
        hir::Item::Effect(_) => lsp::SymbolKind::EVENT,
        hir::Item::Handler(_) => lsp::SymbolKind::NAMESPACE,
    }
}

pub fn symbol_kind_for_type_alias(ta: &hir::HirTypeAlias) -> lsp::SymbolKind {
    match &ta.aliased {
        hir::Ty::Adt(hir::AdtTy::Struct { .. }) => lsp::SymbolKind::STRUCT,
        hir::Ty::Adt(hir::AdtTy::Enum { .. }) => lsp::SymbolKind::ENUM,
        _ => lsp::SymbolKind::TYPE_PARAMETER,
    }
}

pub fn make_document_symbols_for_file(
    analysis: &AnalysisResult,
    path: &Path,
) -> Vec<lsp::DocumentSymbol> {
    let Some(text) = analysis.sources.get(path) else {
        return Vec::new();
    };
    let token_spans = analysis.token_spans.get(path).cloned().unwrap_or_default();
    let canon_this = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let mut out: Vec<lsp::DocumentSymbol> = Vec::new();
    for item in &analysis.hir {
        match item {
            hir::Item::Fn(f)
                if fs::canonicalize(&f.defined_in).unwrap_or_else(|_| f.defined_in.clone())
                    == canon_this =>
            {
                let (range, _selection_range) = make_symbol_range_for(text, &token_spans, f.span);
                let (start_char, end_char) = token_index_span_to_char_offsets(&token_spans, f.span);
                let sel =
                    find_name_char_offsets_in_span(text, (start_char, end_char), &f.signature.name)
                        .unwrap_or((start_char, start_char));
                let selection_range = lsp::Range {
                    start: offset_to_position(text, sel.0),
                    end: offset_to_position(text, sel.1),
                };
                let mut children: Vec<lsp::DocumentSymbol> = Vec::new();
                for p in &f.signature.params {
                    let p_name = p.name.clone();
                    let p_detail = Some(hir_ty_to_string(&p.ty));
                    let (pr, psr) = if let Some(sp) = p.span {
                        make_symbol_range_for(text, &token_spans, sp)
                    } else {
                        (range, selection_range)
                    };
                    children.push(lsp::DocumentSymbol {
                        name: p_name,
                        detail: p_detail,
                        kind: lsp::SymbolKind::VARIABLE,
                        tags: None,
                        deprecated: None,
                        range: pr,
                        selection_range: psr,
                        children: None,
                    });
                }
                out.push(lsp::DocumentSymbol {
                    name: f.signature.name.clone(),
                    detail: Some(format!(
                        "{}",
                        hir_ty_to_string(&hir::Ty::Function {
                            param_types: f.signature.params.iter().map(|p| p.ty.clone()).collect(),
                            ret_type: Box::new(f.signature.ret_type.clone()),
                            effects: f.signature.effects.clone()
                        })
                    )),
                    kind: lsp::SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }
            hir::Item::Struct(s)
                if fs::canonicalize(&s.defined_in).unwrap_or_else(|_| s.defined_in.clone())
                    == canon_this =>
            {
                let (range, _selection_range) = make_symbol_range_for(text, &token_spans, s.span);
                let (start_char, end_char) = token_index_span_to_char_offsets(&token_spans, s.span);
                let sel = find_name_char_offsets_in_span(text, (start_char, end_char), &s.name)
                    .unwrap_or((start_char, start_char));
                let selection_range = lsp::Range {
                    start: offset_to_position(text, sel.0),
                    end: offset_to_position(text, sel.1),
                };
                let mut children: Vec<lsp::DocumentSymbol> = Vec::new();
                for field in &s.fields {
                    let (fr, fsr) = if let Some(nsp) = field.name_span {
                        make_symbol_range_for(text, &token_spans, nsp)
                    } else {
                        (range, selection_range)
                    };
                    children.push(lsp::DocumentSymbol {
                        name: field.name.clone(),
                        detail: Some(hir_ty_to_string(&field.ty)),
                        kind: lsp::SymbolKind::FIELD,
                        tags: None,
                        deprecated: None,
                        range: fr,
                        selection_range: fsr,
                        children: None,
                    });
                }
                out.push(lsp::DocumentSymbol {
                    name: s.name.clone(),
                    detail: None,
                    kind: lsp::SymbolKind::STRUCT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }
            hir::Item::Enum(e)
                if fs::canonicalize(&e.defined_in).unwrap_or_else(|_| e.defined_in.clone())
                    == canon_this =>
            {
                let (range, _selection_range) = make_symbol_range_for(text, &token_spans, e.span);
                let (start_char, end_char) = token_index_span_to_char_offsets(&token_spans, e.span);
                let sel = find_name_char_offsets_in_span(text, (start_char, end_char), &e.name)
                    .unwrap_or((start_char, start_char));
                let selection_range = lsp::Range {
                    start: offset_to_position(text, sel.0),
                    end: offset_to_position(text, sel.1),
                };
                let mut children: Vec<lsp::DocumentSymbol> = Vec::new();
                for v in &e.variants {
                    let (vr, vsr) = if let Some(nsp) = v.name_span {
                        make_symbol_range_for(text, &token_spans, nsp)
                    } else {
                        (range, selection_range)
                    };
                    children.push(lsp::DocumentSymbol {
                        name: v.name.clone(),
                        detail: None,
                        kind: lsp::SymbolKind::ENUM_MEMBER,
                        tags: None,
                        deprecated: None,
                        range: vr,
                        selection_range: vsr,
                        children: None,
                    });
                }
                out.push(lsp::DocumentSymbol {
                    name: e.name.clone(),
                    detail: None,
                    kind: lsp::SymbolKind::ENUM,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }
            hir::Item::Effect(eff)
                if fs::canonicalize(&eff.defined_in).unwrap_or_else(|_| eff.defined_in.clone())
                    == canon_this =>
            {
                let (range, _selection_range) = make_symbol_range_for(text, &token_spans, eff.span);
                let (start_char, end_char) =
                    token_index_span_to_char_offsets(&token_spans, eff.span);
                let sel = find_name_char_offsets_in_span(text, (start_char, end_char), &eff.name)
                    .unwrap_or((start_char, start_char));
                let selection_range = lsp::Range {
                    start: offset_to_position(text, sel.0),
                    end: offset_to_position(text, sel.1),
                };
                let mut children: Vec<lsp::DocumentSymbol> = Vec::new();
                for op in &eff.operations {
                    children.push(lsp::DocumentSymbol {
                        name: op.name.clone(),
                        detail: Some(format!(
                            "{}",
                            hir_ty_to_string(&hir::Ty::Function {
                                param_types: op.params.iter().map(|p| p.ty.clone()).collect(),
                                ret_type: Box::new(op.ret_type.clone()),
                                effects: op.effects.clone()
                            })
                        )),
                        kind: lsp::SymbolKind::METHOD,
                        tags: None,
                        deprecated: None,
                        range,
                        selection_range,
                        children: None,
                    });
                }
                out.push(lsp::DocumentSymbol {
                    name: eff.name.clone(),
                    detail: None,
                    kind: lsp::SymbolKind::EVENT,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }
            hir::Item::Handler(h)
                if fs::canonicalize(&h.defined_in).unwrap_or_else(|_| h.defined_in.clone())
                    == canon_this =>
            {
                let (range, _selection_range) = make_symbol_range_for(text, &token_spans, h.span);
                let (start_char, end_char) = token_index_span_to_char_offsets(&token_spans, h.span);
                let sel = find_name_char_offsets_in_span(text, (start_char, end_char), &h.name)
                    .unwrap_or((start_char, start_char));
                let selection_range = lsp::Range {
                    start: offset_to_position(text, sel.0),
                    end: offset_to_position(text, sel.1),
                };
                let mut children: Vec<lsp::DocumentSymbol> = Vec::new();
                for f in &h.functions {
                    let (fr, fsr) = make_symbol_range_for(text, &token_spans, f.span);
                    let mut params_children: Vec<lsp::DocumentSymbol> = Vec::new();
                    for p in &f.signature.params {
                        let (pr, psr) = if let Some(sp) = p.span {
                            make_symbol_range_for(text, &token_spans, sp)
                        } else {
                            (fr, fsr)
                        };
                        params_children.push(lsp::DocumentSymbol {
                            name: p.name.clone(),
                            detail: Some(hir_ty_to_string(&p.ty)),
                            kind: lsp::SymbolKind::VARIABLE,
                            tags: None,
                            deprecated: None,
                            range: pr,
                            selection_range: psr,
                            children: None,
                        });
                    }
                    children.push(lsp::DocumentSymbol {
                        name: f.signature.name.clone(),
                        detail: Some(format!(
                            "{}",
                            hir_ty_to_string(&hir::Ty::Function {
                                param_types: f
                                    .signature
                                    .params
                                    .iter()
                                    .map(|p| p.ty.clone())
                                    .collect(),
                                ret_type: Box::new(f.signature.ret_type.clone()),
                                effects: f.signature.effects.clone()
                            })
                        )),
                        kind: lsp::SymbolKind::FUNCTION,
                        tags: None,
                        deprecated: None,
                        range: fr,
                        selection_range: fsr,
                        children: if params_children.is_empty() {
                            None
                        } else {
                            Some(params_children)
                        },
                    });
                }
                out.push(lsp::DocumentSymbol {
                    name: h.name.clone(),
                    detail: None,
                    kind: lsp::SymbolKind::NAMESPACE,
                    tags: None,
                    deprecated: None,
                    range,
                    selection_range,
                    children: if children.is_empty() {
                        None
                    } else {
                        Some(children)
                    },
                });
            }
            _ => {}
        }
    }
    out
}

pub fn build_workspace_symbols_from_analysis(
    analysis: &AnalysisResult,
    query_lc: &str,
) -> Vec<lsp::SymbolInformation> {
    let mut out: Vec<lsp::SymbolInformation> = Vec::new();
    for item in &analysis.hir {
        let (name, defined_in, span) = item_name_and_span(item);
        if !query_lc.is_empty() && !name.to_lowercase().contains(query_lc) {
            continue;
        }
        if let Some(text) = analysis.sources.get(defined_in) {
            let token_spans = analysis
                .token_spans
                .get(defined_in)
                .cloned()
                .unwrap_or_default();
            let (schar, echar) = token_index_span_to_char_offsets(&token_spans, span);
            let range = lsp::Range {
                start: offset_to_position(text, schar),
                end: offset_to_position(text, echar),
            };
            if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                out.push(lsp::SymbolInformation {
                    name: name.clone(),
                    kind: symbol_kind_for_item(item),
                    tags: None,
                    deprecated: None,
                    location: lsp::Location {
                        uri: uri.clone(),
                        range,
                    },
                    container_name: None,
                });
            }
            match item {
                hir::Item::Fn(f) => {
                    if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                        let mut lets: Vec<(String, SimpleSpan)> = Vec::new();
                        collect_lets_in_block(&f.body, &mut lets);
                        for (vname, vspan) in lets {
                            let (schar, echar) =
                                token_index_span_to_char_offsets(&token_spans, vspan);
                            let vr = lsp::Range {
                                start: offset_to_position(text, schar),
                                end: offset_to_position(text, echar),
                            };
                            out.push(lsp::SymbolInformation {
                                name: vname,
                                kind: lsp::SymbolKind::VARIABLE,
                                tags: None,
                                deprecated: None,
                                location: lsp::Location {
                                    uri: uri.clone(),
                                    range: vr,
                                },
                                container_name: Some(f.signature.name.clone()),
                            });
                        }
                    }
                }
                hir::Item::Enum(en) => {
                    if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                        for v in &en.variants {
                            let vr = if let Some(vsp) = v.name_span {
                                let (schar, echar) =
                                    token_index_span_to_char_offsets(&token_spans, vsp);
                                lsp::Range {
                                    start: offset_to_position(text, schar),
                                    end: offset_to_position(text, echar),
                                }
                            } else {
                                range
                            };
                            out.push(lsp::SymbolInformation {
                                name: v.name.clone(),
                                kind: lsp::SymbolKind::ENUM_MEMBER,
                                tags: None,
                                deprecated: None,
                                location: lsp::Location {
                                    uri: uri.clone(),
                                    range: vr,
                                },
                                container_name: Some(en.name.clone()),
                            });
                        }
                    }
                }
                hir::Item::Effect(eff) => {
                    if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                        for op in &eff.operations {
                            out.push(lsp::SymbolInformation {
                                name: op.name.clone(),
                                kind: lsp::SymbolKind::METHOD,
                                tags: None,
                                deprecated: None,
                                location: lsp::Location {
                                    uri: uri.clone(),
                                    range,
                                },
                                container_name: Some(eff.name.clone()),
                            });
                        }
                    }
                }
                hir::Item::Handler(h) => {
                    if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                        for f in &h.functions {
                            let (schar, echar) =
                                token_index_span_to_char_offsets(&token_spans, f.span);
                            let fr = lsp::Range {
                                start: offset_to_position(text, schar),
                                end: offset_to_position(text, echar),
                            };
                            out.push(lsp::SymbolInformation {
                                name: f.signature.name.clone(),
                                kind: lsp::SymbolKind::FUNCTION,
                                tags: None,
                                deprecated: None,
                                location: lsp::Location {
                                    uri: uri.clone(),
                                    range: fr,
                                },
                                container_name: Some(h.name.clone()),
                            });
                            let mut lets: Vec<(String, SimpleSpan)> = Vec::new();
                            collect_lets_in_block(&f.body, &mut lets);
                            for (vname, vspan) in lets {
                                let (schar, echar) =
                                    token_index_span_to_char_offsets(&token_spans, vspan);
                                let vr = lsp::Range {
                                    start: offset_to_position(text, schar),
                                    end: offset_to_position(text, echar),
                                };
                                out.push(lsp::SymbolInformation {
                                    name: vname,
                                    kind: lsp::SymbolKind::VARIABLE,
                                    tags: None,
                                    deprecated: None,
                                    location: lsp::Location {
                                        uri: uri.clone(),
                                        range: vr,
                                    },
                                    container_name: Some(f.signature.name.clone()),
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    for ctx in &analysis.contexts {
        let defined_in = &ctx.defined_in;
        let Some(text) = analysis.sources.get(defined_in) else {
            continue;
        };
        let token_spans = analysis
            .token_spans
            .get(defined_in)
            .cloned()
            .unwrap_or_default();
        let container_name: Option<String> = analysis.hir.iter().find_map(|it| match it {
            hir::Item::Fn(f) if f.context_id == Some(ctx.id) => Some(f.signature.name.clone()),
            hir::Item::Struct(s) if s.context_id == Some(ctx.id) => Some(s.name.clone()),
            hir::Item::Enum(e) if e.context_id == Some(ctx.id) => Some(e.name.clone()),
            _ => None,
        });
        for sym in &ctx.symbols {
            use hir::HirSymbolKind as HK;
            let lsp_kind = match sym.kind {
                HK::Variable => Some(lsp::SymbolKind::VARIABLE),
                HK::Param => Some(lsp::SymbolKind::VARIABLE),
                HK::EnumVariant => Some(lsp::SymbolKind::ENUM_MEMBER),
                _ => None,
            };
            if lsp_kind.is_none() {
                continue;
            }
            if !query_lc.is_empty() && !sym.name.to_lowercase().contains(query_lc) {
                continue;
            }
            let (schar, echar) =
                token_index_span_to_char_offsets(&token_spans, sym.name_span.unwrap_or(sym.span));
            let range = lsp::Range {
                start: offset_to_position(text, schar),
                end: offset_to_position(text, echar),
            };
            if let Ok(uri) = lsp::Url::from_file_path(defined_in) {
                out.push(lsp::SymbolInformation {
                    name: sym.name.clone(),
                    kind: lsp_kind.unwrap(),
                    tags: None,
                    deprecated: None,
                    location: lsp::Location {
                        uri: uri.clone(),
                        range,
                    },
                    container_name: container_name.clone(),
                });
            }
        }
    }
    out
}

pub fn find_hir_expr_in_block(block: &hir::HirBlock, tok_idx: usize) -> Option<hir::Expr> {
    let mut best: Option<hir::Expr> = None;
    for s in &block.stmts {
        if let Some(e) = find_hir_expr_in_stmt(s, tok_idx) {
            best = Some(e);
            break;
        }
    }
    if best.is_none() {
        if let Some(le) = &block.last_expr {
            if let Some(e) = find_hir_expr(le, tok_idx) {
                best = Some(e);
            }
        }
    }
    best
}

pub fn find_hir_expr_in_stmt(stmt: &hir::Stmt, tok_idx: usize) -> Option<hir::Expr> {
    use hir::Stmt as HS;
    match stmt {
        HS::Let { value, .. } => {
            if let Some(v) = value {
                return find_hir_expr(v, tok_idx);
            }
            None
        }
        HS::Assign { lhs, rhs, .. } => {
            if let Some(e) = find_hir_expr(lhs, tok_idx) {
                return Some(e);
            }
            if let Some(e) = find_hir_expr(rhs, tok_idx) {
                return Some(e);
            }
            None
        }
        HS::Return { value, .. } => {
            if let Some(v) = value {
                return find_hir_expr(v, tok_idx);
            }
            None
        }
        HS::Expr { expr, .. } => find_hir_expr(expr, tok_idx),
        HS::Error { .. } => None,
    }
}

pub fn find_hir_expr(expr: &hir::Expr, tok_idx: usize) -> Option<hir::Expr> {
    if !(expr.span.start <= tok_idx && tok_idx < expr.span.end) {
        return None;
    }
    use hir::ExprKind as EK;
    match &expr.kind {
        EK::Unary { rhs, .. } => find_hir_expr(rhs, tok_idx).or_else(|| Some(expr.clone())),
        EK::Binary { lhs, rhs, .. } => find_hir_expr(lhs, tok_idx)
            .or_else(|| find_hir_expr(rhs, tok_idx))
            .or_else(|| Some(expr.clone())),
        EK::FieldAccess { receiver, .. } => {
            find_hir_expr(receiver, tok_idx).or_else(|| Some(expr.clone()))
        }
        EK::Call { fun, args } => {
            if let Some(e) = find_hir_expr(fun, tok_idx) {
                return Some(e);
            }
            for a in args {
                if let Some(e) = find_hir_expr(a, tok_idx) {
                    return Some(e);
                }
            }
            Some(expr.clone())
        }
        EK::StructInit { fields, .. } => {
            for (_n, e) in fields {
                if let Some(found) = find_hir_expr(e, tok_idx) {
                    return Some(found);
                }
            }
            Some(expr.clone())
        }
        EK::Array(es) => {
            for e in es {
                if let Some(f) = find_hir_expr(e, tok_idx) {
                    return Some(f);
                }
            }
            Some(expr.clone())
        }
        EK::Map(kvs) => {
            for (k, v) in kvs {
                if let Some(f) = find_hir_expr(k, tok_idx) {
                    return Some(f);
                }
                if let Some(f) = find_hir_expr(v, tok_idx) {
                    return Some(f);
                }
            }
            Some(expr.clone())
        }
        EK::If {
            cond,
            then_block,
            else_block,
        } => {
            if let Some(e) = find_hir_expr(cond, tok_idx) {
                return Some(e);
            }
            if let Some(e) = find_hir_expr_in_block(then_block, tok_idx) {
                return Some(e);
            }
            if let Some(eb) = else_block {
                if let Some(e) = find_hir_expr(eb, tok_idx) {
                    return Some(e);
                }
            }
            Some(expr.clone())
        }
        EK::While { cond, body } => {
            if let Some(e) = find_hir_expr(cond, tok_idx) {
                return Some(e);
            }
            if let Some(e) = find_hir_expr_in_block(body, tok_idx) {
                return Some(e);
            }
            Some(expr.clone())
        }
        EK::Match { scrutinee, arms } => {
            if let Some(e) = find_hir_expr(scrutinee, tok_idx) {
                return Some(e);
            }
            for (_p, arm) in arms {
                if let Some(e) = find_hir_expr(arm, tok_idx) {
                    return Some(e);
                }
            }
            Some(expr.clone())
        }
        EK::Handle { body, .. } => {
            find_hir_expr_in_block(body, tok_idx).or_else(|| Some(expr.clone()))
        }
        EK::Cast { expr: inner } => find_hir_expr(inner, tok_idx).or_else(|| Some(expr.clone())),
        _ => Some(expr.clone()),
    }
}

pub fn collect_lets_in_block(block: &hir::HirBlock, out: &mut Vec<(String, SimpleSpan)>) {
    use hir::Stmt as HS;
    for s in &block.stmts {
        match s {
            HS::Let {
                name,
                span,
                name_span,
                ..
            } => {
                out.push((name.clone(), name_span.unwrap_or(*span)));
            }
            HS::Assign { lhs, rhs, .. } => {
                collect_lets_in_expr(lhs, out);
                collect_lets_in_expr(rhs, out);
            }
            HS::Return { value, .. } => {
                if let Some(v) = value {
                    collect_lets_in_expr(v, out);
                }
            }
            HS::Expr { expr, .. } => {
                collect_lets_in_expr(expr, out);
            }
            HS::Error { .. } => {}
        }
    }
    if let Some(last) = &block.last_expr {
        collect_lets_in_expr(last, out);
    }
}

pub fn collect_lets_in_expr(expr: &hir::Expr, out: &mut Vec<(String, SimpleSpan)>) {
    use hir::ExprKind as EK;
    match &expr.kind {
        EK::Block(b) => collect_lets_in_block(b, out),
        EK::If {
            cond,
            then_block,
            else_block,
        } => {
            collect_lets_in_expr(cond, out);
            collect_lets_in_block(then_block, out);
            if let Some(eb) = else_block {
                collect_lets_in_expr(eb, out);
            }
        }
        EK::While { cond, body } => {
            collect_lets_in_expr(cond, out);
            collect_lets_in_block(body, out);
        }
        EK::Match { scrutinee, arms } => {
            collect_lets_in_expr(scrutinee, out);
            for (_p, arm) in arms {
                collect_lets_in_expr(arm, out);
            }
        }
        EK::Unary { rhs, .. } => collect_lets_in_expr(rhs, out),
        EK::Binary { lhs, rhs, .. } => {
            collect_lets_in_expr(lhs, out);
            collect_lets_in_expr(rhs, out);
        }
        EK::FieldAccess { receiver, .. } => collect_lets_in_expr(receiver, out),
        EK::Call { fun, args } => {
            collect_lets_in_expr(fun, out);
            for a in args {
                collect_lets_in_expr(a, out);
            }
        }
        EK::StructInit { fields, .. } => {
            for (_n, e) in fields {
                collect_lets_in_expr(e, out);
            }
        }
        EK::Array(es) => {
            for e in es {
                collect_lets_in_expr(e, out);
            }
        }
        EK::Map(kvs) => {
            for (k, v) in kvs {
                collect_lets_in_expr(k, out);
                collect_lets_in_expr(v, out);
            }
        }
        EK::Handler(_) => {}
        EK::Handle { body, handler } => {
            collect_lets_in_block(body, out);
            collect_lets_in_expr(handler, out);
        }
        EK::Cast { expr: inner } => collect_lets_in_expr(inner, out),
        EK::Path(_) | EK::Literal(..) | EK::Perform { .. } | EK::Error => {}
        EK::FnLiteral(f) => {} // is this right?
    }
}
