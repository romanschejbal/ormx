//! `textDocument/formatting` — full-document edit produced by `ferriorm-fmt`.

use tower_lsp::lsp_types::{DocumentFormattingParams, Position, Range, TextEdit};

use crate::document::DocumentStore;

#[must_use]
pub fn run(docs: &DocumentStore, params: &DocumentFormattingParams) -> Option<Vec<TextEdit>> {
    let uri = &params.text_document.uri;
    docs.with(uri, |doc| {
        let ast = doc.ast.as_ref()?;
        let formatted = ferriorm_fmt::format_ast(ast);
        if formatted == doc.text {
            return Some(Vec::new());
        }
        let end = doc.line_index.end_position(&doc.text);
        Some(vec![TextEdit {
            range: Range {
                start: Position::new(0, 0),
                end,
            },
            new_text: formatted,
        }])
    })
    .flatten()
}
