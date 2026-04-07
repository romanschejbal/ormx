use ferriorm_core::types::DatabaseProvider;
use ferriorm_migrate::MigrationStrategy;
use std::path::Path;

pub async fn dev(schema_path: &str, name: Option<&str>, use_snapshot: bool) -> miette::Result<()> {
    let source = std::fs::read_to_string(schema_path)
        .map_err(|e| miette::miette!("Failed to read {schema_path}: {e}"))?;

    let schema = ferriorm_parser::parse_and_validate(&source)
        .map_err(|e| miette::miette!("Schema error: {e}"))?;

    let strategy = if use_snapshot {
        println!("Using snapshot strategy (offline mode)");
        MigrationStrategy::Snapshot
    } else {
        println!("Using shadow database strategy");
        MigrationStrategy::ShadowDatabase
    };

    let url = resolve_url(&schema.datasource.url)?;
    let migrations_dir = Path::new("migrations").to_path_buf();
    let runner = ferriorm_migrate::MigrationRunner::new(
        migrations_dir,
        schema.datasource.provider,
        strategy,
    );

    let migration_name = name.unwrap_or("migration");

    println!("Diffing schema...");
    match runner
        .create_migration(&schema, migration_name, Some(&url))
        .await
    {
        Ok(Some(dir)) => {
            println!(
                "Created migration: {}",
                dir.file_name().unwrap().to_string_lossy()
            );

            // Apply the migration
            apply_pending(&runner, &url, schema.datasource.provider).await?;

            println!("Regenerating client...");
            super::generate::run(schema_path).await?;
        }
        Ok(None) => {
            println!("No schema changes detected.");
        }
        Err(e) => {
            return Err(miette::miette!("Failed to create migration: {e}"));
        }
    }

    Ok(())
}

pub async fn deploy(schema_path: &str) -> miette::Result<()> {
    let source = std::fs::read_to_string(schema_path)
        .map_err(|e| miette::miette!("Failed to read {schema_path}: {e}"))?;

    let schema = ferriorm_parser::parse_and_validate(&source)
        .map_err(|e| miette::miette!("Schema error: {e}"))?;

    let url = resolve_url(&schema.datasource.url)?;
    let migrations_dir = Path::new("migrations").to_path_buf();
    let runner = ferriorm_migrate::MigrationRunner::new(
        migrations_dir,
        schema.datasource.provider,
        MigrationStrategy::Snapshot,
    );

    apply_pending(&runner, &url, schema.datasource.provider).await?;

    Ok(())
}

pub async fn status(schema_path: &str) -> miette::Result<()> {
    let source = std::fs::read_to_string(schema_path)
        .map_err(|e| miette::miette!("Failed to read {schema_path}: {e}"))?;

    let schema = ferriorm_parser::parse_and_validate(&source)
        .map_err(|e| miette::miette!("Schema error: {e}"))?;

    let url = resolve_url(&schema.datasource.url)?;
    let migrations_dir = Path::new("migrations").to_path_buf();
    let runner = ferriorm_migrate::MigrationRunner::new(
        migrations_dir,
        schema.datasource.provider,
        MigrationStrategy::Snapshot,
    );

    let statuses = match schema.datasource.provider {
        DatabaseProvider::PostgreSQL => {
            let pool = sqlx::PgPool::connect(&url)
                .await
                .map_err(|e| miette::miette!("Failed to connect: {e}"))?;
            let s = runner
                .status(&pool)
                .await
                .map_err(|e| miette::miette!("Failed to get status: {e}"))?;
            pool.close().await;
            s
        }
        DatabaseProvider::SQLite => {
            let pool = sqlx::SqlitePool::connect(&url)
                .await
                .map_err(|e| miette::miette!("Failed to connect: {e}"))?;
            let s = runner
                .status_sqlite(&pool)
                .await
                .map_err(|e| miette::miette!("Failed to get status: {e}"))?;
            pool.close().await;
            s
        }
        _ => return Err(miette::miette!("Unsupported database provider")),
    };

    if statuses.is_empty() {
        println!("No migrations found.");
    } else {
        for s in &statuses {
            let status = if s.applied {
                format!("applied at {}", s.applied_at.unwrap())
            } else {
                "pending".to_string()
            };
            println!("  {} - {}", s.name, status);
        }
    }

    Ok(())
}

async fn apply_pending(
    runner: &ferriorm_migrate::MigrationRunner,
    url: &str,
    provider: DatabaseProvider,
) -> miette::Result<()> {
    let applied = match provider {
        DatabaseProvider::PostgreSQL => {
            let pool = sqlx::PgPool::connect(url)
                .await
                .map_err(|e| miette::miette!("Failed to connect: {e}"))?;
            let a = runner
                .apply_pending(&pool)
                .await
                .map_err(|e| miette::miette!("Failed to apply migration: {e}"))?;
            pool.close().await;
            a
        }
        DatabaseProvider::SQLite => {
            let pool = sqlx::SqlitePool::connect(url)
                .await
                .map_err(|e| miette::miette!("Failed to connect: {e}"))?;
            let a = runner
                .apply_pending_sqlite(&pool)
                .await
                .map_err(|e| miette::miette!("Failed to apply migration: {e}"))?;
            pool.close().await;
            a
        }
        _ => return Err(miette::miette!("Unsupported database provider")),
    };

    if applied.is_empty() {
        println!("All migrations already applied.");
    } else {
        for name in &applied {
            println!("Applied: {name}");
        }
        println!("Applied {} migration(s).", applied.len());
    }

    Ok(())
}

pub fn resolve_url(url: &str) -> miette::Result<String> {
    let resolved = if url.starts_with("${env:") && url.ends_with('}') {
        let var_name = &url[6..url.len() - 1];
        std::env::var(var_name)
            .map_err(|_| miette::miette!("Environment variable '{var_name}' not set"))?
    } else {
        url.to_string()
    };
    Ok(normalize_sqlite_url_for_cli(&resolved))
}

/// Transform `file:` URLs into `sqlite:` URLs with `mode=rwc` so sqlx can
/// connect and auto-create the database file.
fn normalize_sqlite_url_for_cli(url: &str) -> String {
    if let Some(path) = url.strip_prefix("file:") {
        let sqlite_url = format!("sqlite:{}", path);
        if sqlite_url.contains("mode=") {
            sqlite_url
        } else if sqlite_url.contains('?') {
            format!("{}&mode=rwc", sqlite_url)
        } else {
            format!("{}?mode=rwc", sqlite_url)
        }
    } else {
        url.to_string()
    }
}
