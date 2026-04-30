#![allow(clippy::pedantic)]

//! Advanced diff-engine stress tests.
//!
//! These tests probe corners of the diff engine that the original
//! `diff_engine.rs` does not cover: default-value changes, FK cascade /
//! reference changes, composite PK changes, enum-typed column transitions,
//! and idempotence over a wide fixture set.
//!
//! Tests A1-A3 directly target the standing TODO at
//! `crates/ferriorm-migrate/src/diff.rs:386` (`default: None, // TODO: diff
//! defaults`). They are expected to fail today and document the bug.
//!
//! Tests A4-A5 probe the FK constraint-name comparison in
//! `diff_foreign_keys` (`crates/ferriorm-migrate/src/diff.rs:390`): the
//! constraint name is computed from `(table, related_table, fk_col)` only,
//! so changes to `onDelete` actions or to the *referenced* column on the
//! same FK are silently dropped from the migration.

use ferriorm_core::ast::{DefaultValue, LiteralValue, ReferentialAction};
use ferriorm_core::schema::*;
use ferriorm_core::types::{DatabaseProvider, ScalarType};
use ferriorm_core::utils::to_snake_case;
use ferriorm_migrate::diff::{self, MigrationStep};

// ─── Helpers (mirrored from diff_engine.rs) ─────────────────────────

fn make_schema(models: Vec<Model>, enums: Vec<Enum>) -> Schema {
    Schema {
        datasource: DatasourceConfig {
            name: "db".into(),
            provider: DatabaseProvider::SQLite,
            url: String::new(),
        },
        generators: vec![],
        enums,
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

fn make_field_with_default(name: &str, scalar: ScalarType, default: DefaultValue) -> Field {
    let mut f = make_field(name, scalar, false);
    f.default = Some(default);
    f
}

fn make_unique_field(name: &str, scalar: ScalarType) -> Field {
    let mut f = make_field(name, scalar, false);
    f.is_unique = true;
    f
}

fn make_enum_field(name: &str, enum_name: &str) -> Field {
    Field {
        name: name.into(),
        db_name: to_snake_case(name),
        field_type: FieldKind::Enum(enum_name.into()),
        is_optional: false,
        is_list: false,
        is_id: false,
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
            on_update: ReferentialAction::NoAction,
        }),
        db_type: None,
    }
}

// ─── A1-A3: default-value changes (TODO at diff.rs:386) ─────────────

/// A1: `@default(0)` -> `@default(18)` must produce an AlterColumn whose
/// `changes.default` is `Some(Some("18"))`.
///
/// Targets `crates/ferriorm-migrate/src/diff.rs:386` —
/// `default: None, // TODO: diff defaults`. This test is **expected to fail
/// today**; the failure documents the bug.
#[test]
fn default_value_change_emits_alter() {
    let from = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field_with_default(
                    "age",
                    ScalarType::Int,
                    DefaultValue::Literal(LiteralValue::Int(0)),
                ),
            ],
        )],
        vec![],
    );
    let to = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field_with_default(
                    "age",
                    ScalarType::Int,
                    DefaultValue::Literal(LiteralValue::Int(18)),
                ),
            ],
        )],
        vec![],
    );

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let alter = steps.iter().find_map(|s| match s {
        MigrationStep::AlterColumn {
            table,
            column,
            changes,
        } if table == "users" && column == "age" => Some(changes),
        _ => None,
    });

    assert!(
        alter.is_some(),
        "default-value change must emit AlterColumn; today the diff drops it. Steps: {steps:?}"
    );
    let changes = alter.unwrap();
    assert_eq!(
        changes.default,
        Some(Some("18".to_string())),
        "AlterColumn.changes.default must reflect new default. Got: {changes:?}"
    );
}

/// A2: adding `@default("anon")` to a column with no prior default must
/// emit AlterColumn with `default: Some(Some("'anon'"))`.
#[test]
fn default_value_added_emits_alter() {
    let from = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field("name", ScalarType::String, false),
            ],
        )],
        vec![],
    );
    let to = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field_with_default(
                    "name",
                    ScalarType::String,
                    DefaultValue::Literal(LiteralValue::String("anon".into())),
                ),
            ],
        )],
        vec![],
    );

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let alter = steps.iter().find_map(|s| match s {
        MigrationStep::AlterColumn {
            column, changes, ..
        } if column == "name" => Some(changes),
        _ => None,
    });

    assert!(
        alter.is_some(),
        "adding a default must emit AlterColumn. Steps: {steps:?}"
    );
    assert_eq!(
        alter.unwrap().default,
        Some(Some("'anon'".to_string())),
        "newly-added default must appear in changes.default"
    );
}

/// A3: removing a default must emit AlterColumn with `default: Some(None)`.
#[test]
fn default_value_removed_emits_alter() {
    let from = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field_with_default(
                    "name",
                    ScalarType::String,
                    DefaultValue::Literal(LiteralValue::String("anon".into())),
                ),
            ],
        )],
        vec![],
    );
    let to = make_schema(
        vec![make_model(
            "User",
            "users",
            vec![
                make_field("id", ScalarType::String, true),
                make_field("name", ScalarType::String, false),
            ],
        )],
        vec![],
    );

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let alter = steps.iter().find_map(|s| match s {
        MigrationStep::AlterColumn {
            column, changes, ..
        } if column == "name" => Some(changes),
        _ => None,
    });

    assert!(
        alter.is_some(),
        "dropping a default must emit AlterColumn. Steps: {steps:?}"
    );
    assert_eq!(
        alter.unwrap().default,
        Some(None),
        "removed default must surface as Some(None) in changes.default"
    );
}

// ─── A4-A5: foreign-key changes ─────────────────────────────────────

fn user_post_schema(on_delete: ReferentialAction) -> Schema {
    let user = make_model(
        "User",
        "users",
        vec![make_field("id", ScalarType::String, true)],
    );
    let post = make_model(
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
                on_delete,
            ),
        ],
    );
    make_schema(vec![user, post], vec![])
}

/// A4: changing `onDelete: Cascade` -> `Restrict` must emit a
/// DropForeignKey + AddForeignKey pair, otherwise the database keeps
/// the old action and the user's intended migration is silently
/// dropped.
///
/// Targets `crates/ferriorm-migrate/src/diff.rs:390` (`diff_foreign_keys`):
/// the constraint name is `fk_{table}_{related}_{fk_col}` and does NOT
/// include the cascade action, so identical names compare equal even when
/// actions differ.
#[test]
fn fk_cascade_action_change_redrops_fk() {
    let from = user_post_schema(ReferentialAction::Cascade);
    let to = user_post_schema(ReferentialAction::Restrict);

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let has_drop_fk = steps
        .iter()
        .any(|s| matches!(s, MigrationStep::DropForeignKey { .. }));
    let has_add_fk = steps
        .iter()
        .any(|s| matches!(s, MigrationStep::AddForeignKey(_)));

    assert!(
        has_drop_fk && has_add_fk,
        "cascade change must redrop+readd FK to take effect on Postgres; \
         today the diff is empty because the constraint name is unchanged. \
         Steps: {steps:?}"
    );
}

/// A5: changing `references: [id]` -> `references: [legacyId]` must emit a
/// DropForeignKey + AddForeignKey pair. Today, the constraint name only
/// contains `(table, related_table, fk_col)` so the change is invisible to
/// `diff_foreign_keys`. **Expected to fail today.**
#[test]
fn fk_target_column_change() {
    let user_with_legacy = make_model(
        "User",
        "users",
        vec![
            make_field("id", ScalarType::String, true),
            make_unique_field("legacyId", ScalarType::String),
        ],
    );

    let from = make_schema(
        vec![
            user_with_legacy.clone(),
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
                    ),
                ],
            ),
        ],
        vec![],
    );

    let to = make_schema(
        vec![
            user_with_legacy,
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
                        "legacyId",
                        ReferentialAction::Cascade,
                    ),
                ],
            ),
        ],
        vec![],
    );

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let has_drop_fk = steps
        .iter()
        .any(|s| matches!(s, MigrationStep::DropForeignKey { .. }));
    let has_add_fk = steps.iter().any(
        |s| matches!(s, MigrationStep::AddForeignKey(fk) if fk.referenced_column == "legacy_id"),
    );

    assert!(
        has_drop_fk && has_add_fk,
        "FK referenced-column change must produce drop+add. \
         Today the constraint name omits the referenced column so the diff \
         silently drops the change. Steps: {steps:?}"
    );
}

// ─── A6: composite primary key change ───────────────────────────────

/// A6: switching `@@id([a, b])` -> `@@id([a, c])` must surface in the
/// migration. Today, primary-key changes on existing tables have no
/// diff representation at all (no AlterPrimaryKey step exists), so this
/// test pins the contract: at minimum, *some* step must fire.
#[test]
fn composite_pk_change() {
    let from = {
        let mut m = make_model(
            "Membership",
            "memberships",
            vec![
                make_field("a", ScalarType::String, false),
                make_field("b", ScalarType::String, false),
                make_field("c", ScalarType::String, false),
            ],
        );
        m.primary_key = PrimaryKey {
            fields: vec!["a".into(), "b".into()],
        };
        m
    };
    let to = {
        let mut m = make_model(
            "Membership",
            "memberships",
            vec![
                make_field("a", ScalarType::String, false),
                make_field("b", ScalarType::String, false),
                make_field("c", ScalarType::String, false),
            ],
        );
        m.primary_key = PrimaryKey {
            fields: vec!["a".into(), "c".into()],
        };
        m
    };

    let steps = diff::diff_schemas(
        &make_schema(vec![from], vec![]),
        &make_schema(vec![to], vec![]),
        DatabaseProvider::SQLite,
    );

    assert!(
        !steps.is_empty(),
        "composite PK change MUST produce migration steps; \
         today the diff is silent on primary-key changes for existing tables. \
         Steps: {steps:?}"
    );
}

// ─── A7: idempotence with mixed indexes / unique / FK ───────────────

/// A7: regression for `734dd2b` ("indexes re-emitted in every diff").
/// A schema with `@@index`, `@@unique`, `@unique`, and an FK must produce
/// zero steps when diffed against itself, when driven through the full
/// `parse_and_validate` pipeline.
#[test]
fn idempotent_diff_with_indexes_and_unique() {
    let source = r#"
datasource db {
  provider = "sqlite"
  url      = "sqlite::memory:"
}

model User {
  id        String   @id @default(uuid())
  email     String   @unique
  createdAt DateTime @default(now())

  @@map("users")
  @@index([createdAt])
}

model Post {
  id        String @id @default(uuid())
  authorId  String
  slug      String
  category  String
  author    User   @relation(fields: [authorId], references: [id])

  @@map("posts")
  @@unique([slug, category])
  @@index([authorId])
}
"#;
    let schema = ferriorm_parser::parse_and_validate(source).expect("parse");
    let steps = diff::diff_schemas(&schema, &schema, DatabaseProvider::SQLite);
    assert!(
        steps.is_empty(),
        "diff(B, B) must be empty; got steps: {steps:?}"
    );
}

// ─── A8: enum-typed column transition ───────────────────────────────

/// A8: `status Int` -> `status Status` (enum) must emit either an
/// AlterColumn (type change) or a Drop+Add pair. SQLite represents both
/// types as TEXT/INTEGER, so we expect at minimum a non-empty diff and
/// the new column to be TEXT.
#[test]
fn enum_field_type_change_int_to_enum() {
    let from = make_schema(
        vec![make_model(
            "Job",
            "jobs",
            vec![
                make_field("id", ScalarType::String, true),
                make_field("status", ScalarType::Int, false),
            ],
        )],
        vec![],
    );
    let to = make_schema(
        vec![make_model(
            "Job",
            "jobs",
            vec![
                make_field("id", ScalarType::String, true),
                make_enum_field("status", "Status"),
            ],
        )],
        vec![Enum {
            name: "Status".into(),
            db_name: "status".into(),
            variants: vec!["Active".into(), "Inactive".into()],
        }],
    );

    let steps = diff::diff_schemas(&from, &to, DatabaseProvider::SQLite);

    let touches_status = steps.iter().any(|s| match s {
        MigrationStep::AlterColumn { column, .. } if column == "status" => true,
        MigrationStep::DropColumn { column, .. } if column == "status" => true,
        MigrationStep::AddColumn { column, .. } if column.name == "status" => true,
        _ => false,
    });

    assert!(
        touches_status,
        "INT -> Enum column type change must produce a step touching `status`. \
         Steps: {steps:?}"
    );
}

// ─── A9: idempotence over a fixture set ─────────────────────────────

/// A9: table-driven idempotence. Each fixture is parsed and then
/// diffed against itself; the result must be empty. A regression here
/// would re-emit the corresponding step on every `migrate dev`.
#[test]
fn idempotence_over_fixture_set() {
    let fixtures: &[(&str, &str)] = &[
        (
            "single-PK",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A { id String @id }
"#,
        ),
        (
            "compound-unique",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String @id
  a  String
  b  String
  @@unique([a, b])
}
"#,
        ),
        (
            "index",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id   String @id
  name String
  @@index([name])
}
"#,
        ),
        (
            "map-on-fields-and-model",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id        String @id @map("id_col")
  fullName  String @map("full_name")
  @@map("a_table")
}
"#,
        ),
        (
            "optional-with-defaults",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id     String  @id
  age    Int     @default(0)
  active Boolean @default(true)
  bio    String?
}
"#,
        ),
        (
            "fk-cascade",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U { id String @id }
model P {
  id       String @id
  authorId String
  author   U @relation(fields: [authorId], references: [id], onDelete: Cascade)
}
"#,
        ),
        (
            "updated-at",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id        String   @id
  updatedAt DateTime @updatedAt
}
"#,
        ),
        (
            "enum-with-default",
            r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
enum Role { Admin User Guest }
model A {
  id   String @id
  role Role   @default(User)
}
"#,
        ),
    ];

    for (name, source) in fixtures {
        let schema = ferriorm_parser::parse_and_validate(source)
            .unwrap_or_else(|e| panic!("[{name}] parse failed: {e}"));
        let steps = diff::diff_schemas(&schema, &schema, DatabaseProvider::SQLite);
        assert!(
            steps.is_empty(),
            "[{name}] diff(B, B) must be empty; got: {steps:?}"
        );
    }
}
