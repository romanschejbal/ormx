//! `tower-lsp::LanguageServer` implementation.

use std::sync::Arc;
use tower_lsp::Client;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::{
    CompletionOptions, CompletionParams, CompletionResponse, DidChangeTextDocumentParams,
    DidCloseTextDocumentParams, DidOpenTextDocumentParams, DocumentFormattingParams,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverParams, HoverProviderCapability,
    InitializeParams, InitializeResult, InitializedParams, MessageType, OneOf, ServerCapabilities,
    ServerInfo, TextDocumentSyncCapability, TextDocumentSyncKind, TextEdit, Url,
};
use tower_lsp::{LanguageServer, lsp_types::WorkDoneProgressOptions};

use crate::document::DocumentStore;
use crate::handlers;

pub struct Backend {
    pub client: Client,
    pub docs: Arc<DocumentStore>,
}

impl Backend {
    #[must_use]
    pub fn new(client: Client) -> Self {
        Self {
            client,
            docs: Arc::new(DocumentStore::default()),
        }
    }

    /// Re-parse a document and publish its diagnostics.
    pub async fn refresh(&self, uri: &Url, version: Option<i32>) {
        let Some(diags) = self.docs.refresh(uri) else {
            return;
        };
        self.client
            .publish_diagnostics(uri.clone(), diags, version)
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                document_formatting_provider: Some(OneOf::Left(true)),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                completion_provider: Some(CompletionOptions {
                    trigger_characters: Some(vec!["@".into(), ".".into(), " ".into()]),
                    work_done_progress_options: WorkDoneProgressOptions::default(),
                    ..Default::default()
                }),
                definition_provider: Some(OneOf::Left(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "ferriorm-lsp".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "ferriorm-lsp ready")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let doc = params.text_document;
        self.docs.upsert(doc.uri.clone(), doc.text, doc.version);
        self.refresh(&doc.uri, Some(doc.version)).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let version = params.text_document.version;
        // FULL sync: take the last full text in the change list.
        if let Some(change) = params.content_changes.into_iter().next_back() {
            self.docs.upsert(uri.clone(), change.text, version);
            self.refresh(&uri, Some(version)).await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let uri = params.text_document.uri;
        self.docs.remove(&uri);
        // Clear any lingering diagnostics on the client.
        self.client.publish_diagnostics(uri, Vec::new(), None).await;
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        Ok(handlers::formatting::run(&self.docs, &params))
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        Ok(handlers::hover::run(&self.docs, &params))
    }

    async fn completion(&self, params: CompletionParams) -> Result<Option<CompletionResponse>> {
        Ok(handlers::completion::run(&self.docs, &params))
    }

    async fn goto_definition(
        &self,
        params: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        Ok(handlers::definition::run(&self.docs, &params))
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}
