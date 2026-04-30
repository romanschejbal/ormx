//! Domain-level errors for schema validation.
//!
//! [`CoreError`] covers issues that can be detected purely from the schema
//! structure: missing primary keys, unknown types, invalid defaults, duplicate
//! names, and malformed relation attributes. These errors are raised during the
//! validation step (AST to Schema IR) and are independent of any database
//! connection.

use core::fmt;

use crate::ast::Span;

/// Core errors that can occur in ferriorm domain logic.
///
/// Each variant carries an optional source [`Span`] that points at the offending
/// AST node. The span is `None` only for errors that don't have a single
/// localizable origin (e.g. a missing top-level datasource block).
#[derive(Debug)]
pub enum CoreError {
    /// A model is missing a primary key (`@id` or `@@id`).
    MissingPrimaryKey {
        model_name: String,
        span: Option<Span>,
    },

    /// A field references an unknown type.
    UnknownType {
        model_name: String,
        field_name: String,
        type_name: String,
        span: Option<Span>,
    },

    /// A default value doesn't match the field type.
    InvalidDefault {
        model_name: String,
        field_name: String,
        message: String,
        span: Option<Span>,
    },

    /// Duplicate model or enum name.
    DuplicateName {
        name: String,
        kind: &'static str,
        span: Option<Span>,
    },

    /// A `@relation` attribute references unknown fields.
    InvalidRelationFields {
        model_name: String,
        field_name: String,
        message: String,
        span: Option<Span>,
    },

    /// Unknown database provider.
    UnknownProvider {
        provider: String,
        span: Option<Span>,
    },

    /// Generic validation error.
    Validation { message: String, span: Option<Span> },
}

impl CoreError {
    /// Source span of the offending node, when available.
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        match self {
            Self::MissingPrimaryKey { span, .. }
            | Self::UnknownType { span, .. }
            | Self::InvalidDefault { span, .. }
            | Self::DuplicateName { span, .. }
            | Self::InvalidRelationFields { span, .. }
            | Self::UnknownProvider { span, .. }
            | Self::Validation { span, .. } => *span,
        }
    }
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPrimaryKey { model_name, .. } => {
                write!(
                    f,
                    "Model `{model_name}` is missing a primary key (@id or @@id)"
                )
            }
            Self::UnknownType {
                model_name,
                field_name,
                type_name,
                ..
            } => {
                write!(
                    f,
                    "Unknown type `{type_name}` in field `{field_name}` of model `{model_name}`"
                )
            }
            Self::InvalidDefault {
                model_name,
                field_name,
                message,
                ..
            } => {
                write!(
                    f,
                    "Invalid default for `{model_name}.{field_name}`: {message}"
                )
            }
            Self::DuplicateName { name, kind, .. } => {
                write!(f, "Duplicate {kind} name: `{name}`")
            }
            Self::InvalidRelationFields {
                model_name,
                field_name,
                message,
                ..
            } => {
                write!(
                    f,
                    "Invalid @relation on `{model_name}.{field_name}`: {message}"
                )
            }
            Self::UnknownProvider { provider, .. } => {
                write!(f, "Unknown database provider: `{provider}`")
            }
            Self::Validation { message, .. } => {
                write!(f, "Validation error: {message}")
            }
        }
    }
}

impl std::error::Error for CoreError {}
