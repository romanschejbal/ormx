//! Schema parser for `.ormx` files.
//!
//! This crate turns a `.ormx` schema string into a fully validated
//! [`ormx_core::schema::Schema`] IR. It operates in two phases:
//!
//! 1. **Parsing** ([`parser`]) -- a PEG grammar (defined in `grammar.pest`)
//!    tokenizes the input and builds a raw [`ormx_core::ast::SchemaFile`].
//! 2. **Validation** ([`validator`]) -- resolves types, checks constraints,
//!    and produces the canonical [`ormx_core::schema::Schema`] consumed by
//!    codegen and the migration engine.
//!
//! For convenience, [`parse_and_validate`] combines both steps.
//!
//! # Related crates
//!
//! - `ormx_core` -- domain types produced by this crate.
//! - `ormx_codegen` -- consumes the `Schema` IR to generate Rust code.
//! - `ormx_migrate` -- consumes the `Schema` IR to produce migrations.

pub mod error;
pub mod parser;
pub mod validator;

pub use parser::parse;
pub use validator::validate;

/// Parse and validate a schema file in one step.
pub fn parse_and_validate(source: &str) -> Result<ormx_core::schema::Schema, error::ParseError> {
    let ast = parse(source)?;
    validate(&ast).map_err(|e| error::ParseError::Validation(e.to_string()))
}
