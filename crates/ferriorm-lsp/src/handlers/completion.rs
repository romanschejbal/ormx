//! `textDocument/completion` — context-aware suggestions.
//!
//! Three contexts are recognized by inspecting the bytes preceding the cursor:
//! - `@@<here>` inside a model body → block-attribute names.
//! - `@<here>` after an attribute marker → field-attribute names.
//! - whitespace after a field-name token → scalar types + model + enum names.

use ferriorm_core::ast::SchemaFile;
use tower_lsp::lsp_types::{
    CompletionItem, CompletionItemKind, CompletionParams, CompletionResponse,
};

use crate::document::DocumentStore;

const SCALAR_TYPES: &[&str] = &[
    "String", "Int", "BigInt", "Boolean", "Float", "DateTime", "Json", "Bytes",
];

const FIELD_ATTRS: &[&str] = &[
    "id",
    "unique",
    "default",
    "relation",
    "updatedAt",
    "map",
    "db",
];

const BLOCK_ATTRS: &[&str] = &["id", "unique", "index", "map"];

#[must_use]
pub fn run(docs: &DocumentStore, params: &CompletionParams) -> Option<CompletionResponse> {
    let uri = &params.text_document_position.text_document.uri;
    let pos = params.text_document_position.position;
    docs.with(uri, |doc| {
        let offset = doc.line_index.offset_of(&doc.text, pos);
        let bytes = doc.text.as_bytes();

        // Look at the immediately preceding non-whitespace marker.
        let two_before = bytes.get(offset.saturating_sub(2)).copied();
        let one_before = bytes.get(offset.saturating_sub(1)).copied();
        if one_before == Some(b'@') && two_before == Some(b'@') {
            return Some(items(BLOCK_ATTRS, "@@", CompletionItemKind::PROPERTY));
        }
        if one_before == Some(b'@') {
            return Some(items(FIELD_ATTRS, "@", CompletionItemKind::PROPERTY));
        }

        // After whitespace, suggest scalar types and known model/enum names.
        if matches!(one_before, Some(b' ' | b'\t'))
            && in_model_body(&doc.text, offset)
            && let Some(ast) = doc.ast.as_ref()
        {
            return Some(type_items(ast));
        }

        None
    })
    .flatten()
    .map(CompletionResponse::Array)
}

fn items(names: &[&str], prefix: &str, kind: CompletionItemKind) -> Vec<CompletionItem> {
    names
        .iter()
        .map(|name| CompletionItem {
            label: format!("{prefix}{name}"),
            kind: Some(kind),
            insert_text: Some((*name).to_string()),
            ..Default::default()
        })
        .collect()
}

fn type_items(ast: &SchemaFile) -> Vec<CompletionItem> {
    let mut out: Vec<CompletionItem> = SCALAR_TYPES
        .iter()
        .map(|name| CompletionItem {
            label: (*name).to_string(),
            kind: Some(CompletionItemKind::CLASS),
            ..Default::default()
        })
        .collect();
    for m in &ast.models {
        out.push(CompletionItem {
            label: m.name.clone(),
            kind: Some(CompletionItemKind::STRUCT),
            ..Default::default()
        });
    }
    for e in &ast.enums {
        out.push(CompletionItem {
            label: e.name.clone(),
            kind: Some(CompletionItemKind::ENUM),
            ..Default::default()
        });
    }
    out
}

/// Walk backwards from `offset` and decide whether we're inside a `model X { ... }`
/// body. The parser's AST may or may not be available (broken text), so we use
/// a brace-counting heuristic.
fn in_model_body(text: &str, offset: usize) -> bool {
    fn scan_back(bytes: &[u8], from: usize, pred: impl Fn(u8) -> bool) -> usize {
        let mut p = from;
        while p > 0 && pred(bytes[p - 1]) {
            p -= 1;
        }
        p
    }
    let is_ws = |b: u8| b.is_ascii_whitespace();
    let is_id = |b: u8| b.is_ascii_alphanumeric() || b == b'_';

    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut idx = offset;
    while idx > 0 {
        idx -= 1;
        match bytes[idx] {
            b'}' => depth -= 1,
            b'{' => {
                if depth == 0 {
                    let after_ws = scan_back(bytes, idx, is_ws);
                    let name_start = scan_back(bytes, after_ws, is_id);
                    let between_ws = scan_back(bytes, name_start, is_ws);
                    let kw_start = scan_back(bytes, between_ws, is_id);
                    return text
                        .get(kw_start..between_ws)
                        .is_some_and(|s| s == "model");
                }
                depth += 1;
            }
            _ => {}
        }
    }
    false
}
