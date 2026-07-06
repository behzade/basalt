use std::path::PathBuf;

use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types as lsp;

use crate::analysis::{AnalysisResult, Analyzer};
use crate::backend::Backend;
use crate::symbols;

pub async fn initialize(
    backend: &Backend,
    _: lsp::InitializeParams,
) -> LspResult<lsp::InitializeResult> {
    let server_caps = lsp::ServerCapabilities {
        text_document_sync: Some(lsp::TextDocumentSyncCapability::Kind(
            lsp::TextDocumentSyncKind::FULL,
        )),
        hover_provider: Some(lsp::HoverProviderCapability::Simple(true)),
        definition_provider: Some(lsp::OneOf::Left(true)),
        document_symbol_provider: Some(lsp::OneOf::Left(true)),
        workspace_symbol_provider: Some(lsp::OneOf::Left(true)),
        ..Default::default()
    };
    Ok(lsp::InitializeResult {
        capabilities: server_caps,
        server_info: None,
    })
}

pub async fn did_open(backend: &Backend, params: lsp::DidOpenTextDocumentParams) {
    if let Some(path) = Backend::url_to_path(&params.text_document.uri) {
        backend
            .open_files
            .write()
            .insert(path.clone(), params.text_document.text.clone());
        reanalyze_and_publish(backend, path, params.text_document.text).await;
    }
}

pub async fn did_change(backend: &Backend, params: lsp::DidChangeTextDocumentParams) {
    if let Some(path) = Backend::url_to_path(&params.text_document.uri) {
        if let Some(change) = params.content_changes.into_iter().last() {
            backend
                .open_files
                .write()
                .insert(path.clone(), change.text.clone());
            reanalyze_and_publish(backend, path, change.text).await;
        }
    }
}

pub async fn hover(backend: &Backend, params: lsp::HoverParams) -> LspResult<Option<lsp::Hover>> {
    let lsp::TextDocumentPositionParams {
        text_document,
        position,
    } = match params.text_document_position_params {
        p => p,
    };
    let path = match Backend::url_to_path(&text_document.uri) {
        Some(p) => p,
        None => return Ok(None),
    };
    let text = match backend.open_files.read().get(&path) {
        Some(t) => t.clone(),
        None => return Ok(None),
    };

    let analysis_map = backend.analysis.read();
    let analysis = match analysis_map.get(&path) {
        Some(a) => a.clone(),
        None => return Ok(None),
    };
    drop(analysis_map);

    let char_offset = symbols::position_to_offset(&text, position);
    let token_index_opt = analysis.token_spans.get(&path).and_then(|spans| {
        spans
            .iter()
            .position(|s| s.start <= char_offset && char_offset < s.end)
    });
    if let Some(tok_idx) = token_index_opt {
        let mut candidate_blocks: Vec<&basalt::hir::HirBlock> = Vec::new();
        for item in &analysis.hir {
            match item {
                basalt::hir::Item::Fn(f) if f.defined_in == path => candidate_blocks.push(&f.body),
                _ => {}
            }
        }
        for b in candidate_blocks {
            if let Some(expr) = symbols::find_hir_expr_in_block(b, tok_idx) {
                let contents = lsp::HoverContents::Markup(lsp::MarkupContent {
                    kind: lsp::MarkupKind::PlainText,
                    value: format!("{:?}", expr.ty),
                });
                return Ok(Some(lsp::Hover {
                    contents,
                    range: None,
                }));
            }
        }
    }
    Ok(None)
}

pub async fn goto_definition(
    backend: &Backend,
    params: lsp::GotoDefinitionParams,
) -> LspResult<Option<lsp::GotoDefinitionResponse>> {
    let pos_params = params.text_document_position_params;
    let path = match Backend::url_to_path(&pos_params.text_document.uri) {
        Some(p) => p,
        None => return Ok(None),
    };
    let text = match backend.open_files.read().get(&path) {
        Some(t) => t.clone(),
        None => return Ok(None),
    };

    let analysis_map = backend.analysis.read();
    let analysis = match analysis_map.get(&path) {
        Some(a) => a.clone(),
        None => return Ok(None),
    };
    drop(analysis_map);

    let char_offset = symbols::position_to_offset(&text, pos_params.position);
    let token_index_opt = analysis.token_spans.get(&path).and_then(|spans| {
        spans
            .iter()
            .position(|s| s.start <= char_offset && char_offset < s.end)
    });

    if let Some(tok_idx) = token_index_opt {
        let mut candidate_blocks: Vec<&basalt::hir::HirBlock> = Vec::new();
        for item in &analysis.hir {
            match item {
                basalt::hir::Item::Fn(f) if f.defined_in == path => candidate_blocks.push(&f.body),
                _ => {}
            }
        }
        for b in candidate_blocks {
            if let Some(expr) = symbols::find_hir_expr_in_block(b, tok_idx) {
                if let Some(res) = &expr.resolution {
                    match res {
                        basalt::hir::Resolution::Local {
                            name: _name,
                            decl_span,
                        } => {
                            let target_span = decl_span.unwrap_or(expr.span);
                            if let Some(src_text) = analysis.sources.get(&path) {
                                let ranges =
                                    analysis.token_spans.get(&path).cloned().unwrap_or_default();
                                let range =
                                    symbols::token_index_span_to_range(&text, &ranges, target_span);
                                if let Some(uri) = lsp::Url::from_file_path(&path).ok() {
                                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(
                                        lsp::Location { uri, range },
                                    )));
                                }
                            }
                        }
                        basalt::hir::Resolution::Field { owner, field } => {
                            if let Some(loc) =
                                symbols::find_struct_field_location(&analysis, owner, field)
                            {
                                return Ok(Some(lsp::GotoDefinitionResponse::Scalar(loc)));
                            }
                        }
                        basalt::hir::Resolution::Function { defined_in, span } => {
                            if let Some(src_text) = analysis.sources.get(defined_in) {
                                let tok_spans = analysis
                                    .token_spans
                                    .get(defined_in)
                                    .cloned()
                                    .unwrap_or_default();
                                let range =
                                    symbols::token_index_span_to_range(src_text, &tok_spans, *span);
                                if let Some(uri) = lsp::Url::from_file_path(defined_in).ok() {
                                    return Ok(Some(lsp::GotoDefinitionResponse::Scalar(
                                        lsp::Location { uri, range },
                                    )));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(None)
}

pub async fn document_symbol(
    backend: &Backend,
    params: lsp::DocumentSymbolParams,
) -> LspResult<Option<lsp::DocumentSymbolResponse>> {
    let path = match Backend::url_to_path(&params.text_document.uri) {
        Some(p) => p,
        None => return Ok(None),
    };
    let analysis_opt = {
        let analysis_map = backend.analysis.read();
        let direct = analysis_map.get(&path).cloned();
        match direct {
            Some(a) => Some(a),
            None => {
                let canon_req = match std::fs::canonicalize(&path) {
                    Ok(p) => p,
                    Err(_) => return Ok(None),
                };
                analysis_map.iter().find_map(|(k, v)| {
                    if std::fs::canonicalize(k).ok().as_ref() == Some(&canon_req) {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
            }
        }
    };
    let analysis = match analysis_opt {
        Some(a) => a,
        None => {
            let _ = backend
                .client
                .log_message(
                    lsp::MessageType::INFO,
                    "document_symbol: no analysis for file",
                )
                .await;
            return Ok(None);
        }
    };

    let symbols_vec = symbols::make_document_symbols_for_file(&analysis, &path);
    let _ = backend
        .client
        .log_message(
            lsp::MessageType::INFO,
            format!("document_symbol: built {} symbols", symbols_vec.len()),
        )
        .await;
    Ok(Some(lsp::DocumentSymbolResponse::Nested(symbols_vec)))
}

pub async fn workspace_symbol(
    backend: &Backend,
    params: lsp::WorkspaceSymbolParams,
) -> LspResult<Option<Vec<lsp::SymbolInformation>>> {
    let query = params.query.to_lowercase();
    let analyses: Vec<AnalysisResult> = {
        let map = backend.analysis.read();
        map.values().cloned().collect()
    };

    let mut out: Vec<lsp::SymbolInformation> = Vec::new();
    let mut file_count = 0usize;
    let mut ctx_count = 0usize;
    let mut local_count = 0usize;
    for analysis in analyses {
        let before = out.len();
        out.extend(symbols::build_workspace_symbols_from_analysis(
            &analysis, &query,
        ));
        file_count += analysis.sources.len();
        ctx_count += analysis.contexts.len();
        local_count += out.len().saturating_sub(before);
    }
    let _ = backend
        .client
        .log_message(
            lsp::MessageType::INFO,
            format!(
                "workspace_symbol: files={}, contexts={}, symbols_added={}",
                file_count, ctx_count, local_count
            ),
        )
        .await;
    Ok(Some(out))
}

pub async fn reanalyze_and_publish(backend: &Backend, root_path: PathBuf, text: String) {
    let _guard = backend.analyze_lock.lock().await;
    let previously_published: Vec<PathBuf> = {
        let map = backend.analysis.read();
        if let Some(prev) = map.get(&root_path) {
            prev.sources.keys().cloned().collect()
        } else {
            Vec::new()
        }
    };

    let result = Analyzer::analyze(&root_path, &text);
    backend
        .analysis
        .write()
        .insert(root_path.clone(), result.clone());

    let mut all_paths: std::collections::HashSet<PathBuf> =
        result.sources.keys().cloned().collect();
    for p in previously_published {
        all_paths.insert(p);
    }

    for path in all_paths {
        let uri = lsp::Url::from_file_path(&path)
            .unwrap_or_else(|_| lsp::Url::parse("file:///").unwrap());
        let diags = result.diagnostics.get(&path).cloned().unwrap_or_default();
        backend.client.publish_diagnostics(uri, diags, None).await;
    }
}
