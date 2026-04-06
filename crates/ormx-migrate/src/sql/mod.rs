pub mod postgres;
pub mod sqlite;

use crate::diff::MigrationStep;

/// Trait for rendering migration steps into SQL.
pub trait SqlRenderer {
    fn render(&self, steps: &[MigrationStep]) -> String;
}

/// Get the SQL renderer for the given provider.
pub fn renderer_for(provider: ormx_core::types::DatabaseProvider) -> Box<dyn SqlRenderer> {
    match provider {
        ormx_core::types::DatabaseProvider::PostgreSQL => Box::new(postgres::PostgresRenderer),
        ormx_core::types::DatabaseProvider::SQLite => Box::new(sqlite::SqliteRenderer),
        _ => Box::new(postgres::PostgresRenderer), // TODO: MySQL renderer
    }
}
