# ferriorm - Prisma-Inspired ORM for Rust

## Architecture

6-crate workspace following onion architecture (dependency flows inward):

- **ferriorm-core**: Pure domain types (AST, Schema IR, ScalarType, DatabaseProvider). ZERO external dependencies (optional `serde` behind feature flag). Foundation crate -- all others depend on it. `Index`/`UniqueConstraint`/`ResolvedRelation` carry an optional `name`.
- **ferriorm-parser**: PEG grammar (pest) to parse `.ferriorm` schema files into raw AST (`ast::SchemaFile`), then validates and resolves into Schema IR (`schema::Schema`). Entry point: `parse_and_validate()`. The validator screens for: missing PK, unknown types, duplicate model/enum names, duplicate `@@map` table names, Rust-keyword field names, `@id` on optional fields, `autoincrement()` on non-integer fields, `@@index`/`@@unique`/`@@id` referencing unknown fields, `@relation` `fields`/`references` length mismatch, `Json` in composite PK, and multi-relation disambiguation by `@relation("Name", ...)`.
- **ferriorm-codegen**: Generates type-safe Rust source files from Schema IR using `quote` + `prettyplease`. Produces model structs, filter/data/order submodules, relation types, CRUD query builders, and the `FerriormClient` entry point. Entry point: `generator::generate()`. LIKE filters route through `ferriorm_runtime::filter::like_escape` and emit `LIKE ? ESCAPE '\\'`. Relation back-references are paired by `name` when set.
- **ferriorm-runtime**: Ships with user's app. Database client (`DatabaseClient` enum wrapping sqlx pools), filter types (`StringFilter`, `IntFilter`, etc.), `like_escape()` helper, parameterized `SqlBuilder`, ordering, transactions, and the `prelude` module.
- **ferriorm-migrate**: Migration engine. Diffs two Schema IRs to produce `MigrationStep`s, renders them to SQL (PostgreSQL and SQLite renderers), manages migration directories with checksums. Supports two strategies: shadow database (default, accurate) and snapshot (offline, fast). Also handles database introspection for `db pull`. Diff detects: column add/drop/alter (incl. default-value changes), FK shape changes (cascade actions, referenced column), `AlterPrimaryKey` for PK changes on existing tables, `AlterEnumName` for enum `@@map` renames, plus indexes and unique constraints with custom names.
- **ferriorm-cli**: CLI binary (`ferriorm init`, `ferriorm generate`, `ferriorm migrate dev/deploy/status`, `ferriorm db pull`).

## Commands

```bash
cargo test --workspace                                                    # Run all tests
cargo run -p ferriorm-cli -- --schema path/to/schema.ferriorm generate            # Generate code
cargo run -p ferriorm-cli -- --schema path/to/schema.ferriorm migrate dev --name init  # Create + apply migration
cargo run -p ferriorm-cli -- --schema path/to/schema.ferriorm migrate deploy      # Apply pending migrations
cargo run -p ferriorm-cli -- --schema path/to/schema.ferriorm migrate status      # Show migration status
cargo run -p ferriorm-cli -- --schema path/to/schema.ferriorm db pull             # Introspect DB into schema
```

## Key patterns

- All dependencies are defined at workspace level in root `Cargo.toml`
- Database backends (postgres, sqlite) are feature-flagged in ferriorm-runtime and ferriorm-migrate
- Generated code lives in user's project (e.g., `src/generated/`)
- Schema files use `.ferriorm` extension
- `examples/basic/` is excluded from the workspace and has its own Cargo.toml
- The `ferriorm-core` crate has a `serde` feature flag for JSON snapshot serialization (used by ferriorm-migrate). New optional fields on IR types (e.g. `Index.name`, `UniqueConstraint.name`, `ResolvedRelation.name`) use `#[serde(default, skip_serializing_if = "Option::is_none")]` so older snapshots stay forward-compatible.
- Migration files live in `migrations/` directories, each containing `migration.sql` and `_schema_snapshot.json`
- The `_ferriorm_migrations` table in the user's database tracks applied migrations with SHA-256 checksums
- Code generation uses `proc_macro2::TokenStream` (not actual proc macros) fed through `syn` + `prettyplease` for formatting
- Relations use batched loading (`SELECT ... WHERE fk IN (...)`) to avoid N+1 queries
- Multi-relation pairing is by `name`: when two relations connect the same pair of models, both sides must use `@relation("Name", ...)` and the validator rejects ambiguous schemas
- Index / unique constraint names: `@@index([..], name: "...")` and `@@unique([..], name: "...")` override the auto-generated `idx_<table>_<cols>` / `uq_<table>_<cols>` identifiers (`map:` is accepted as a Prisma-style alias)
- LIKE escape: codegen emits `LIKE ? ESCAPE '\'` and routes user input through `like_escape()` so `%`, `_`, `\` in `contains` / `starts_with` / `ends_with` are matched literally
- `MigrationStep` includes `AlterPrimaryKey { table, from_columns, to_columns }` and `AlterEnumName { from_name, to_name }`; SQLite renders these as comments because in-place ALTER COLUMN isn't supported

## Testing

- `cargo test --workspace` runs everything; e2e tests live in `tests/e2e/tests/*.rs`
- CI gate (matches `.github/workflows/ci.yml`):
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings -W clippy::pedantic`
  - `RUSTDOCFLAGS=-D warnings cargo doc --workspace --no-deps`
  - `cargo check --workspace` on MSRV 1.88.0
- When a planned breakage is intentional, prefer a validator-level rejection over an `#[allow]` or `#[ignore]` so the failure message guides the user
