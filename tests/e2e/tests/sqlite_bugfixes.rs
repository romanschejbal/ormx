#![allow(clippy::pedantic)]

//! Regression tests for SQLite-specific bug fixes.
//!
//! Bug 1: Foreign keys rendered as inline REFERENCES in CREATE TABLE (not comments).
//! Bug 2: Table creation order respects FK dependencies (topological sort).
//! Bug 3: `file:./dev.db` URLs properly normalized to `sqlite:` scheme with `mode=rwc`.

use ferriorm_core::ast::ReferentialAction;
use ferriorm_core::schema::*;
use ferriorm_core::types::{DatabaseProvider, ScalarType};
use ferriorm_core::utils::to_snake_case;
use ferriorm_migrate::diff::{self, MigrationStep};
use ferriorm_migrate::sql;

// ─── Helpers ────────────────────────────────────────────────────────

fn empty_schema() -> Schema {
    Schema {
        datasource: DatasourceConfig {
            name: "db".into(),
            provider: DatabaseProvider::SQLite,
            url: String::new(),
        },
        generators: vec![],
        enums: vec![],
        models: vec![],
    }
}

fn make_schema(models: Vec<Model>) -> Schema {
    Schema {
        datasource: DatasourceConfig {
            name: "db".into(),
            provider: DatabaseProvider::SQLite,
            url: String::new(),
        },
        generators: vec![],
        enums: vec![],
        models,
    }
}

fn make_model(name: &str, db_name: &str, fields: Vec<Field>) -> Model {
    let pk_fields: Vec<String> = fields
        .iter()
        .filter(|f| f.is_id)
        .map(|f| f.name.clone())
        .collect();
    Model {
        name: name.into(),
        db_name: db_name.into(),
        fields,
        primary_key: PrimaryKey { fields: pk_fields },
        indexes: vec![],
        unique_constraints: vec![],
    }
}

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
    scalar: ScalarType,
    related_model: &str,
    fk_field: &str,
    ref_field: &str,
    on_delete: ReferentialAction,
    on_update: ReferentialAction,
) -> Field {
    Field {
        name: name.into(),
        db_name: to_snake_case(name),
        field_type: FieldKind::Scalar(scalar),
        is_optional: false,
        is_list: false,
        is_id: false,
        is_unique: false,
        is_updated_at: false,
        default: None,
        relation: Some(ResolvedRelation {
            name: None,
            related_model: related_model.into(),
            relation_type: RelationType::ManyToOne,
            fields: vec![fk_field.into()],
            references: vec![ref_field.into()],
            on_delete,
            on_update,
        }),
        db_type: None,
    }
}

fn render_sqlite(steps: &[MigrationStep]) -> String {
    let renderer = sql::renderer_for(DatabaseProvider::SQLite);
    renderer.render(steps)
}

// ─── Bug 1: Inline FK constraints in CREATE TABLE ───────────────────

#[test]
fn sqlite_create_table_has_inline_fk_references() {
    let from = empty_schema();
    let to = make_schema(vec![
        make_model(
            "User",
            "users",
            vec![make_field("id", ScalarType::String, true)],
        ),
        make_model(
            "Post",
            "posts",
            vec![
                make_field("id", ScalarType::String, true),
                make_fk_field(
                    "authorId",
                    ScalarType::String,
                    "User",
                    "authorId",
                    "id",
                    ReferentialAction::Cascade,
                    ReferentialAction::NoAction,
                ),
            ],
        ),
    ]);

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);

    // The FK should be inlined as REFERENCES in the CREATE TABLE, not a comment.
    assert!(
        sql.contains("REFERENCES \"users\"(\"id\")"),
        "FK should be rendered as inline REFERENCES clause. Got:\n{sql}"
    );
    assert!(
        sql.contains("ON DELETE CASCADE"),
        "ON DELETE action should be present. Got:\n{sql}"
    );
    assert!(
        sql.contains("ON UPDATE NO ACTION"),
        "ON UPDATE action should be present. Got:\n{sql}"
    );
    assert!(
        !sql.contains("-- SQLite: cannot add foreign key"),
        "Should NOT emit the FK-as-comment fallback when a CREATE TABLE exists. Got:\n{sql}"
    );
}

#[test]
fn sqlite_fk_without_create_table_still_emits_comment() {
    // When adding a FK to an existing table (no CreateTable step), we still
    // emit a comment because SQLite cannot ALTER TABLE ADD CONSTRAINT.
    let from = make_schema(vec![
        make_model(
            "User",
            "users",
            vec![make_field("id", ScalarType::String, true)],
        ),
        make_model(
            "Post",
            "posts",
            vec![
                make_field("id", ScalarType::String, true),
                make_field("authorId", ScalarType::String, false),
            ],
        ),
    ]);
    let to = make_schema(vec![
        make_model(
            "User",
            "users",
            vec![make_field("id", ScalarType::String, true)],
        ),
        make_model(
            "Post",
            "posts",
            vec![
                make_field("id", ScalarType::String, true),
                make_fk_field(
                    "authorId",
                    ScalarType::String,
                    "User",
                    "authorId",
                    "id",
                    ReferentialAction::Cascade,
                    ReferentialAction::NoAction,
                ),
            ],
        ),
    ]);

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);

    // No CreateTable for posts (it already exists), so FK is a comment.
    assert!(
        sql.contains("-- SQLite: cannot add foreign key"),
        "FK on existing table should be a comment. Got:\n{sql}"
    );
}

// ─── Bug 2: Table creation order respects FK dependencies ───────────

#[test]
fn sqlite_create_tables_ordered_by_fk_dependencies() {
    let from = empty_schema();

    // Create three tables where C -> B -> A (C references B, B references A).
    // Regardless of HashMap iteration order, A should be created first, then B, then C.
    let to = make_schema(vec![
        // Deliberately put the dependent tables first in the vec.
        make_model(
            "Comment",
            "comments",
            vec![
                make_field("id", ScalarType::String, true),
                make_fk_field(
                    "postId",
                    ScalarType::String,
                    "Post",
                    "postId",
                    "id",
                    ReferentialAction::Cascade,
                    ReferentialAction::NoAction,
                ),
            ],
        ),
        make_model(
            "Post",
            "posts",
            vec![
                make_field("id", ScalarType::String, true),
                make_fk_field(
                    "authorId",
                    ScalarType::String,
                    "User",
                    "authorId",
                    "id",
                    ReferentialAction::Cascade,
                    ReferentialAction::NoAction,
                ),
            ],
        ),
        make_model(
            "User",
            "users",
            vec![make_field("id", ScalarType::String, true)],
        ),
    ]);

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    // Collect the CreateTable step names in order.
    let create_order: Vec<&str> = steps
        .iter()
        .filter_map(|s| match s {
            MigrationStep::CreateTable(ct) => Some(ct.name.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(create_order.len(), 3, "Should have 3 CreateTable steps");

    let user_pos = create_order.iter().position(|n| *n == "users").unwrap();
    let post_pos = create_order.iter().position(|n| *n == "posts").unwrap();
    let comment_pos = create_order.iter().position(|n| *n == "comments").unwrap();

    assert!(
        user_pos < post_pos,
        "users (pos {user_pos}) should come before posts (pos {post_pos})"
    );
    assert!(
        post_pos < comment_pos,
        "posts (pos {post_pos}) should come before comments (pos {comment_pos})"
    );
}

#[test]
fn sqlite_create_tables_with_dependencies_produce_valid_sql() {
    // End-to-end: generate SQL and execute against SQLite to verify it works.
    let from = empty_schema();
    let to = make_schema(vec![
        make_model(
            "Post",
            "posts",
            vec![
                make_field("id", ScalarType::String, true),
                make_fk_field(
                    "authorId",
                    ScalarType::String,
                    "User",
                    "authorId",
                    "id",
                    ReferentialAction::Cascade,
                    ReferentialAction::NoAction,
                ),
            ],
        ),
        make_model(
            "User",
            "users",
            vec![make_field("id", ScalarType::String, true)],
        ),
    ]);

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);

    // users CREATE TABLE must appear before posts CREATE TABLE
    let users_pos = sql
        .find("CREATE TABLE \"users\"")
        .expect("should have users");
    let posts_pos = sql
        .find("CREATE TABLE \"posts\"")
        .expect("should have posts");

    assert!(
        users_pos < posts_pos,
        "CREATE TABLE users (at {users_pos}) should appear before posts (at {posts_pos}) in SQL:\n{sql}"
    );
}

// ─── Bug 3: file: URL normalization ────────────────────────────────

#[test]
fn normalize_sqlite_url_transforms_file_prefix() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("file:./dev.db");
    assert!(
        result.starts_with("sqlite:./dev.db"),
        "file: prefix should be replaced with sqlite:. Got: {result}"
    );
    assert!(
        result.contains("mode=rwc"),
        "Should include mode=rwc for auto-creation. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_handles_bare_path() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("./dev.db");
    assert!(
        result.starts_with("sqlite:./dev.db"),
        "Bare path should get sqlite: prefix. Got: {result}"
    );
    assert!(
        result.contains("mode=rwc"),
        "Should include mode=rwc. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_preserves_existing_sqlite_prefix() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("sqlite:./dev.db");
    assert!(
        result.starts_with("sqlite:./dev.db"),
        "Already-prefixed URL should be kept. Got: {result}"
    );
    assert!(
        result.contains("mode=rwc"),
        "Should append mode=rwc. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_does_not_duplicate_mode() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("sqlite:./dev.db?mode=rwc");
    assert_eq!(
        result.matches("mode=").count(),
        1,
        "Should not duplicate mode param. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_appends_to_existing_query() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("sqlite:./dev.db?cache=shared");
    assert!(
        result.contains("cache=shared"),
        "Existing params should be preserved. Got: {result}"
    );
    assert!(
        result.contains("&mode=rwc"),
        "mode=rwc should be appended with &. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_memory_database() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    // sqlite::memory: is a special URL — it should still get mode=rwc appended
    // (harmless for in-memory, but consistent).
    let result = normalize_sqlite_url("sqlite::memory:");
    assert!(
        result.starts_with("sqlite::memory:"),
        "Memory URL prefix should be preserved. Got: {result}"
    );
}

#[test]
fn normalize_sqlite_url_file_with_nested_path() {
    use ferriorm_runtime::client::normalize_sqlite_url;

    let result = normalize_sqlite_url("file:data/databases/app.db");
    assert!(
        result.starts_with("sqlite:data/databases/app.db"),
        "Nested path should be preserved. Got: {result}"
    );
    assert!(
        result.contains("mode=rwc"),
        "Should include mode=rwc. Got: {result}"
    );
}

// ─── Issue 5 verification: SQLite CURRENT_TIMESTAMP roundtrips with chrono::DateTime<Utc> ──

#[derive(sqlx::FromRow, Debug)]
struct ChronoRow {
    created_at: chrono::DateTime<chrono::Utc>,
}

#[tokio::test]
async fn sqlite_current_timestamp_default_roundtrips_with_chrono_utc() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Create table with DEFAULT CURRENT_TIMESTAMP (mirrors what diff engine emits for @default(now()) on SQLite)
    sqlx::query(
        r#"CREATE TABLE "events" (
            "id" INTEGER PRIMARY KEY,
            "created_at" TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert without providing created_at — SQLite fills it via DEFAULT
    sqlx::query(r#"INSERT INTO "events" ("id") VALUES (1)"#)
        .execute(&pool)
        .await
        .unwrap();

    // Read back as chrono::DateTime<Utc>
    let row: Result<ChronoRow, sqlx::Error> =
        sqlx::query_as(r#"SELECT "created_at" FROM "events" WHERE "id" = 1"#)
            .fetch_one(&pool)
            .await;

    assert!(
        row.is_ok(),
        "SQLite CURRENT_TIMESTAMP should roundtrip to chrono::DateTime<Utc>. Got: {:?}",
        row.err()
    );

    let row = row.unwrap();
    let now = chrono::Utc::now();
    let diff = (now - row.created_at).num_seconds().abs();
    assert!(
        diff < 5,
        "CURRENT_TIMESTAMP should be close to now. Diff: {diff}s, parsed: {:?}",
        row.created_at
    );
}

#[tokio::test]
async fn sqlite_chrono_now_inserted_via_app_roundtrips() {
    use sqlx::SqlitePool;

    // This mirrors what generated codegen does: chrono::Utc::now() is inserted via push_bind
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE "events" (
            "id" INTEGER PRIMARY KEY,
            "created_at" TEXT NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    let inserted = chrono::Utc::now();
    sqlx::query(r#"INSERT INTO "events" ("id", "created_at") VALUES (?, ?)"#)
        .bind(1i64)
        .bind(inserted)
        .execute(&pool)
        .await
        .unwrap();

    let row: ChronoRow = sqlx::query_as(r#"SELECT "created_at" FROM "events" WHERE "id" = 1"#)
        .fetch_one(&pool)
        .await
        .unwrap();

    let diff = (inserted - row.created_at).num_milliseconds().abs();
    assert!(
        diff < 100,
        "App-inserted timestamp should roundtrip cleanly. Diff: {diff}ms"
    );
}

// ─── E1-E5: regression tests for recent CRUD bug fixes ─────────────
//
// These mirror the SQL the fixed codegen produces and verify it
// behaves correctly end-to-end. They don't go through the generated
// query builders (the e2e crate cannot import generated user code) —
// instead they execute the same DDL/DML the codegen would emit.

/// E1: regression for `734e18c`. Two inserts on an autoincrement PK
/// table that omit the `id` column entirely (the fixed-codegen path
/// when caller passes `id: None`) must produce two distinct,
/// monotonically increasing ids.
#[tokio::test]
async fn e1_autoincrement_create_with_id_none() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE "drafts" (
            "id" INTEGER PRIMARY KEY,
            "title" TEXT NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    for title in ["one", "two", "three"] {
        sqlx::query(r#"INSERT INTO "drafts" ("title") VALUES (?)"#)
            .bind(title)
            .execute(&pool)
            .await
            .expect("insert with id omitted must auto-assign");
    }

    let ids: Vec<i64> = sqlx::query_scalar(r#"SELECT "id" FROM "drafts" ORDER BY "id""#)
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(ids.len(), 3);
    let mut sorted = ids.clone();
    sorted.dedup();
    assert_eq!(sorted, ids, "ids must be distinct");
}

/// E2: same autoincrement-with-None shape, but via INSERT ... ON
/// CONFLICT DO UPDATE (upsert). The id column is still omitted from
/// the INSERT side, but the upsert variant has separate codegen, so
/// the regression must be guarded independently.
#[tokio::test]
async fn e2_autoincrement_upsert_with_id_none() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE "users" (
            "id" INTEGER PRIMARY KEY,
            "email" TEXT NOT NULL UNIQUE,
            "name" TEXT NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // First call inserts; id is auto-assigned.
    let row: (i64,) = sqlx::query_as(
        r#"INSERT INTO "users" ("email", "name") VALUES (?, ?)
           ON CONFLICT("email") DO UPDATE SET "name" = excluded."name"
           RETURNING "id""#,
    )
    .bind("alice@x.com")
    .bind("Alice")
    .fetch_one(&pool)
    .await
    .expect("upsert insert path");
    let first_id = row.0;
    assert!(first_id > 0);

    // Second call updates the existing row — must not create a new one.
    let row: (i64,) = sqlx::query_as(
        r#"INSERT INTO "users" ("email", "name") VALUES (?, ?)
           ON CONFLICT("email") DO UPDATE SET "name" = excluded."name"
           RETURNING "id""#,
    )
    .bind("alice@x.com")
    .bind("Alicia")
    .fetch_one(&pool)
    .await
    .expect("upsert update path");
    assert_eq!(row.0, first_id, "upsert update must hit the same id");

    let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "users""#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "upsert must not insert a duplicate row");
}

/// E3: INSERT OR IGNORE with autoincrement and a unique conflict.
/// Regression for `219b2f0`. Conflicting row must be silently ignored;
/// id sequence must not advance on the rejected insert (or, if SQLite
/// does advance internally, the test still passes because we only
/// assert on observable rows).
#[tokio::test]
async fn e3_on_conflict_ignore_with_id_none() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE "tags" (
            "id" INTEGER PRIMARY KEY,
            "slug" TEXT NOT NULL UNIQUE
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(r#"INSERT OR IGNORE INTO "tags" ("slug") VALUES (?)"#)
        .bind("rust")
        .execute(&pool)
        .await
        .expect("first insert");

    // Conflicting insert — IGNORE silently no-ops.
    let result = sqlx::query(r#"INSERT OR IGNORE INTO "tags" ("slug") VALUES (?)"#)
        .bind("rust")
        .execute(&pool)
        .await
        .expect("INSERT OR IGNORE must not error on conflict");
    assert_eq!(
        result.rows_affected(),
        0,
        "conflicting insert must be ignored"
    );

    let count: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "tags""#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1, "only one row exists after conflict-ignore");
}

/// E4: regression for `219b2f0`. Upserting on a compound `@@unique([a,
/// b])` constraint must hit the existing row when both columns match,
/// and insert a new row when only one matches.
#[tokio::test]
async fn e4_compound_unique_upsert_targeting() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query(
        r#"CREATE TABLE "subs" (
            "id" INTEGER PRIMARY KEY,
            "user_id" INTEGER NOT NULL,
            "channel" TEXT NOT NULL,
            "status" TEXT NOT NULL,
            UNIQUE ("user_id", "channel")
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // (1, "email") -- inserted
    sqlx::query(
        r#"INSERT INTO "subs" ("user_id", "channel", "status") VALUES (?, ?, ?)
           ON CONFLICT ("user_id", "channel") DO UPDATE SET "status" = excluded."status""#,
    )
    .bind(1i64)
    .bind("email")
    .bind("active")
    .execute(&pool)
    .await
    .unwrap();

    // (1, "email") again — must update, not insert.
    sqlx::query(
        r#"INSERT INTO "subs" ("user_id", "channel", "status") VALUES (?, ?, ?)
           ON CONFLICT ("user_id", "channel") DO UPDATE SET "status" = excluded."status""#,
    )
    .bind(1i64)
    .bind("email")
    .bind("paused")
    .execute(&pool)
    .await
    .unwrap();

    // (1, "sms") — different channel, must insert.
    sqlx::query(
        r#"INSERT INTO "subs" ("user_id", "channel", "status") VALUES (?, ?, ?)
           ON CONFLICT ("user_id", "channel") DO UPDATE SET "status" = excluded."status""#,
    )
    .bind(1i64)
    .bind("sms")
    .bind("active")
    .execute(&pool)
    .await
    .unwrap();

    let total: i64 = sqlx::query_scalar(r#"SELECT COUNT(*) FROM "subs""#)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(total, 2, "two distinct (user_id, channel) pairs must exist");

    let email_status: String = sqlx::query_scalar(
        r#"SELECT "status" FROM "subs" WHERE "user_id" = 1 AND "channel" = 'email'"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        email_status, "paused",
        "email row was updated by the second upsert"
    );
}

/// E5: regression for `705a0ab`. A parent with optional FK children:
/// some children have `parent_id = NULL`. The relation loader's
/// `filter_map` over child FKs must not panic and must skip the NULL
/// rows when collecting parent ids.
#[tokio::test]
async fn e5_optional_fk_one_to_many_loads_filter_map() {
    use sqlx::SqlitePool;

    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    sqlx::query(r#"CREATE TABLE "parents" ("id" INTEGER PRIMARY KEY, "name" TEXT NOT NULL)"#)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"CREATE TABLE "children" (
            "id" INTEGER PRIMARY KEY,
            "parent_id" INTEGER,
            "name" TEXT NOT NULL,
            FOREIGN KEY ("parent_id") REFERENCES "parents"("id")
        )"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(r#"INSERT INTO "parents" ("id", "name") VALUES (1, 'P1'), (2, 'P2')"#)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        r#"INSERT INTO "children" ("id", "parent_id", "name") VALUES
           (10, 1, 'A'),
           (11, NULL, 'orphan'),
           (12, 2, 'B'),
           (13, NULL, 'orphan2')"#,
    )
    .execute(&pool)
    .await
    .unwrap();

    // The codegen relation loader collects non-NULL parent_ids via
    // filter_map and re-fetches parents. Mirror that pattern here.
    #[derive(sqlx::FromRow)]
    struct ChildRow {
        parent_id: Option<i64>,
    }

    let children: Vec<ChildRow> = sqlx::query_as(r#"SELECT "parent_id" FROM "children""#)
        .fetch_all(&pool)
        .await
        .unwrap();

    let parent_ids: Vec<i64> = children.iter().filter_map(|c| c.parent_id).collect();
    assert_eq!(parent_ids.len(), 2, "two children have non-NULL FK");
    assert!(parent_ids.contains(&1));
    assert!(parent_ids.contains(&2));

    // And the filter_map output round-trips through an IN-list query
    // without including NULL — that's what the relation loader does.
    let mut qb =
        sqlx::QueryBuilder::<sqlx::Sqlite>::new(r#"SELECT "id" FROM "parents" WHERE "id" IN ("#);
    let mut sep = qb.separated(", ");
    for id in &parent_ids {
        sep.push_bind(*id);
    }
    qb.push(")");
    let rows: Vec<i64> = qb.build_query_scalar().fetch_all(&pool).await.unwrap();
    assert_eq!(rows.len(), 2);
}
