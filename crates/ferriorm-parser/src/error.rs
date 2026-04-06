//! Error types for the schema parser and validator.
//!
//! [`ParseError`] covers both syntactic failures (grammar mismatches) and
//! semantic validation errors (unknown types, missing primary keys, etc.).
//! It implements `miette::Diagnostic` for rich, user-friendly error reporting.

use miette::Diagnostic;
use thiserror::Error;

#[derive(Debug, Error, Diagnostic)]
pub enum ParseError {
    #[error("Syntax error: {0}")]
    #[diagnostic(code(ferriorm::parser::syntax))]
    Syntax(String),

    #[error("Validation error: {0}")]
    #[diagnostic(code(ferriorm::parser::validation))]
    Validation(String),
}
