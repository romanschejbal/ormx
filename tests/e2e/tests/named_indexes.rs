#![allow(clippy::pedantic)]

//! Tests for `@@index([..], name: "...")` and `@@unique([..], name: "...")`.
//!
//! When `name` is set, the rendered SQL uses that custom identifier in
//! place of the auto-generated `idx_<table>_<cols>` /
//! `uq_<table>_<cols>`. `map: "..."` is accepted as an alias for `name:`.
//! Both index and unique attributes accept these args.

use ferriorm_core::types::DatabaseProvider;
use ferriorm_migrate::diff;
use ferriorm_migrate::sql;

fn render_sqlite(steps: &[diff::MigrationStep]) -> String {
    sql::renderer_for(DatabaseProvider::SQLite).render(steps)
}

fn render_postgres(steps: &[diff::MigrationStep]) -> String {
    sql::renderer_for(DatabaseProvider::PostgreSQL).render(steps)
}

#[test]
fn index_with_name_arg_overrides_auto_name() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model Article {
  id      String @id
  slug    String
  authorId String
  @@index([slug, authorId], name: "ix_articles_slug_author")
}
"#;
    let s = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let from = ferriorm_migrate::snapshot::empty_schema(DatabaseProvider::SQLite);
    let steps = diff::diff_schemas(&from, &s, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);

    assert!(
        sql.contains("\"ix_articles_slug_author\""),
        "rendered SQL must use the custom index name. Got:\n{sql}"
    );
    assert!(
        !sql.contains("idx_articles_"),
        "must NOT fall back to the auto-generated name. Got:\n{sql}"
    );
}

#[test]
fn index_with_map_arg_alias_overrides_auto_name() {
    // `map:` is the Prisma-style alias; both `name:` and `map:` set the
    // database identifier in ferriorm.
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model Article {
  id   String @id
  slug String
  @@index([slug], map: "ix_articles_slug_alias")
}
"#;
    let s = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let from = ferriorm_migrate::snapshot::empty_schema(DatabaseProvider::SQLite);
    let steps = diff::diff_schemas(&from, &s, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);
    assert!(
        sql.contains("\"ix_articles_slug_alias\""),
        "`map:` must work as an alias for `name:`. Got:\n{sql}"
    );
}

#[test]
fn index_without_name_falls_back_to_auto_generated_name() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model Article {
  id      String @id
  slug    String
  @@index([slug])
}
"#;
    let s = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let from = ferriorm_migrate::snapshot::empty_schema(DatabaseProvider::SQLite);
    let steps = diff::diff_schemas(&from, &s, DatabaseProvider::SQLite);
    let sql = render_sqlite(&steps);
    assert!(
        sql.contains("\"idx_articles_slug\""),
        "without name: arg, the auto-generated name must be used. Got:\n{sql}"
    );
}

#[test]
fn unique_with_name_arg_overrides_auto_name() {
    let schema = r#"
datasource db { provider = "postgresql" url = "postgresql://x" }
model Subscription {
  id      String @id
  userId  String
  channel String
  @@unique([userId, channel], name: "uq_subs_user_channel")
}
"#;
    let s = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let from = ferriorm_migrate::snapshot::empty_schema(DatabaseProvider::PostgreSQL);
    let steps = diff::diff_schemas(&from, &s, DatabaseProvider::PostgreSQL);
    let sql = render_postgres(&steps);
    assert!(
        sql.contains("\"uq_subs_user_channel\""),
        "rendered SQL must use the custom UNIQUE constraint name. Got:\n{sql}"
    );
    assert!(
        !sql.contains("uq_subscriptions_"),
        "must NOT fall back to the auto-generated UQ name. Got:\n{sql}"
    );
}

#[test]
fn named_index_round_trips_through_snapshot_and_diff_is_idempotent() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model Article {
  id   String @id
  slug String
  @@index([slug], name: "ix_articles_slug_pinned")
}
"#;
    let s = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let json = ferriorm_migrate::snapshot::serialize(&s).expect("serialize");
    let back = ferriorm_migrate::snapshot::deserialize(&json).expect("deserialize");
    let article = back.models.iter().find(|m| m.name == "Article").unwrap();
    assert_eq!(
        article.indexes[0].name.as_deref(),
        Some("ix_articles_slug_pinned"),
        "named index must round-trip through JSON snapshot"
    );

    // Idempotence: diff(B, B) is empty.
    let steps = diff::diff_schemas(&s, &back, DatabaseProvider::SQLite);
    assert!(steps.is_empty(), "named index must not re-emit. Steps: {steps:?}");
}
