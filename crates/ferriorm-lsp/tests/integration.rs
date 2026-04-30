//! Integration tests for the LSP handlers.
//!
//! These don't go through the JSON-RPC transport — instead they construct a
//! `DocumentStore` and call the handler functions with prepared params.
//! Transport-level behavior is verified by manual smoke tests.

#![allow(clippy::cast_possible_truncation)]

use ferriorm_lsp::document::DocumentStore;
use ferriorm_lsp::handlers;
use tower_lsp::lsp_types::{
    CompletionParams, CompletionResponse, DocumentFormattingParams, FormattingOptions,
    GotoDefinitionParams, GotoDefinitionResponse, Hover, HoverContents, HoverParams,
    PartialResultParams, Position, TextDocumentIdentifier, TextDocumentPositionParams, Url,
    WorkDoneProgressParams,
};

fn store_with(uri: &Url, text: &str) -> DocumentStore {
    let docs = DocumentStore::default();
    docs.upsert(uri.clone(), text.to_string(), 1);
    let _ = docs.refresh(uri);
    docs
}

#[test]
fn diagnostics_for_missing_primary_key() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = r#"
datasource db {
  provider = "postgresql"
  url      = "postgres://x"
}

model NoPk {
  email String
}
"#;
    let docs = DocumentStore::default();
    docs.upsert(uri.clone(), text.to_string(), 1);
    let diags = docs.refresh(&uri).unwrap();
    assert_eq!(diags.len(), 1);
    assert!(diags[0].message.contains("primary key"));
    // Range should be non-empty (points at the model).
    assert!(
        diags[0].range.end.line >= diags[0].range.start.line,
        "range should be valid"
    );
}

#[test]
fn formatting_returns_full_document_edit() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = "model X { id   String   @id }\n";
    let docs = store_with(&uri, text);

    let params = DocumentFormattingParams {
        text_document: TextDocumentIdentifier { uri: uri.clone() },
        options: FormattingOptions::default(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let edits = handlers::formatting::run(&docs, &params).expect("edits");
    assert_eq!(edits.len(), 1);
    assert!(edits[0].new_text.contains("model X {"));
    assert!(edits[0].new_text.contains("  id String @id"));
}

#[test]
fn hover_on_model_returns_signature() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = r#"datasource db {
  provider = "postgresql"
  url      = "postgres://x"
}

model Author {
  id String @id
  name String
}

model Post {
  id String @id
  author Author
}
"#;
    let docs = store_with(&uri, text);

    let line = text
        .lines()
        .position(|l| l.contains("author Author"))
        .unwrap() as u32;
    let col = text
        .lines()
        .nth(line as usize)
        .unwrap()
        .find("Author")
        .unwrap() as u32;
    let params = HoverParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line,
                character: col + 1,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let Hover { contents, .. } = handlers::hover::run(&docs, &params).expect("hover");
    let HoverContents::Markup(mk) = contents else {
        panic!("expected markup hover");
    };
    assert!(mk.value.contains("model Author"));
    assert!(mk.value.contains("name String"));
}

#[test]
fn completion_after_at_lists_field_attributes() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = "model X {\n  id String @";
    let docs = store_with(&uri, text);

    let line = 1;
    let col = text.lines().nth(1).unwrap().chars().count() as u32;
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line,
                character: col,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };
    let resp = handlers::completion::run(&docs, &params).expect("completion");
    let labels = labels(&resp);
    assert!(labels.iter().any(|l| l == "@id"));
    assert!(labels.iter().any(|l| l == "@unique"));
    assert!(labels.iter().any(|l| l == "@default"));
}

#[test]
fn completion_after_double_at_lists_block_attributes() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = "model X {\n  id String @id\n  @@";
    let docs = store_with(&uri, text);

    let line = 2;
    let col = 4;
    let params = CompletionParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position: Position {
                line,
                character: col,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
        context: None,
    };
    let resp = handlers::completion::run(&docs, &params).expect("completion");
    let labels = labels(&resp);
    assert!(labels.iter().any(|l| l == "@@index"));
    assert!(labels.iter().any(|l| l == "@@map"));
}

#[test]
fn goto_definition_jumps_to_model() {
    let uri = Url::parse("file:///test.ferriorm").unwrap();
    let text = r#"datasource db {
  provider = "postgresql"
  url      = "postgres://x"
}

model Author {
  id String @id
  name String
}

model Post {
  id String @id
  author Author
}
"#;
    let docs = store_with(&uri, text);

    let line = text
        .lines()
        .position(|l| l.contains("author Author"))
        .unwrap() as u32;
    let col = text
        .lines()
        .nth(line as usize)
        .unwrap()
        .find("Author")
        .unwrap() as u32;
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri: uri.clone() },
            position: Position {
                line,
                character: col + 1,
            },
        },
        work_done_progress_params: WorkDoneProgressParams::default(),
        partial_result_params: PartialResultParams::default(),
    };
    let resp = handlers::definition::run(&docs, &params).expect("definition");
    let target_line = match resp {
        GotoDefinitionResponse::Scalar(loc) => {
            assert_eq!(loc.uri, uri);
            loc.range.start.line
        }
        other => panic!("expected scalar goto, got {other:?}"),
    };
    let expected_line = text
        .lines()
        .position(|l| l.starts_with("model Author"))
        .unwrap() as u32;
    assert_eq!(target_line, expected_line);
}

fn labels(resp: &CompletionResponse) -> Vec<String> {
    match resp {
        CompletionResponse::Array(items) => items.iter().map(|i| i.label.clone()).collect(),
        CompletionResponse::List(list) => list.items.iter().map(|i| i.label.clone()).collect(),
    }
}
