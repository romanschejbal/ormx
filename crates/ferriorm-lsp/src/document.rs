//! Per-document state: source text, parsed AST, validated schema, diagnostics.

use dashmap::DashMap;
use ferriorm_core::ast::SchemaFile;
use ferriorm_core::schema::Schema;
use ferriorm_parser::error::ParseError;
use tower_lsp::lsp_types::{Diagnostic, Url};

use crate::conv::LineIndex;

#[derive(Debug)]
pub struct DocState {
    pub text: String,
    pub version: i32,
    pub line_index: LineIndex,
    pub ast: Option<SchemaFile>,
    pub schema: Option<Schema>,
    pub diagnostics: Vec<Diagnostic>,
}

impl DocState {
    #[must_use]
    pub fn new(text: String, version: i32) -> Self {
        let line_index = LineIndex::new(&text);
        Self {
            text,
            version,
            line_index,
            ast: None,
            schema: None,
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Default, Debug)]
pub struct DocumentStore {
    docs: DashMap<Url, DocState>,
}

impl DocumentStore {
    pub fn upsert(&self, uri: Url, text: String, version: i32) {
        self.docs.insert(uri, DocState::new(text, version));
    }

    pub fn remove(&self, uri: &Url) {
        self.docs.remove(uri);
    }

    /// Run a closure with mutable access to the doc, refreshing the parse and
    /// validation. Returns the freshly computed diagnostics.
    #[must_use]
    pub fn refresh(&self, uri: &Url) -> Option<Vec<Diagnostic>> {
        let mut entry = self.docs.get_mut(uri)?;
        entry.line_index = LineIndex::new(&entry.text);
        let (ast, schema, diagnostics) = parse_and_diagnose(&entry.text, &entry.line_index);
        entry.ast = ast;
        entry.schema = schema;
        entry.diagnostics.clone_from(&diagnostics);
        Some(diagnostics)
    }

    /// Run a read-only closure against a doc.
    pub fn with<R>(&self, uri: &Url, f: impl FnOnce(&DocState) -> R) -> Option<R> {
        let entry = self.docs.get(uri)?;
        Some(f(entry.value()))
    }
}

fn parse_and_diagnose(
    text: &str,
    line_index: &LineIndex,
) -> (Option<SchemaFile>, Option<Schema>, Vec<Diagnostic>) {
    use tower_lsp::lsp_types::DiagnosticSeverity;

    match ferriorm_parser::parse(text) {
        Ok(ast) => match ferriorm_parser::validate(&ast) {
            Ok(schema) => (Some(ast), Some(schema), Vec::new()),
            Err(err) => {
                let span = err.span();
                let diag = Diagnostic {
                    range: span
                        .map(|s| line_index.range_of(text, s))
                        .unwrap_or_default(),
                    severity: Some(DiagnosticSeverity::ERROR),
                    source: Some("ferriorm".into()),
                    message: err.to_string(),
                    ..Default::default()
                };
                (Some(ast), None, vec![diag])
            }
        },
        Err(ParseError::Syntax { message, span }) => {
            let diag = Diagnostic {
                range: span
                    .map(|s| line_index.range_of(text, s))
                    .unwrap_or_default(),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ferriorm".into()),
                message,
                ..Default::default()
            };
            (None, None, vec![diag])
        }
        Err(ParseError::Validation(err)) => {
            let span = err.span();
            let diag = Diagnostic {
                range: span
                    .map(|s| line_index.range_of(text, s))
                    .unwrap_or_default(),
                severity: Some(DiagnosticSeverity::ERROR),
                source: Some("ferriorm".into()),
                message: err.to_string(),
                ..Default::default()
            };
            (None, None, vec![diag])
        }
    }
}
