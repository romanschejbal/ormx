//! Unified error type for ferriorm runtime operations.
//!
//! [`FerriormError`] wraps sqlx database errors and adds ferriorm-specific variants
//! for not-found conditions, query-building failures, and connection issues.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum FerriormError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Record not found")]
    NotFound,

    #[error("Query error: {0}")]
    Query(String),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("{0}")]
    Other(String),
}

impl From<String> for FerriormError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}
