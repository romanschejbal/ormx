#![warn(clippy::pedantic)]

//! Schema formatter for `.ferriorm` files.
//!
//! Round-trips a parsed schema back to a canonical, idempotent string form.
//! Comments (both leading `// ...` lines and same-line trailing `// ...`)
//! captured by the parser are preserved.
//!
//! ## Algorithm
//!
//! The formatter is a single-pass AST visitor with no width-based wrapping,
//! which guarantees `format(format(s)) == format(s)`. Within a model body,
//! field columns are aligned to the longest name and longest type so that
//! attributes line up vertically. Block-level attributes (`@@index`, `@@map`,
//! ...) are emitted after a blank line and are not column-aligned.
//!
//! The public entry points are [`format_schema`] (parse + format) and
//! [`format_ast`] (skip parsing when an AST is already in hand, used by the
//! LSP).

mod visitor;
mod writer;

use ferriorm_parser::error::ParseError;

pub use visitor::format_schema_file as format_ast;

/// Format a `.ferriorm` source string.
///
/// # Errors
///
/// Returns the underlying [`ParseError`] if `source` does not parse.
pub fn format_schema(source: &str) -> Result<String, ParseError> {
    let ast = ferriorm_parser::parse(source)?;
    Ok(format_ast(&ast))
}
