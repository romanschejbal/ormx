//! ferriorm-lsp binary — speaks LSP over stdio.
//!
//! Logs go to stderr (controlled by `RUST_LOG`). The LSP transport on
//! stdout is reserved for JSON-RPC.

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    ferriorm_lsp::run_stdio().await;
}
