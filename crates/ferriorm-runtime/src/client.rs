//! Database connection pool wrapper.
//!
//! [`DatabaseClient`] is an enum that wraps either a PostgreSQL or SQLite
//! connection pool (via sqlx). It provides auto-detection from the connection
//! URL and exposes typed `fetch_all`, `fetch_optional`, `fetch_one`, and
//! `execute` helpers used by the generated query builders.

use crate::error::FerriormError;

/// The database client, wrapping an sqlx connection pool.
///
/// Supports PostgreSQL and SQLite via feature flags.
#[derive(Debug, Clone)]
pub enum DatabaseClient {
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
}

impl DatabaseClient {
    /// Connect to a PostgreSQL database.
    #[cfg(feature = "postgres")]
    pub async fn connect_postgres(url: &str) -> Result<Self, FerriormError> {
        let pool = sqlx::PgPool::connect(url).await?;
        Ok(Self::Postgres(pool))
    }

    /// Connect to a SQLite database.
    #[cfg(feature = "sqlite")]
    pub async fn connect_sqlite(url: &str) -> Result<Self, FerriormError> {
        let pool = sqlx::SqlitePool::connect(url).await?;
        Ok(Self::Sqlite(pool))
    }

    /// Connect by auto-detecting the database type from the URL.
    pub async fn connect(url: &str) -> Result<Self, FerriormError> {
        #[cfg(feature = "sqlite")]
        if url.starts_with("sqlite:") || url.starts_with("file:") || url.ends_with(".db") {
            return Self::connect_sqlite(url).await;
        }

        #[cfg(feature = "postgres")]
        {
            return Self::connect_postgres(url).await;
        }

        #[allow(unreachable_code)]
        Err(FerriormError::Connection(
            "No database backend enabled. Enable 'postgres' or 'sqlite' feature.".into(),
        ))
    }

    /// Close the connection pool.
    pub async fn disconnect(self) {
        match self {
            #[cfg(feature = "postgres")]
            Self::Postgres(pool) => pool.close().await,
            #[cfg(feature = "sqlite")]
            Self::Sqlite(pool) => pool.close().await,
        }
    }

    /// Execute a query builder against the appropriate pool, returning all rows.
    #[cfg(feature = "postgres")]
    pub async fn fetch_all_pg<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Postgres>,
    ) -> Result<Vec<T>, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        match self {
            Self::Postgres(pool) => Ok(qb.build_query_as::<T>().fetch_all(pool).await?),
            #[cfg(feature = "sqlite")]
            _ => Err(FerriormError::Query(
                "Expected PostgreSQL connection".into(),
            )),
        }
    }

    #[cfg(feature = "postgres")]
    pub async fn fetch_optional_pg<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Postgres>,
    ) -> Result<Option<T>, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        match self {
            Self::Postgres(pool) => Ok(qb.build_query_as::<T>().fetch_optional(pool).await?),
            #[cfg(feature = "sqlite")]
            _ => Err(FerriormError::Query(
                "Expected PostgreSQL connection".into(),
            )),
        }
    }

    #[cfg(feature = "postgres")]
    pub async fn fetch_one_pg<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Postgres>,
    ) -> Result<T, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Send + Unpin,
    {
        match self {
            Self::Postgres(pool) => Ok(qb.build_query_as::<T>().fetch_one(pool).await?),
            #[cfg(feature = "sqlite")]
            _ => Err(FerriormError::Query(
                "Expected PostgreSQL connection".into(),
            )),
        }
    }

    #[cfg(feature = "postgres")]
    pub async fn execute_pg<'q>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Postgres>,
    ) -> Result<u64, FerriormError> {
        match self {
            Self::Postgres(pool) => Ok(qb.build().execute(pool).await?.rows_affected()),
            #[cfg(feature = "sqlite")]
            _ => Err(FerriormError::Query(
                "Expected PostgreSQL connection".into(),
            )),
        }
    }

    // SQLite variants
    #[cfg(feature = "sqlite")]
    pub async fn fetch_all_sqlite<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Sqlite>,
    ) -> Result<Vec<T>, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        match self {
            Self::Sqlite(pool) => Ok(qb.build_query_as::<T>().fetch_all(pool).await?),
            #[cfg(feature = "postgres")]
            _ => Err(FerriormError::Query("Expected SQLite connection".into())),
        }
    }

    #[cfg(feature = "sqlite")]
    pub async fn fetch_optional_sqlite<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Sqlite>,
    ) -> Result<Option<T>, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        match self {
            Self::Sqlite(pool) => Ok(qb.build_query_as::<T>().fetch_optional(pool).await?),
            #[cfg(feature = "postgres")]
            _ => Err(FerriormError::Query("Expected SQLite connection".into())),
        }
    }

    #[cfg(feature = "sqlite")]
    pub async fn fetch_one_sqlite<'q, T>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Sqlite>,
    ) -> Result<T, FerriormError>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::sqlite::SqliteRow> + Send + Unpin,
    {
        match self {
            Self::Sqlite(pool) => Ok(qb.build_query_as::<T>().fetch_one(pool).await?),
            #[cfg(feature = "postgres")]
            _ => Err(FerriormError::Query("Expected SQLite connection".into())),
        }
    }

    #[cfg(feature = "sqlite")]
    pub async fn execute_sqlite<'q>(
        &self,
        mut qb: sqlx::QueryBuilder<'q, sqlx::Sqlite>,
    ) -> Result<u64, FerriormError> {
        match self {
            Self::Sqlite(pool) => Ok(qb.build().execute(pool).await?.rows_affected()),
            #[cfg(feature = "postgres")]
            _ => Err(FerriormError::Query("Expected SQLite connection".into())),
        }
    }
}
