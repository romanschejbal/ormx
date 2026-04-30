#![allow(clippy::pedantic)]

//! Codegen identifier-collision and structural tests.
//!
//! These tests parse a small schema, generate code into a tempdir, and
//! verify that every generated `.rs` file is valid Rust by feeding it to
//! `syn::parse_file`. Some tests probe specific shapes — self-referencing
//! relations, multi-relations between the same two models, `@db.*` type
//! hints, and field names that collide with names used by the codegen
//! itself (`data`, `and`, `or`, `not`).
//!
//! Several are **expected to fail today** because the codegen does not
//! escape identifiers or namespace generated members against user fields.

use std::panic;
use std::path::Path;

fn generate_to_tempdir(schema_str: &str) -> Result<tempfile::TempDir, String> {
    let schema = ferriorm_parser::parse_and_validate(schema_str)
        .map_err(|e| format!("parse_and_validate failed: {e}"))?;
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let out = tmp.path().join("generated");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        ferriorm_codegen::generator::generate(&schema, &out)
    }));
    match result {
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".to_string());
            Err(format!("codegen panicked: {msg}"))
        }
        Ok(Err(e)) => Err(format!("codegen returned error: {e}")),
        Ok(Ok(())) => Ok(tmp),
    }
}

fn assert_all_generated_files_parse(tmp: &Path) {
    let out = tmp.join("generated");
    let entries =
        std::fs::read_dir(&out).unwrap_or_else(|e| panic!("read_dir {out:?}: {e}"));
    for entry in entries {
        let path = entry.unwrap().path();
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        syn::parse_file(&content).unwrap_or_else(|e| {
            panic!(
                "generated file {} is not valid Rust:\n{e}\n--- file ---\n{content}",
                path.display()
            )
        });
    }
}

// ─── C1: field named `data` ─────────────────────────────────────────

/// The generated `Update*Input` carries a `.data` member used by the
/// query builder. A user field also named `data` either shadows the
/// builder member or produces a duplicate field. **Expected to fail
/// today** if codegen doesn't disambiguate.
#[test]
fn field_named_data_does_not_collide_with_create_input_data() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
generator client { output = "./src/generated" }

model Event {
  id   String @id @default(uuid())
  data String
  @@map("events")
}
"#;
    let tmp = generate_to_tempdir(schema)
        .unwrap_or_else(|e| panic!("codegen for `data` field failed: {e}"));
    assert_all_generated_files_parse(tmp.path());
}

// ─── C2: field names `and`, `or`, `not` ─────────────────────────────

/// Filter inputs carry `and`, `or`, `not` members for boolean
/// composition. A user field with one of these names collides.
#[test]
fn field_named_and_or_not() {
    for fname in ["and", "or", "not"] {
        let schema = format!(
            r#"
datasource db {{ provider = "sqlite" url = "sqlite::memory:" }}
generator client {{ output = "./src/generated" }}

model Rule {{
  id    String @id @default(uuid())
  {fname} String
  @@map("rules")
}}
"#
        );
        let tmp = generate_to_tempdir(&schema)
            .unwrap_or_else(|e| panic!("codegen for `{fname}` field failed: {e}"));
        assert_all_generated_files_parse(tmp.path());
    }
}

// ─── C3: self-referencing relation ──────────────────────────────────

/// A model with an FK to itself (tree structure) must generate valid
/// Rust. The Include / WithRelations type generation must terminate.
/// (The parser does not currently support named relations, so we use a
/// single back-reference; this also pins the limitation.)
#[test]
fn self_referencing_relation_compiles() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
generator client { output = "./src/generated" }

model Tree {
  id       String  @id @default(uuid())
  name     String
  parentId String?
  parent   Tree?   @relation(fields: [parentId], references: [id])
  children Tree[]
  @@map("trees")
}
"#;
    let tmp = generate_to_tempdir(schema)
        .unwrap_or_else(|e| panic!("self-referencing relation codegen failed: {e}"));
    assert_all_generated_files_parse(tmp.path());
}

// ─── C4: multiple relations between the same two models ─────────────

/// `Post.author` and `Post.reviewer` both point to `User` via two
/// different FK fields. With named relations (`@relation("Authored",
/// ...)`), the validator pairs forward and back references by name and
/// codegen must produce valid Rust with distinct accessors on both
/// sides.
#[test]
fn multiple_relations_same_target() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
generator client { output = "./src/generated" }

model User {
  id       String @id @default(uuid())
  authored Post[] @relation("Authored")
  reviewed Post[] @relation("Reviewed")
  @@map("users")
}

model Post {
  id         String @id @default(uuid())
  title      String
  authorId   String
  reviewerId String
  author     User   @relation("Authored", fields: [authorId], references: [id])
  reviewer   User   @relation("Reviewed", fields: [reviewerId], references: [id])
  @@map("posts")
}
"#;
    let tmp = generate_to_tempdir(schema)
        .unwrap_or_else(|e| panic!("multi-relation codegen failed: {e}"));
    assert_all_generated_files_parse(tmp.path());

    // The User-side accessors must wire through their respective FK
    // columns: `authored` -> author_id, `reviewed` -> reviewer_id.
    let user_src =
        std::fs::read_to_string(tmp.path().join("generated").join("user.rs")).unwrap();
    assert!(
        user_src.contains("author_id"),
        "User accessors must reference author_id; codegen failed to pair `authored` \
         with `author` via @relation(\"Authored\")"
    );
    assert!(
        user_src.contains("reviewer_id"),
        "User accessors must reference reviewer_id; codegen failed to pair `reviewed` \
         with `reviewer` via @relation(\"Reviewed\")"
    );
}

// ─── C5: @db.* native type hints survive ────────────────────────────

/// `@db.BigInt` must produce BIGINT in Postgres rendering (already
/// covered by `diff_engine.rs`); we additionally pin that the IR
/// preserves the `@db.*` hint after parsing. A regression that drops
/// the hint at the parser layer would let bug `219b2f0` reappear.
#[test]
fn db_native_types_preserved_in_ir() {
    let schema = r#"
datasource db { provider = "postgresql" url = "postgresql://x" }
generator client { output = "./src/generated" }

model Stat {
  id        String @id @default(uuid())
  viewCount Int    @db.BigInt
  @@map("stats")
}
"#;
    let ir = ferriorm_parser::parse_and_validate(schema).expect("parse");
    let stat = ir.models.iter().find(|m| m.name == "Stat").expect("Stat");
    let view_count = stat
        .fields
        .iter()
        .find(|f| f.name == "viewCount")
        .expect("viewCount");
    assert_eq!(
        view_count.db_type.as_ref().map(|(t, _)| t.as_str()),
        Some("BigInt"),
        "@db.BigInt hint must survive into the Schema IR. Got: {:?}",
        view_count.db_type
    );
}
