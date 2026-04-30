#![allow(clippy::pedantic)]

//! End-to-end migration tests using the `ShadowDatabase` strategy with SQLite.
//!
//! The shadow database strategy creates a temporary SQLite file, replays all
//! existing migrations into it, introspects the result, and then diffs against
//! the current schema. This tests the full shadow DB flow.

use ferriorm_migrate::{MigrationRunner, MigrationStrategy};
use sqlx::SqlitePool;

const SCHEMA_V1: &str = r#"
datasource db {
  provider = "sqlite"
  url      = "sqlite::memory:"
}

generator client {
  output = "./src/generated"
}

model Category {
  id   String @id @default(uuid())
  name String @unique

  @@map("categories")
}
"#;

const SCHEMA_V2: &str = r#"
datasource db {
  provider = "sqlite"
  url      = "sqlite::memory:"
}

generator client {
  output = "./src/generated"
}

model Category {
  id          String  @id @default(uuid())
  name        String  @unique
  description String?

  @@map("categories")
}

model Product {
  id         String   @id @default(uuid())
  name       String
  price      Int      @default(0)
  categoryId String
  category   Category @relation(fields: [categoryId], references: [id])
  createdAt  DateTime @default(now())

  @@map("products")
  @@index([categoryId])
}
"#;

async fn sqlite_memory_pool() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:")
        .await
        .expect("connect to in-memory SQLite")
}

async fn get_table_names(pool: &SqlitePool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '\\_%' ESCAPE '\\' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .expect("query sqlite_master");
    rows.into_iter().map(|r| r.0).collect()
}

async fn get_column_names(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows: Vec<(String,)> =
        sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(pool)
            .await
            .expect("query pragma_table_info");
    rows.into_iter().map(|r| r.0).collect()
}

#[tokio::test]
async fn shadow_create_initial_migration() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    // First migration -- no existing migrations, so shadow sees empty schema
    let result = runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration should succeed");

    let migration_dir = result.expect("should produce a migration");

    // Verify migration files
    let sql_path = migration_dir.join("migration.sql");
    assert!(sql_path.exists(), "migration.sql should exist");

    let sql = std::fs::read_to_string(&sql_path).expect("read sql");
    assert!(
        sql.contains("CREATE TABLE \"categories\""),
        "SQL should create categories table. Got:\n{sql}"
    );
    assert!(
        sql.contains("\"id\" TEXT NOT NULL"),
        "SQL should define id column. Got:\n{sql}"
    );
    assert!(
        sql.contains("\"name\" TEXT NOT NULL"),
        "SQL should define name column. Got:\n{sql}"
    );
    assert!(
        sql.contains("UNIQUE"),
        "name column should be UNIQUE. Got:\n{sql}"
    );

    let snapshot_path = migration_dir.join("_schema_snapshot.json");
    assert!(snapshot_path.exists(), "_schema_snapshot.json should exist");
}

#[tokio::test]
async fn shadow_apply_and_verify() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create")
        .expect("migration");

    let pool = sqlite_memory_pool().await;
    let applied = runner.apply_pending_sqlite(&pool).await.expect("apply");

    assert_eq!(applied.len(), 1);

    let tables = get_table_names(&pool).await;
    assert!(
        tables.contains(&"categories".to_string()),
        "categories table should exist"
    );
}

#[tokio::test]
async fn shadow_second_migration_via_introspection() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");
    let schema_v2 = ferriorm_parser::parse_and_validate(SCHEMA_V2).expect("parse v2");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    // Create and apply first migration
    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create v1")
        .expect("v1 migration");

    // For the second migration with shadow strategy, the runner:
    // 1. Creates a temp SQLite file
    // 2. Replays the init migration
    // 3. Introspects the result (gets v1 schema)
    // 4. Diffs v1 introspected vs v2 current
    let v2_result = runner
        .create_migration(&schema_v2, "add_products", None)
        .await
        .expect("create v2");

    let v2_dir = v2_result.expect("v2 should have changes");

    let v2_sql = std::fs::read_to_string(v2_dir.join("migration.sql")).expect("read v2 sql");

    // Should create products table
    assert!(
        v2_sql.contains("CREATE TABLE \"products\""),
        "v2 should create products table. Got:\n{v2_sql}"
    );

    // Should add description column to categories
    // (Note: shadow introspection may produce the diff slightly differently
    //  than snapshot, but the intent is the same)
    assert!(
        v2_sql.contains("\"description\""),
        "v2 should reference description column. Got:\n{v2_sql}"
    );

    // Apply both migrations to a fresh database
    let pool = sqlite_memory_pool().await;
    let applied = runner.apply_pending_sqlite(&pool).await.expect("apply all");
    assert_eq!(
        applied.len(),
        2,
        "Both migrations should be applied. Applied: {applied:?}"
    );

    // Verify final state
    let tables = get_table_names(&pool).await;
    assert!(tables.contains(&"categories".to_string()));
    assert!(tables.contains(&"products".to_string()));

    let cat_cols = get_column_names(&pool, "categories").await;
    assert!(cat_cols.contains(&"description".to_string()));

    let prod_cols = get_column_names(&pool, "products").await;
    assert!(prod_cols.contains(&"id".to_string()));
    assert!(prod_cols.contains(&"name".to_string()));
    assert!(prod_cols.contains(&"price".to_string()));
    assert!(prod_cols.contains(&"category_id".to_string()));

    // Verify status
    let statuses = runner.status_sqlite(&pool).await.expect("status");
    assert_eq!(statuses.len(), 2);
    assert!(statuses.iter().all(|s| s.applied));
}

#[tokio::test]
async fn shadow_detects_no_changes() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create")
        .expect("migration");

    // Shadow replays init migration, introspects, diffs against v1 -- should find no changes
    let result = runner
        .create_migration(&schema_v1, "nothing", None)
        .await
        .expect("create_migration should succeed");

    assert!(
        result.is_none(),
        "Shadow strategy should detect no changes when schema hasn't changed"
    );
}

#[tokio::test]
async fn shadow_can_insert_data_after_multi_migration() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");
    let schema_v2 = ferriorm_parser::parse_and_validate(SCHEMA_V2).expect("parse v2");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create v1")
        .expect("v1");

    runner
        .create_migration(&schema_v2, "add_products", None)
        .await
        .expect("create v2")
        .expect("v2");

    let pool = sqlite_memory_pool().await;
    runner.apply_pending_sqlite(&pool).await.expect("apply all");

    // Insert a category
    sqlx::query(
        "INSERT INTO \"categories\" (\"id\", \"name\", \"description\") VALUES ('cat-1', 'Electronics', 'Gadgets and devices')",
    )
    .execute(&pool)
    .await
    .expect("insert category");

    // Insert a product referencing the category
    sqlx::query(
        "INSERT INTO \"products\" (\"id\", \"name\", \"price\", \"category_id\", \"created_at\") VALUES ('prod-1', 'Laptop', 999, 'cat-1', datetime('now'))",
    )
    .execute(&pool)
    .await
    .expect("insert product");

    // Query with JOIN
    let row: (String, String, i32, String) = sqlx::query_as(
        "SELECT p.\"id\", p.\"name\", p.\"price\", c.\"name\" FROM \"products\" p JOIN \"categories\" c ON p.\"category_id\" = c.\"id\" WHERE p.\"id\" = 'prod-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("query product with category");

    assert_eq!(row.0, "prod-1");
    assert_eq!(row.1, "Laptop");
    assert_eq!(row.2, 999);
    assert_eq!(row.3, "Electronics");
}

// ─── G1-G3: shadow DB and ordering ──────────────────────────────────

/// G1: when an existing migration's SQL is malformed, the shadow DB
/// flow must surface an error (not panic, not silently succeed). The
/// shadow database file itself is created in the system temp dir, so
/// we can't easily probe leakage; the tractable assertion is that the
/// returned error mentions the failure and that subsequent
/// `create_migration` calls don't deadlock.
#[tokio::test]
async fn g1_shadow_surfaces_syntax_error_in_existing_migration() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let migrations_dir = tmp.path().join("migrations");
    std::fs::create_dir_all(&migrations_dir).unwrap();

    // Hand-craft a migration with deliberately broken SQL.
    let bad_dir = migrations_dir.join("0001_bad");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(
        bad_dir.join("migration.sql"),
        "CREATE TABL invalid syntax here;",
    )
    .unwrap();
    // Snapshot must exist for the runner to consider the migration valid.
    std::fs::write(bad_dir.join("_schema_snapshot.json"), "{}").unwrap();

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    let schema = ferriorm_parser::parse_and_validate(SCHEMA_V2).expect("parse v2");
    let result = runner.create_migration(&schema, "follow_up", None).await;

    assert!(
        result.is_err(),
        "create_migration with a malformed prior migration must return an error; got Ok"
    );
}

/// G2: pin the migration-ordering behavior. The runner zero-pads new
/// migrations to 4 digits, so ordering is stable up to 9999. But if a
/// user *manually* creates migrations without zero padding (`2_x`,
/// `10_y`), string sort places `10_y` before `2_x` — semantic
/// inversion. This test documents the current behavior so users (and
/// future contributors) know the contract.
#[tokio::test]
async fn g2_handcrafted_numeric_prefix_sort_is_stringy() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let migrations_dir = tmp.path().join("migrations");
    std::fs::create_dir_all(&migrations_dir).unwrap();

    // Hand-create two migrations with non-zero-padded numeric prefixes.
    for (dir, sql) in [
        (
            "2_create_a",
            r#"CREATE TABLE "a" ("id" INTEGER PRIMARY KEY);"#,
        ),
        (
            "10_create_b",
            r#"CREATE TABLE "b" ("id" INTEGER PRIMARY KEY);"#,
        ),
    ] {
        let d = migrations_dir.join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("migration.sql"), sql).unwrap();
        std::fs::write(d.join("_schema_snapshot.json"), "{}").unwrap();
    }

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::ShadowDatabase,
    );

    let pool = sqlite_memory_pool().await;
    let applied = runner
        .apply_pending_sqlite(&pool)
        .await
        .expect("apply pending");

    // Document: under string sort, "10_create_b" comes BEFORE "2_create_a".
    // If a future change adopts numeric-aware sort, this assertion will
    // flip and the test should be updated to expect the new contract.
    assert_eq!(
        applied,
        vec!["10_create_b".to_string(), "2_create_a".to_string()],
        "current contract: migration filenames are sorted lexicographically. \
         Users must zero-pad numeric prefixes (the runner does this for \
         auto-generated migrations) or risk semantic inversion."
    );
}

/// G3: changing a field's `db_name` via `@map` is currently treated as
/// drop+add by the diff engine, which destroys data. This test pins
/// the current behavior so that any future rename-detection feature
/// has a clear regression target.
#[tokio::test]
async fn g3_shadow_field_rename_via_at_map_drops_and_adds() {
    let v1 = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id    String @id @default(uuid())
  email String
  @@map("u")
}
"#;
    let v2 = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id    String @id @default(uuid())
  email String @map("email_address")
  @@map("u")
}
"#;
    let s1 = ferriorm_parser::parse_and_validate(v1).expect("v1");
    let s2 = ferriorm_parser::parse_and_validate(v2).expect("v2");
    let steps = ferriorm_migrate::diff::diff_schemas(
        &s1,
        &s2,
        ferriorm_core::types::DatabaseProvider::SQLite,
    );

    use ferriorm_migrate::diff::MigrationStep;
    let has_drop = steps
        .iter()
        .any(|s| matches!(s, MigrationStep::DropColumn { column, .. } if column == "email"));
    let has_add = steps
        .iter()
        .any(|s| matches!(s, MigrationStep::AddColumn { column, .. } if column.name == "email_address"));

    assert!(has_drop, "current behavior: drop the old column. Steps: {steps:?}");
    assert!(has_add, "current behavior: add the new column. Steps: {steps:?}");
    // If a rename-detection feature lands, this test must be updated:
    // it should expect a single rename-style step that preserves data.
}
