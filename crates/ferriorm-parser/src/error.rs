//! Error types for the schema parser and validator.
//!
//! [`ParseError`] covers both syntactic failures (grammar mismatches) and
//! semantic validation errors (unknown types, missing primary keys, etc.).
//! It implements `miette::Diagnostic` for rich, user-friendly error reporting.

use ferriorm_core::ast::Span;
use ferriorm_core::error::CoreError;
use miette::Diagnostic;
use thiserror::Error;

/// All errors produced by [`crate::parse`] and [`crate::parse_and_validate`].
///
/// `Syntax` is raised when the source does not conform to the PEG grammar.
/// `Validation` wraps a structured [`CoreError`] from the validator. Both
/// variants carry an optional source [`Span`] for IDE/LSP integration.
#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("Syntax error: {message}")]
    #[diagnostic(code(ferriorm::parser::syntax))]
    Syntax { message: String, span: Option<Span> },

    #[error(transparent)]
    #[diagnostic(code(ferriorm::parser::validation))]
    Validation(#[from] CoreError),
}

impl ParseError {
    /// Construct a syntax error without a span.
    #[must_use]
    pub fn syntax(message: impl Into<String>) -> Self {
        Self::Syntax {
            message: message.into(),
            span: None,
        }
    }

    /// Construct a syntax error with an explicit span.
    #[must_use]
    pub fn syntax_at(message: impl Into<String>, span: Span) -> Self {
        Self::Syntax {
            message: message.into(),
            span: Some(span),
        }
    }

    /// Source span of the offending node, when available.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::Syntax { span, .. } => *span,
            Self::Validation(err) => err.span(),
        }
    }
}
