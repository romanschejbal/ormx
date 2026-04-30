#![allow(clippy::pedantic)]

//! End-to-end migration tests using an in-memory SQLite database.
//!
//! These tests exercise the full migration lifecycle:
//! 1. Parse a schema
//! 2. Create a migration (generates SQL + snapshot)
//! 3. Connect to SQLite in-memory
//! 4. Apply the migration
//! 5. Verify the database state
//! 6. Modify the schema, create another migration, apply, verify

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

enum Status {
  Active
  Inactive
}

model User {
  id        String   @id @default(uuid())
  email     String   @unique
  name      String?
  status    Status   @default(Active)
  age       Int      @default(0)
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@map("users")
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

enum Status {
  Active
  Inactive
  Suspended
}

model User {
  id        String   @id @default(uuid())
  email     String   @unique
  name      String?
  status    Status   @default(Active)
  age       Int      @default(0)
  bio       String?
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@map("users")
}

model Post {
  id        String   @id @default(uuid())
  title     String
  content   String?
  authorId  String
  author    User     @relation(fields: [authorId], references: [id])
  createdAt DateTime @default(now())

  @@map("posts")
}
"#;

/// Helper: connect to an in-memory SQLite database.
async fn sqlite_memory_pool() -> SqlitePool {
    SqlitePool::connect("sqlite::memory:")
        .await
        .expect("connect to in-memory SQLite")
}

/// Helper: query sqlite_master for table names.
async fn get_table_names(pool: &SqlitePool) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '\\_%' ESCAPE '\\' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .expect("query sqlite_master");
    rows.into_iter().map(|r| r.0).collect()
}

/// Helper: query column names for a given table.
async fn get_column_names(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows: Vec<(String,)> =
        sqlx::query_as(&format!("SELECT name FROM pragma_table_info('{table}')"))
            .fetch_all(pool)
            .await
            .expect("query pragma_table_info");
    rows.into_iter().map(|r| r.0).collect()
}

// ─── Snapshot strategy tests ────────────────────────────────────────

#[tokio::test]
async fn snapshot_create_initial_migration() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    // Create the first migration
    let result = runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration should succeed");

    let migration_dir = result.expect("should produce a migration (not None)");

    // Verify the migration directory was created
    assert!(
        migration_dir.exists(),
        "Migration directory should exist at {}",
        migration_dir.display()
    );

    // Verify migration.sql exists and contains CREATE TABLE
    let sql_path = migration_dir.join("migration.sql");
    assert!(sql_path.exists(), "migration.sql should exist");

    let sql_content = std::fs::read_to_string(&sql_path).expect("read migration.sql");
    assert!(
        sql_content.contains("CREATE TABLE \"users\""),
        "SQL should contain CREATE TABLE for users. Got:\n{sql_content}"
    );
    assert!(
        sql_content.contains("\"id\" TEXT NOT NULL"),
        "SQL should contain id column definition. Got:\n{sql_content}"
    );
    assert!(
        sql_content.contains("\"email\" TEXT NOT NULL"),
        "SQL should contain email column. Got:\n{sql_content}"
    );
    assert!(
        sql_content.contains("UNIQUE"),
        "SQL should contain UNIQUE for email. Got:\n{sql_content}"
    );
    assert!(
        sql_content.contains("PRIMARY KEY"),
        "SQL should contain PRIMARY KEY. Got:\n{sql_content}"
    );

    // Verify snapshot file exists
    let snapshot_path = migration_dir.join("_schema_snapshot.json");
    assert!(snapshot_path.exists(), "_schema_snapshot.json should exist");

    let snapshot_json = std::fs::read_to_string(&snapshot_path).expect("read snapshot");
    let snapshot_value: serde_json::Value =
        serde_json::from_str(&snapshot_json).expect("snapshot should be valid JSON");
    assert!(
        snapshot_value.get("models").is_some(),
        "Snapshot should contain models key"
    );
    assert!(
        snapshot_value.get("enums").is_some(),
        "Snapshot should contain enums key"
    );
}

#[tokio::test]
async fn snapshot_apply_migration_creates_table() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    // Create migration
    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration")
        .expect("migration should not be None");

    // Connect to SQLite and apply
    let pool = sqlite_memory_pool().await;
    let applied = runner
        .apply_pending_sqlite(&pool)
        .await
        .expect("apply_pending_sqlite should succeed");

    assert_eq!(applied.len(), 1, "One migration should have been applied");
    assert!(
        applied[0].contains("init"),
        "Applied migration name should contain 'init', got: {}",
        applied[0]
    );

    // Verify the table was created
    let tables = get_table_names(&pool).await;
    assert!(
        tables.contains(&"users".to_string()),
        "users table should exist. Tables found: {tables:?}"
    );

    // Verify columns
    let columns = get_column_names(&pool, "users").await;
    assert!(columns.contains(&"id".to_string()), "should have id column");
    assert!(
        columns.contains(&"email".to_string()),
        "should have email column"
    );
    assert!(
        columns.contains(&"name".to_string()),
        "should have name column"
    );
    assert!(
        columns.contains(&"status".to_string()),
        "should have status column"
    );
    assert!(
        columns.contains(&"age".to_string()),
        "should have age column"
    );
    assert!(
        columns.contains(&"created_at".to_string()),
        "should have created_at column"
    );
    assert!(
        columns.contains(&"updated_at".to_string()),
        "should have updated_at column"
    );
}

#[tokio::test]
async fn snapshot_migration_status_after_apply() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration")
        .expect("should produce migration");

    let pool = sqlite_memory_pool().await;
    runner.apply_pending_sqlite(&pool).await.expect("apply");

    // Check status
    let statuses = runner
        .status_sqlite(&pool)
        .await
        .expect("status_sqlite should succeed");

    assert_eq!(statuses.len(), 1, "Should have one migration in status");
    assert!(statuses[0].applied, "Migration should be marked as applied");
    assert!(
        statuses[0].applied_at.is_some(),
        "Applied migration should have a timestamp"
    );
    assert!(
        statuses[0].name.contains("init"),
        "Migration name should contain 'init'"
    );
}

#[tokio::test]
async fn snapshot_second_migration_adds_column_and_table() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");
    let schema_v2 = ferriorm_parser::parse_and_validate(SCHEMA_V2).expect("parse v2");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    // Create and apply first migration
    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration v1")
        .expect("v1 migration");

    let pool = sqlite_memory_pool().await;
    runner.apply_pending_sqlite(&pool).await.expect("apply v1");

    // Create second migration (schema diff)
    let v2_migration = runner
        .create_migration(&schema_v2, "add_posts_and_bio", None)
        .await
        .expect("create_migration v2");

    let v2_dir = v2_migration.expect("v2 should have changes");

    // Verify the second migration SQL contains the expected changes
    let v2_sql = std::fs::read_to_string(v2_dir.join("migration.sql")).expect("read v2 sql");

    // Should add bio column to users
    assert!(
        v2_sql.contains("ALTER TABLE \"users\" ADD COLUMN"),
        "v2 SQL should contain ALTER TABLE ADD COLUMN. Got:\n{v2_sql}"
    );
    assert!(
        v2_sql.contains("\"bio\""),
        "v2 SQL should add bio column. Got:\n{v2_sql}"
    );

    // Should create posts table
    assert!(
        v2_sql.contains("CREATE TABLE \"posts\""),
        "v2 SQL should create posts table. Got:\n{v2_sql}"
    );

    // Should mention new enum variant (as a comment for SQLite)
    assert!(
        v2_sql.contains("Suspended") || v2_sql.contains("suspended"),
        "v2 SQL should reference new Suspended enum variant. Got:\n{v2_sql}"
    );

    // Apply second migration
    let applied = runner.apply_pending_sqlite(&pool).await.expect("apply v2");

    assert_eq!(applied.len(), 1, "Only one new migration should be applied");

    // Verify database state
    let tables = get_table_names(&pool).await;
    assert!(
        tables.contains(&"users".to_string()),
        "users table should still exist"
    );
    assert!(
        tables.contains(&"posts".to_string()),
        "posts table should now exist. Tables: {tables:?}"
    );

    // Verify bio column was added to users
    let user_columns = get_column_names(&pool, "users").await;
    assert!(
        user_columns.contains(&"bio".to_string()),
        "users should now have bio column. Columns: {user_columns:?}"
    );

    // Verify posts columns
    let post_columns = get_column_names(&pool, "posts").await;
    assert!(
        post_columns.contains(&"id".to_string()),
        "posts should have id"
    );
    assert!(
        post_columns.contains(&"title".to_string()),
        "posts should have title"
    );
    assert!(
        post_columns.contains(&"content".to_string()),
        "posts should have content"
    );
    assert!(
        post_columns.contains(&"author_id".to_string()),
        "posts should have author_id"
    );

    // Verify migration status shows both applied
    let statuses = runner.status_sqlite(&pool).await.expect("status_sqlite");
    assert_eq!(statuses.len(), 2, "Should have two migrations");
    assert!(statuses[0].applied, "First migration should be applied");
    assert!(statuses[1].applied, "Second migration should be applied");
}

#[tokio::test]
async fn snapshot_no_changes_returns_none() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    // Create first migration
    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration")
        .expect("should create initial migration");

    // Try to create another migration with the same schema -- should return None
    let result = runner
        .create_migration(&schema_v1, "no_changes", None)
        .await
        .expect("create_migration should succeed");

    assert!(
        result.is_none(),
        "Creating migration with identical schema should return None (no changes)"
    );
}

#[tokio::test]
async fn snapshot_apply_is_idempotent() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration")
        .expect("migration");

    let pool = sqlite_memory_pool().await;

    // Apply once
    let first = runner
        .apply_pending_sqlite(&pool)
        .await
        .expect("first apply");
    assert_eq!(first.len(), 1);

    // Apply again -- should be a no-op
    let second = runner
        .apply_pending_sqlite(&pool)
        .await
        .expect("second apply");
    assert_eq!(
        second.len(),
        0,
        "Second apply should find no pending migrations"
    );
}

#[tokio::test]
async fn snapshot_can_insert_and_query_data_after_migration() {
    let schema_v1 = ferriorm_parser::parse_and_validate(SCHEMA_V1).expect("parse v1");

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let migrations_dir = tmp_dir.path().join("migrations");

    let runner = MigrationRunner::new(
        migrations_dir.clone(),
        ferriorm_core::types::DatabaseProvider::SQLite,
        MigrationStrategy::Snapshot,
    );

    runner
        .create_migration(&schema_v1, "init", None)
        .await
        .expect("create_migration")
        .expect("migration");

    let pool = sqlite_memory_pool().await;
    runner.apply_pending_sqlite(&pool).await.expect("apply");

    // Insert a row using raw SQL
    sqlx::query(
        "INSERT INTO \"users\" (\"id\", \"email\", \"name\", \"status\", \"age\", \"created_at\", \"updated_at\") VALUES ('user-1', 'alice@example.com', 'Alice', 'active', 30, datetime('now'), datetime('now'))"
    )
    .execute(&pool)
    .await
    .expect("insert user");

    // Query it back
    let row: (String, String, Option<String>, String, i32) = sqlx::query_as(
        "SELECT \"id\", \"email\", \"name\", \"status\", \"age\" FROM \"users\" WHERE \"id\" = 'user-1'"
    )
    .fetch_one(&pool)
    .await
    .expect("query user");

    assert_eq!(row.0, "user-1");
    assert_eq!(row.1, "alice@example.com");
    assert_eq!(row.2, Some("Alice".to_string()));
    assert_eq!(row.3, "active");
    assert_eq!(row.4, 30);

    // Verify UNIQUE constraint works
    let duplicate_result = sqlx::query(
        "INSERT INTO \"users\" (\"id\", \"email\", \"status\", \"age\", \"created_at\", \"updated_at\") VALUES ('user-2', 'alice@example.com', 'active', 25, datetime('now'), datetime('now'))"
    )
    .execute(&pool)
    .await;

    assert!(
        duplicate_result.is_err(),
        "Inserting duplicate email should fail due to UNIQUE constraint"
    );
}

// ─── F1-F5: SQL renderer string-level assertions ────────────────────
//
// These tests render `MigrationStep`s through the SQLite or Postgres
// renderer and assert on the produced SQL strings. They need no live
// database — they pin renderer contracts cheaply across both backends.

#[test]
fn f1_sqlite_inline_fk_with_mixed_cascade_actions() {
    use ferriorm_core::ast::ReferentialAction;
    use ferriorm_core::schema::*;
    use ferriorm_core::types::{DatabaseProvider, ScalarType};
    use ferriorm_core::utils::to_snake_case;
    use ferriorm_migrate::diff;
    use ferriorm_migrate::sql;

    fn make_field(name: &str, scalar: ScalarType, is_id: bool) -> Field {
        Field {
            name: name.into(),
            db_name: to_snake_case(name),
            field_type: FieldKind::Scalar(scalar),
            is_optional: false,
            is_list: false,
            is_id,
            is_unique: false,
            is_updated_at: false,
            default: None,
            relation: None,
            db_type: None,
        }
    }
    fn make_fk_field(
        name: &str,
        target: &str,
        on_delete: ReferentialAction,
    ) -> Field {
        let mut f = make_field(name, ScalarType::String, false);
        f.relation = Some(ResolvedRelation {
            related_model: target.into(),
            relation_type: RelationType::ManyToOne,
            fields: vec![name.into()],
            references: vec!["id".into()],
            on_delete,
            on_update: ReferentialAction::NoAction,
        });
        f
    }
    fn model(name: &str, db_name: &str, fields: Vec<Field>) -> Model {
        let pk: Vec<String> = fields
            .iter()
            .filter(|f| f.is_id)
            .map(|f| f.name.clone())
            .collect();
        Model {
            name: name.into(),
            db_name: db_name.into(),
            fields,
            primary_key: PrimaryKey { fields: pk },
            indexes: vec![],
            unique_constraints: vec![],
        }
    }

    let user = model("User", "users", vec![make_field("id", ScalarType::String, true)]);
    let cat = model("Cat", "cats", vec![make_field("id", ScalarType::String, true)]);
    let post = model(
        "Post",
        "posts",
        vec![
            make_field("id", ScalarType::String, true),
            make_fk_field("authorId", "User", ReferentialAction::Cascade),
            make_fk_field("editorId", "User", ReferentialAction::SetNull),
            make_fk_field("categoryId", "Cat", ReferentialAction::Restrict),
        ],
    );

    let from = Schema {
        datasource: DatasourceConfig {
            name: "db".into(),
            provider: DatabaseProvider::SQLite,
            url: String::new(),
        },
        generators: vec![],
        enums: vec![],
        models: vec![],
    };
    let to = Schema {
        datasource: from.datasource.clone(),
        generators: vec![],
        enums: vec![],
        models: vec![user, cat, post],
    };

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);
    let sql = sql::renderer_for(DatabaseProvider::SQLite).render(&steps);

    assert!(
        sql.contains("ON DELETE CASCADE"),
        "Cascade FK action must render. Got:\n{sql}"
    );
    assert!(
        sql.contains("ON DELETE SET NULL"),
        "SetNull FK action must render. Got:\n{sql}"
    );
    assert!(
        sql.contains("ON DELETE RESTRICT"),
        "Restrict FK action must render. Got:\n{sql}"
    );
}

#[test]
fn f2_postgres_drop_constraint_uses_if_exists() {
    use ferriorm_core::types::DatabaseProvider;
    use ferriorm_migrate::diff::MigrationStep;
    use ferriorm_migrate::sql;

    let steps = vec![
        MigrationStep::DropForeignKey {
            table: "posts".into(),
            name: "fk_posts_users_author_id".into(),
        },
        MigrationStep::DropUniqueConstraint {
            table: "users".into(),
            name: "uq_users_email".into(),
        },
    ];
    let sql = sql::renderer_for(DatabaseProvider::PostgreSQL).render(&steps);
    assert!(
        sql.contains("DROP CONSTRAINT IF EXISTS \"fk_posts_users_author_id\""),
        "DROP FK must be IF EXISTS. Got:\n{sql}"
    );
    assert!(
        sql.contains("DROP CONSTRAINT IF EXISTS \"uq_users_email\""),
        "DROP UNIQUE must be IF EXISTS. Got:\n{sql}"
    );
}

#[test]
fn f3_postgres_alter_type_add_value_for_enum_extension() {
    use ferriorm_core::types::DatabaseProvider;
    use ferriorm_migrate::diff::MigrationStep;
    use ferriorm_migrate::sql;

    let step = MigrationStep::AddEnumVariant {
        enum_name: "status".into(),
        variant: "Suspended".into(),
    };
    let sql = sql::renderer_for(DatabaseProvider::PostgreSQL).render(&[step]);
    assert!(
        sql.contains("ALTER TYPE \"status\" ADD VALUE"),
        "must use ALTER TYPE ADD VALUE for enum extension. Got:\n{sql}"
    );
    assert!(
        sql.contains("'suspended'"),
        "variant must be lowercased and quoted. Got:\n{sql}"
    );
}

/// F4: changing `Int` -> `BigInt` on Postgres requires a `USING` cast in
/// `ALTER COLUMN TYPE`, otherwise PG rejects the statement when data is
/// present. **Expected to fail today** — the renderer emits no `USING`
/// clause (`crates/ferriorm-migrate/src/sql/postgres.rs:159-164`).
#[test]
fn f4_postgres_alter_column_int_to_bigint_uses_cast() {
    use ferriorm_core::types::DatabaseProvider;
    use ferriorm_migrate::diff::{ColumnChanges, MigrationStep};
    use ferriorm_migrate::sql;

    let step = MigrationStep::AlterColumn {
        table: "stats".into(),
        column: "view_count".into(),
        changes: ColumnChanges {
            sql_type: Some("BIGINT".into()),
            nullable: None,
            default: None,
        },
    };
    let sql = sql::renderer_for(DatabaseProvider::PostgreSQL).render(&[step]);
    assert!(
        sql.to_uppercase().contains("USING"),
        "ALTER COLUMN TYPE must include `USING <col>::<type>` for safety on PG. \
         Today the renderer omits it; PG rejects the statement on populated tables. \
         Got:\n{sql}"
    );
}

/// F5: renaming an enum via `@@map("new_name")` should produce
/// `ALTER TYPE ... RENAME TO ...`, not DROP + CREATE (which destroys
/// existing column values). Today there is no enum-rename detection in
/// `diff_enums` — a rename is treated as drop+add. **Expected to fail
/// today.**
#[test]
fn f5_enum_rename_via_at_map_postgres_uses_alter_rename() {
    let v1 = r#"
datasource db { provider = "postgresql" url = "postgresql://x" }
enum Status {
  Active
  Inactive
  @@map("status_old")
}
model A {
  id     String @id
  status Status
  @@map("a")
}
"#;
    let v2 = r#"
datasource db { provider = "postgresql" url = "postgresql://x" }
enum Status {
  Active
  Inactive
  @@map("status_new")
}
model A {
  id     String @id
  status Status
  @@map("a")
}
"#;

    // Most of the parser's enum-related grammar may not even support `@@map`
    // on enums today. If parsing v1 fails, document the limitation by
    // failing the test with a clear message — that's the bug.
    let s1 = match ferriorm_parser::parse_and_validate(v1) {
        Ok(s) => s,
        Err(e) => {
            panic!(
                "@@map on enum is not yet supported; renaming an enum requires this. Got: {e}"
            );
        }
    };
    let s2 = ferriorm_parser::parse_and_validate(v2).expect("parse v2");

    let steps = ferriorm_migrate::diff::diff_schemas(
        &s1,
        &s2,
        ferriorm_core::types::DatabaseProvider::PostgreSQL,
    );
    let sql = ferriorm_migrate::sql::renderer_for(
        ferriorm_core::types::DatabaseProvider::PostgreSQL,
    )
    .render(&steps);

    assert!(
        sql.to_uppercase().contains("ALTER TYPE")
            && sql.to_uppercase().contains("RENAME"),
        "enum rename via @@map must use ALTER TYPE ... RENAME TO; today the diff treats \
         it as drop+create which destroys data. Got:\n{sql}"
    );
}
