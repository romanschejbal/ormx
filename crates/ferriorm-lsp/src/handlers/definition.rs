//! `textDocument/definition` — jump from a model/enum reference to its
//! declaration. Scope: field-type identifiers (e.g. `User` in `author User`).

use ferriorm_core::ast::SchemaFile;
use tower_lsp::lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, Range};

use crate::document::DocumentStore;

#[must_use]
pub fn run(docs: &DocumentStore, params: &GotoDefinitionParams) -> Option<GotoDefinitionResponse> {
    let uri = &params
        .text_document_position_params
        .text_document
        .uri
        .clone();
    let pos = params.text_document_position_params.position;
    docs.with(uri, |doc| {
        let ast = doc.ast.as_ref()?;
        let offset = doc.line_index.offset_of(&doc.text, pos);
        let (word, _) = identifier_at(&doc.text, offset)?;
        let target_span = resolve(ast, &word)?;
        let range: Range = doc.line_index.range_of(&doc.text, target_span);
        Some(GotoDefinitionResponse::Scalar(Location {
            uri: uri.clone(),
            range,
        }))
    })
    .flatten()
}

fn resolve(ast: &SchemaFile, name: &str) -> Option<ferriorm_core::ast::Span> {
    if let Some(m) = ast.models.iter().find(|m| m.name == name) {
        return Some(m.span);
    }
    if let Some(e) = ast.enums.iter().find(|e| e.name == name) {
        return Some(e.span);
    }
    None
}

fn identifier_at(text: &str, offset: usize) -> Option<(String, ferriorm_core::ast::Span)> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if offset > len {
        return None;
    }
    let is_id_byte = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = offset;
    while start > 0 && is_id_byte(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < len && is_id_byte(bytes[end]) {
        end += 1;
    }
    if start == end {
        return None;
    }
    if !bytes[start].is_ascii_alphabetic() && bytes[start] != b'_' {
        return None;
    }
    let word = std::str::from_utf8(&bytes[start..end]).ok()?.to_string();
    Some((word, ferriorm_core::ast::Span { start, end }))
}
