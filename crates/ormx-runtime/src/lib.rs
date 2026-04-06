//! Runtime library shipped with the user's application.
//!
//! This crate provides everything the generated ormx client needs at runtime:
//!
//! - [`client::DatabaseClient`] -- connection pool wrapper (PostgreSQL + SQLite)
//! - [`filter`] -- type-safe filter structs (`StringFilter`, `IntFilter`, ...)
//! - [`query::SqlBuilder`] -- parameterized SQL builder supporting `$1` and `?` styles
//! - [`order`] -- `SortOrder` and `OrderByClause` trait
//! - [`transaction`] -- transaction execution helpers
//! - [`error::OrmxError`] -- unified error type
//!
//! A [`prelude`] module re-exports the most commonly used items.
//!
//! # Related crates
//!
//! - [`ormx_core`] -- domain types (this crate does not depend on `ormx-core`
//!   at runtime; the generated code bridges the two).
//! - [`ormx_codegen`] -- generates code that depends on this crate.

pub mod client;
pub mod error;
pub mod filter;
pub mod order;
pub mod query;
pub mod transaction;

pub mod prelude {
    pub use crate::SetValue;
    pub use crate::client::DatabaseClient;
    pub use crate::error::OrmxError;
    pub use crate::filter::*;
    pub use crate::order::SortOrder;
    pub use crate::query::SqlBuilder;
    pub use chrono;
    pub use sqlx;
    pub use uuid;
}

/// Wrapper for update operations: distinguishes "not set" from "set to value".
#[derive(Debug, Clone)]
pub enum SetValue<T> {
    Set(T),
}
