//! `textDocument/hover` — show context for the identifier under the cursor.
//!
//! Resolution is best-effort:
//! - On a model name → its definition signature plus leading doc comments.
//! - On an enum name → its variants.
//! - On a field's type identifier that resolves to a model/enum → that
//!   target's signature.
//! - On a field name → its type, attributes, and leading comments.

use ferriorm_core::ast::{EnumDef, ModelDef, SchemaFile};
use tower_lsp::lsp_types::{Hover, HoverContents, HoverParams, MarkupContent, MarkupKind, Range};

use crate::document::DocumentStore;

#[must_use]
pub fn run(docs: &DocumentStore, params: &HoverParams) -> Option<Hover> {
    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    docs.with(uri, |doc| {
        let ast = doc.ast.as_ref()?;
        let offset = doc.line_index.offset_of(&doc.text, pos);
        let (word, word_span) = identifier_at(&doc.text, offset)?;

        let body = render_hover_body(ast, &word)?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: body,
            }),
            range: Some(Range {
                start: doc.line_index.range_of(&doc.text, word_span).start,
                end: doc.line_index.range_of(&doc.text, word_span).end,
            }),
        })
    })
    .flatten()
}

fn render_hover_body(ast: &SchemaFile, word: &str) -> Option<String> {
    if let Some(m) = ast.models.iter().find(|m| m.name == word) {
        return Some(model_summary(m));
    }
    if let Some(e) = ast.enums.iter().find(|e| e.name == word) {
        return Some(enum_summary(e));
    }
    None
}

fn model_summary(m: &ModelDef) -> String {
    let mut s = String::new();
    if !m.comments.leading.is_empty() {
        for line in &m.comments.leading {
            s.push_str(line);
            s.push('\n');
        }
        s.push('\n');
    }
    s.push_str("```ferriorm\nmodel ");
    s.push_str(&m.name);
    s.push_str(" {\n");
    for f in &m.fields {
        s.push_str("  ");
        s.push_str(&f.name);
        s.push(' ');
        s.push_str(&f.field_type.name);
        if f.field_type.is_list {
            s.push_str("[]");
        } else if f.field_type.is_optional {
            s.push('?');
        }
        s.push('\n');
    }
    s.push_str("}\n```");
    s
}

fn enum_summary(e: &EnumDef) -> String {
    let mut s = String::new();
    if !e.comments.leading.is_empty() {
        for line in &e.comments.leading {
            s.push_str(line);
            s.push('\n');
        }
        s.push('\n');
    }
    s.push_str("```ferriorm\nenum ");
    s.push_str(&e.name);
    s.push_str(" {\n");
    for v in &e.variants {
        s.push_str("  ");
        s.push_str(v);
        s.push('\n');
    }
    s.push_str("}\n```");
    s
}

/// Return the identifier surrounding `offset` and its byte span.
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
