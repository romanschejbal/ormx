//! Code generator that produces a type-safe Rust client from a validated schema.
//!
//! Given an [`ormx_core::schema::Schema`], this crate emits a complete Rust
//! module tree consisting of:
//!
//! - **Model structs** with `sqlx::FromRow` derives ([`model`])
//! - **Enum definitions** with `sqlx::Type` derives ([`enums`])
//! - **Filter / order / data submodules** for type-safe queries ([`model`])
//! - **Relation types** and batched include loading ([`relations`])
//! - **`OrmxClient`** -- the entry-point struct users interact with ([`client`])
//!
//! The main entry point is [`generator::generate`], which writes all files to
//! the configured output directory.
//!
//! # Related crates
//!
//! - [`ormx_core`] -- the `Schema` IR consumed by this crate.
//! - [`ormx_parser`] -- parses `.ormx` files into the `Schema` IR.
//! - [`ormx_runtime`] -- the runtime library that generated code depends on.

pub mod client;
pub mod enums;
pub mod formatter;
pub mod generator;
pub mod model;
pub mod relations;

mod rust_type;
