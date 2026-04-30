#![allow(clippy::pedantic)]

//! Parser/validator edge-case tests.
//!
//! Each test pins a contract that the validator (or downstream codegen)
//! should enforce. Several are **expected to fail today** because the
//! validator currently performs only a small subset of checks (missing
//! primary key, unknown types, duplicate enum/model names). The failures
//! document gaps to be closed.
//!
//! See `crates/ferriorm-parser/src/validator.rs` for the live checks.

use std::panic;

/// Run `parse_and_validate(schema)` and `generator::generate(...)` to a
/// tempdir, catching panics. Returns:
/// - `Ok(())` if validation rejected the schema (acceptable contract).
/// - `Ok(())` if codegen completed and produced parseable Rust files.
/// - `Err(msg)` if validation succeeded but codegen panicked, or if
///   generated files failed to parse as Rust.
fn schema_either_rejects_or_codegens_cleanly(schema: &str) -> Result<(), String> {
    let parsed = ferriorm_parser::parse_and_validate(schema);
    let Ok(schema_ir) = parsed else {
        return Ok(());
    };

    let tmp_dir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let output_dir = tmp_dir.path().join("generated");

    let result = panic::catch_unwind(panic::AssertUnwindSafe(|| {
        ferriorm_codegen::generator::generate(&schema_ir, &output_dir)
    }));

    match result {
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| panic_payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".to_string());
            Err(format!("codegen panicked: {msg}"))
        }
        Ok(Err(e)) => Err(format!("codegen returned error: {e}")),
        Ok(Ok(())) => {
            for file in std::fs::read_dir(&output_dir).map_err(|e| format!("read_dir: {e}"))? {
                let path = file.map_err(|e| format!("dir entry: {e}"))?.path();
                if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    let content = std::fs::read_to_string(&path)
                        .map_err(|e| format!("read {}: {e}", path.display()))?;
                    syn::parse_file(&content).map_err(|e| {
                        format!(
                            "generated {} is not valid Rust: {e}",
                            path.file_name().unwrap().to_string_lossy()
                        )
                    })?;
                }
            }
            Ok(())
        }
    }
}

fn expect_validation_err(schema: &str, needle: &str) {
    let err = ferriorm_parser::parse_and_validate(schema)
        .err()
        .unwrap_or_else(|| {
            panic!(
                "expected validation failure containing {needle:?}, but parse_and_validate succeeded for schema:\n{schema}"
            )
        });
    let lc = format!("{err}").to_lowercase();
    assert!(
        lc.contains(&needle.to_lowercase()),
        "validation error did not mention {needle:?}\n  got: {err}\n  schema:\n{schema}"
    );
}

// ─── B1: Rust keywords as field names ───────────────────────────────

/// A schema with a field named after a Rust keyword (`type`, `match`,
/// `loop`, ...) must either be rejected by the validator or be
/// successfully code-generated into valid Rust (with raw-identifier
/// escaping). It must NEVER panic in codegen — that is what users see.
///
/// Today, codegen calls `format_ident!("{}", name)` directly in
/// `crates/ferriorm-codegen/src/model.rs:65,100,274,...` which panics
/// for reserved keywords. **Expected to fail today.**
#[test]
fn rust_keyword_as_field_name_table_driven() {
    let keywords = [
        "type", "match", "loop", "fn", "async", "await", "yield", "impl", "self",
    ];
    let mut failures: Vec<(String, String)> = Vec::new();

    for kw in keywords {
        let schema = format!(
            r#"
datasource db {{ provider = "sqlite" url = "sqlite::memory:" }}
model A {{
  id  String @id
  {kw} String
}}
"#
        );
        if let Err(e) = schema_either_rejects_or_codegens_cleanly(&schema) {
            failures.push((kw.to_string(), e));
        }
    }

    assert!(
        failures.is_empty(),
        "Rust-keyword field names must be rejected at validation OR escape cleanly in codegen. \
         Failures: {failures:#?}"
    );
}

// ─── B2: @id on optional field ──────────────────────────────────────

/// `id String? @id` is a contradiction (a primary key column cannot be
/// NULL). Validator should reject. **Expected to fail today.**
#[test]
fn id_on_optional_field_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String? @id
}
"#;
    expect_validation_err(schema, "optional");
}

// ─── B3: @id with autoincrement on String ───────────────────────────

/// `@default(autoincrement())` only applies to integer PKs. Pairing it
/// with `String` is a type error the validator should catch.
/// **Expected to fail today.**
#[test]
fn id_with_autoincrement_on_string_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String @id @default(autoincrement())
}
"#;
    expect_validation_err(schema, "autoincrement");
}

// ─── B4: block attribute referencing unknown field ──────────────────

/// `@@index([ghost])` and `@@unique([ghost])` reference fields that
/// don't exist on the model. Validator should reject; today it accepts
/// silently and the migration emits a CREATE INDEX naming a column
/// that doesn't exist. **Expected to fail today.**
#[test]
fn index_references_unknown_field_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String @id
  @@index([ghost])
}
"#;
    expect_validation_err(schema, "ghost");
}

#[test]
fn unique_references_unknown_field_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String @id
  @@unique([ghost])
}
"#;
    expect_validation_err(schema, "ghost");
}

// ─── B5: relation fields/references length mismatch ─────────────────

/// `@relation(fields: [a, b], references: [id])` has 2 source columns
/// but 1 referenced column. This produces an FK that no database can
/// honor; validator should reject. **Expected to fail today.**
#[test]
fn relation_fields_references_length_mismatch_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model U {
  id String @id
}
model P {
  id       String @id
  authorId String
  extraId  String
  author   U      @relation(fields: [authorId, extraId], references: [id])
}
"#;
    expect_validation_err(schema, "length");
}

// ─── B6: duplicate db_name via @@map ────────────────────────────────

/// Two models mapping to the same table name produces conflicting
/// CREATE TABLE statements. Validator should reject the duplicate.
/// **Expected to fail today.**
#[test]
fn duplicate_db_name_via_at_map_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  id String @id
  @@map("shared")
}
model B {
  id String @id
  @@map("shared")
}
"#;
    expect_validation_err(schema, "shared");
}

// ─── B7: composite PK including a Json field ────────────────────────

/// JSON columns aren't comparable / hashable in most databases and
/// can't form part of a primary key. Validator should reject.
/// **Expected to fail today.**
#[test]
fn composite_pk_with_json_field_rejected() {
    let schema = r#"
datasource db { provider = "sqlite" url = "sqlite::memory:" }
model A {
  a    String
  data Json
  @@id([a, data])
}
"#;
    expect_validation_err(schema, "json");
}
