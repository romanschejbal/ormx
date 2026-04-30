#![allow(clippy::pedantic)]

//! Tests for optional `@relation("Name", ...)` disambiguation.
//!
//! Relation names allow multiple foreign keys between the same two
//! models to be paired correctly. The validator requires names when
//! ambiguity exists; single-relation schemas continue to work without
//! them.
//!
//! Both syntaxes are supported:
//!   - positional: `@relation("Authored", fields: [...], references: [...])`
//!   - named arg:  `@relation(name: "Authored", fields: [...], references: [...])`
//!
//! Forward and back-reference fields with matching names are paired by
//! the codegen.

use std::panic;

fn parse(schema: &str) -> ferriorm_core::schema::Schema {
    ferriorm_parser::parse_and_validate(schema).expect("parse_and_validate")
}

fn parse_err(schema: &str) -> String {
    ferriorm_parser::parse_and_validate(schema)
        .err()
        .map(|e| e.to_string())
        .expect("expected validation error")
}

fn codegen_to_tempdir(schema: &str) -> tempfile::TempDir {
    let ir = parse(schema);
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = tmp.path().join("generated");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        ferriorm_codegen::generator::generate(&ir, &out)
    }));
    match result {
        Ok(Ok(())) => tmp,
        Ok(Err(e)) => panic!("codegen returned error: {e}"),
        Err(p) => {
            let msg = p
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| p.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".into());
            panic!("codegen panicked: {msg}");
        }
    }
}

// ─── Parser surface ─────────────────────────────────────────────────

#[test]
fn relation_name_positional_first_arg_parses() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U { id String @id }
model P {
  id       String @id
  authorId String
  author   U      @relation("Authored", fields: [authorId], references: [id])
}
"#;
    let ir = parse(schema);
    let post = ir.models.iter().find(|m| m.name == "P").unwrap();
    let author = post.fields.iter().find(|f| f.name == "author").unwrap();
    let rel = author.relation.as_ref().expect("relation present");
    assert_eq!(rel.name.as_deref(), Some("Authored"));
}

#[test]
fn relation_name_named_arg_parses() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U { id String @id }
model P {
  id       String @id
  authorId String
  author   U      @relation(name: "Authored", fields: [authorId], references: [id])
}
"#;
    let ir = parse(schema);
    let post = ir.models.iter().find(|m| m.name == "P").unwrap();
    let author = post.fields.iter().find(|f| f.name == "author").unwrap();
    let rel = author.relation.as_ref().expect("relation present");
    assert_eq!(rel.name.as_deref(), Some("Authored"));
}

#[test]
fn relation_without_name_still_works_for_single_relation() {
    // The pre-existing behavior must still be supported.
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U { id String @id }
model P {
  id       String @id
  authorId String
  author   U      @relation(fields: [authorId], references: [id])
}
"#;
    let ir = parse(schema);
    let post = ir.models.iter().find(|m| m.name == "P").unwrap();
    let author = post.fields.iter().find(|f| f.name == "author").unwrap();
    let rel = author.relation.as_ref().expect("relation present");
    assert!(rel.name.is_none());
}

// ─── Validator: disambiguation contract ─────────────────────────────

#[test]
fn multi_relation_without_names_is_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id       String @id
  authored P[]
  reviewed P[]
}
model P {
  id         String @id
  authorId   String
  reviewerId String
  author     U      @relation(fields: [authorId], references: [id])
  reviewer   U      @relation(fields: [reviewerId], references: [id])
}
"#;
    let err = parse_err(schema);
    let msg = err.to_lowercase();
    assert!(
        msg.contains("multiple relations") || msg.contains("disambiguat"),
        "error must mention disambiguation; got: {err}"
    );
}

#[test]
fn multi_relation_with_unique_names_validates() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id       String @id
  authored P[]    @relation("Authored")
  reviewed P[]    @relation("Reviewed")
}
model P {
  id         String @id
  authorId   String
  reviewerId String
  author     U      @relation("Authored", fields: [authorId], references: [id])
  reviewer   U      @relation("Reviewed", fields: [reviewerId], references: [id])
}
"#;
    let ir = parse(schema);
    let p = ir.models.iter().find(|m| m.name == "P").unwrap();
    let author = p.fields.iter().find(|f| f.name == "author").unwrap();
    let reviewer = p.fields.iter().find(|f| f.name == "reviewer").unwrap();
    assert_eq!(
        author.relation.as_ref().and_then(|r| r.name.as_deref()),
        Some("Authored")
    );
    assert_eq!(
        reviewer.relation.as_ref().and_then(|r| r.name.as_deref()),
        Some("Reviewed")
    );
}

#[test]
fn multi_relation_with_duplicate_names_is_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id       String @id
  authored P[]    @relation("Same")
  reviewed P[]    @relation("Same")
}
model P {
  id         String @id
  authorId   String
  reviewerId String
  author     U      @relation("Same", fields: [authorId], references: [id])
  reviewer   U      @relation("Same", fields: [reviewerId], references: [id])
}
"#;
    let err = parse_err(schema);
    let msg = err.to_lowercase();
    assert!(
        msg.contains("duplicate") && msg.contains("same"),
        "error must call out duplicate relation name; got: {err}"
    );
}

// ─── Codegen: back-reference pairing ────────────────────────────────

#[test]
fn codegen_pairs_named_relations_to_correct_fk_columns() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
generator client { output = "./src/generated" }

model U {
  id       String @id
  authored P[]    @relation("Authored")
  reviewed P[]    @relation("Reviewed")
  @@map("u")
}
model P {
  id         String @id
  authorId   String
  reviewerId String
  author     U      @relation("Authored", fields: [authorId], references: [id])
  reviewer   U      @relation("Reviewed", fields: [reviewerId], references: [id])
  @@map("p")
}
"#;
    let tmp = codegen_to_tempdir(schema);
    let user_rs = std::fs::read_to_string(tmp.path().join("generated").join("u.rs")).unwrap();

    // `authored` accessor must wire through `author_id`.
    // `reviewed` accessor must wire through `reviewer_id`.
    // These are columns referenced in the generated SQL for the
    // relation loaders, so the substring assertions are robust.
    assert!(
        user_rs.contains("author_id"),
        "User must wire `authored` through author_id. user.rs:\n{user_rs}"
    );
    assert!(
        user_rs.contains("reviewer_id"),
        "User must wire `reviewed` through reviewer_id. user.rs:\n{user_rs}"
    );

    // syn must accept the generated file as valid Rust.
    syn::parse_file(&user_rs).unwrap_or_else(|e| panic!("generated u.rs is not valid Rust: {e}"));
}

#[test]
fn codegen_unnamed_single_relation_still_works() {
    // Regression guard: introducing relation names must not break the
    // common case of single relations without names.
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
generator client { output = "./src/generated" }

model User {
  id    String @id
  posts Post[]
  @@map("users")
}
model Post {
  id       String @id
  authorId String
  author   User   @relation(fields: [authorId], references: [id])
  @@map("posts")
}
"#;
    let tmp = codegen_to_tempdir(schema);
    let user_rs = std::fs::read_to_string(tmp.path().join("generated").join("user.rs")).unwrap();
    syn::parse_file(&user_rs).expect("user.rs must be valid Rust");
    assert!(user_rs.contains("author_id"));
}

// ─── Round-trip through snapshot serde ──────────────────────────────

#[test]
fn relation_name_round_trips_through_snapshot() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U { id String @id }
model P {
  id       String @id
  authorId String
  author   U      @relation("Authored", fields: [authorId], references: [id])
}
"#;
    let ir = parse(schema);
    let json = ferriorm_migrate::snapshot::serialize(&ir).expect("serialize");
    let back = ferriorm_migrate::snapshot::deserialize(&json).expect("deserialize");
    let p = back.models.iter().find(|m| m.name == "P").unwrap();
    let author = p.fields.iter().find(|f| f.name == "author").unwrap();
    assert_eq!(
        author.relation.as_ref().and_then(|r| r.name.as_deref()),
        Some("Authored")
    );
}
