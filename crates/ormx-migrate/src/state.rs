//! Tracks migration state in the database via the `_ormx_migrations` table.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppliedMigration {
    pub id: i32,
    pub name: String,
    pub checksum: String,
    pub applied_at: DateTime<Utc>,
}

const MIGRATIONS_TABLE: &str = "_ormx_migrations";

/// Ensure the migrations tracking table exists.
pub async fn ensure_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS "{MIGRATIONS_TABLE}" (
            "id" SERIAL PRIMARY KEY,
            "name" TEXT NOT NULL UNIQUE,
            "checksum" TEXT NOT NULL,
            "applied_at" TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#
    ))
    .execute(pool)
    .await?;
    Ok(())
}

/// Get all applied migrations, ordered by id.
pub async fn get_applied(pool: &PgPool) -> Result<Vec<AppliedMigration>, sqlx::Error> {
    sqlx::query_as::<_, AppliedMigration>(&format!(
        r#"SELECT "id", "name", "checksum", "applied_at" FROM "{MIGRATIONS_TABLE}" ORDER BY "id""#
    ))
    .fetch_all(pool)
    .await
}

/// Record a migration as applied.
pub async fn mark_applied(pool: &PgPool, name: &str, checksum: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        r#"INSERT INTO "{MIGRATIONS_TABLE}" ("name", "checksum") VALUES ($1, $2)"#
    ))
    .bind(name)
    .bind(checksum)
    .execute(pool)
    .await?;
    Ok(())
}

/// Remove a migration record (for reset).
pub async fn clear_all(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(r#"DELETE FROM "{MIGRATIONS_TABLE}""#))
        .execute(pool)
        .await?;
    Ok(())
}
