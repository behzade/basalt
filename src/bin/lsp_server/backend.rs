use std::collections::HashMap;
use std::path::PathBuf;

use parking_lot::RwLock;
use tokio::sync::Mutex;
use tower_lsp::lsp_types as lsp;
use tower_lsp::{Client, LanguageServer};

use crate::analysis::{AnalysisResult, Analyzer};
use crate::handlers;
use crate::symbols;

pub struct Backend {
    pub client: Client,
    pub open_files: RwLock<HashMap<PathBuf, String>>, // latest open text per file
    pub analysis: RwLock<HashMap<PathBuf, AnalysisResult>>, // last analysis per root file
    pub analyze_lock: Mutex<()>, // serialize analyses
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            open_files: RwLock::new(HashMap::new()),
            analysis: RwLock::new(HashMap::new()),
            analyze_lock: Mutex::new(()),
        }
    }

    pub fn url_to_path(url: &lsp::Url) -> Option<PathBuf> {
        url.to_file_path().ok()
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: lsp::InitializeParams) -> tower_lsp::jsonrpc::Result<lsp::InitializeResult> {
        handlers::initialize(self, params).await
    }

    async fn initialized(&self, _: lsp::InitializedParams) {
        let _ = self.client.log_message(lsp::MessageType::INFO, "basalt LSP initialized").await;
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> { Ok(()) }

    async fn did_open(&self, params: lsp::DidOpenTextDocumentParams) {
        handlers::did_open(self, params).await;
    }

    async fn did_change(&self, params: lsp::DidChangeTextDocumentParams) {
        handlers::did_change(self, params).await;
    }

    async fn hover(&self, params: lsp::HoverParams) -> tower_lsp::jsonrpc::Result<Option<lsp::Hover>> {
        handlers::hover(self, params).await
    }

    async fn goto_definition(&self, params: lsp::GotoDefinitionParams) -> tower_lsp::jsonrpc::Result<Option<lsp::GotoDefinitionResponse>> {
        handlers::goto_definition(self, params).await
    }

    async fn document_symbol(&self, params: lsp::DocumentSymbolParams) -> tower_lsp::jsonrpc::Result<Option<lsp::DocumentSymbolResponse>> {
        handlers::document_symbol(self, params).await
    }

    async fn symbol(&self, params: lsp::WorkspaceSymbolParams) -> tower_lsp::jsonrpc::Result<Option<Vec<lsp::SymbolInformation>>> {
        handlers::workspace_symbol(self, params).await
    }
}


