#![warn(clippy::pedantic)]

//! ferriorm Language Server Protocol implementation.
//!
//! Wires `tower-lsp` to the ferriorm parser, validator, and formatter,
//! exposing diagnostics, formatting, hover, completion, and go-to-definition
//! over `stdio`. The binary at `src/main.rs` is a thin wrapper around
//! [`run_stdio`].

pub mod conv;
pub mod document;
pub mod handlers;
pub mod server;

use tower_lsp::{LspService, Server};

/// Run the language server on stdin/stdout. Blocks until the client closes
/// the connection.
pub async fn run_stdio() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(server::Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
