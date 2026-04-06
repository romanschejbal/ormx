# ormx - Prisma-Inspired ORM for Rust

## Architecture

6-crate workspace following onion architecture (dependency flows inward):

- **ormx-core**: Pure domain types (AST, Schema IR, ScalarType, DatabaseProvider). ZERO external dependencies (optional `serde` behind feature flag). Foundation crate -- all others depend on it.
- **ormx-parser**: PEG grammar (pest) to parse `.ormx` schema files into raw AST (`ast::SchemaFile`), then validates and resolves into Schema IR (`schema::Schema`). Entry point: `parse_and_validate()`.
- **ormx-codegen**: Generates type-safe Rust source files from Schema IR using `quote` + `prettyplease`. Produces model structs, filter/data/order submodules, relation types, CRUD query builders, and the `OrmxClient` entry point. Entry point: `generator::generate()`.
- **ormx-runtime**: Ships with user's app. Database client (`DatabaseClient` enum wrapping sqlx pools), filter types (`StringFilter`, `IntFilter`, etc.), parameterized `SqlBuilder`, ordering, transactions, and the `prelude` module.
- **ormx-migrate**: Migration engine. Diffs two Schema IRs to produce `MigrationStep`s, renders them to SQL (PostgreSQL and SQLite renderers), manages migration directories with checksums. Supports two strategies: shadow database (default, accurate) and snapshot (offline, fast). Also handles database introspection for `db pull`.
- **ormx-cli**: CLI binary (`ormx init`, `ormx generate`, `ormx migrate dev/deploy/status`, `ormx db pull`).

## Commands

```bash
cargo test --workspace                                                    # Run all tests
cargo run -p ormx-cli -- --schema path/to/schema.ormx generate            # Generate code
cargo run -p ormx-cli -- --schema path/to/schema.ormx migrate dev --name init  # Create + apply migration
cargo run -p ormx-cli -- --schema path/to/schema.ormx migrate deploy      # Apply pending migrations
cargo run -p ormx-cli -- --schema path/to/schema.ormx migrate status      # Show migration status
cargo run -p ormx-cli -- --schema path/to/schema.ormx db pull             # Introspect DB into schema
```

## Key patterns

- All dependencies are defined at workspace level in root `Cargo.toml`
- Database backends (postgres, sqlite) are feature-flagged in ormx-runtime and ormx-migrate
- Generated code lives in user's project (e.g., `src/generated/`)
- Schema files use `.ormx` extension
- `examples/basic/` is excluded from the workspace and has its own Cargo.toml
- The `ormx-core` crate has a `serde` feature flag for JSON snapshot serialization (used by ormx-migrate)
- Migration files live in `migrations/` directories, each containing `migration.sql` and `_schema_snapshot.json`
- The `_ormx_migrations` table in the user's database tracks applied migrations with SHA-256 checksums
- Code generation uses `proc_macro2::TokenStream` (not actual proc macros) fed through `syn` + `prettyplease` for formatting
- Relations use batched loading (`SELECT ... WHERE fk IN (...)`) to avoid N+1 queries
