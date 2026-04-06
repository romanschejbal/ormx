//! Core domain types for the ormx ecosystem.
//!
//! This crate is the foundation of ormx. It defines the Abstract Syntax Tree
//! ([`ast`]), the validated Schema Intermediate Representation ([`schema`]),
//! scalar and provider types ([`types`]), and domain-level errors ([`error`]).
//!
//! `ormx-core` has **zero external dependencies** (aside from optional `serde`
//! support behind a feature flag). Every other ormx crate depends on it, but it
//! depends on nothing outside `std`.
//!
//! # Crate relationships
//!
//! ```text
//! ormx-parser ──┐
//! ormx-codegen ─┤
//! ormx-runtime ─┼── all depend on ──► ormx-core
//! ormx-migrate ─┘
//! ```

pub mod ast;
pub mod error;
pub mod schema;
pub mod types;
pub mod utils;
