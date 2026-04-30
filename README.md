# ferriorm

**A Prisma-inspired ORM for Rust** -- schema-first, type-safe, with automatic code generation.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](#license)
[![CI](https://github.com/romanschejbal/ferriorm/actions/workflows/ci.yml/badge.svg)](https://github.com/romanschejbal/ferriorm/actions/workflows/ci.yml)

## Why ferriorm?

Existing Rust ORMs require manually defining structs, writing migrations, and wiring everything together. ferriorm takes the Prisma approach: **define your schema once**, and everything else -- type-safe Rust client, migrations, query builders -- is generated for you.

- **Schema-first**: Single `.ferriorm` file is your source of truth
- **Type-safe**: Generated Rust code catches errors at compile time
- **Zero boilerplate**: No derive macros to write, no manual struct definitions
- **Easy migrations**: Automatic schema diffing with shadow database support
- **Multi-database**: PostgreSQL and SQLite from day one

## Documentation

Full user guide, schema reference, client API, and migration workflows live
in the **[ferriorm book](https://romanschejbal.github.io/ferriorm/)**.

Quick links:

- [Installation](https://romanschejbal.github.io/ferriorm/getting-started/installation.html)
- [Quick Start](https://romanschejbal.github.io/ferriorm/getting-started/quick-start.html)
- [Schema Reference](https://romanschejbal.github.io/ferriorm/schema/overview.html)
- [Client API](https://romanschejbal.github.io/ferriorm/client/connecting.html)
- [Migrations](https://romanschejbal.github.io/ferriorm/migrations/overview.html)

```bash
cargo install ferriorm-cli
ferriorm init --provider postgresql
ferriorm migrate dev --name init
```

## Features

### Schema Language
- Models with scalar fields, enums, relations
- Field attributes: `@id`, `@unique`, `@default`, `@updatedAt`, `@map`, `@relation`, `@db.*`
- Block attributes: `@@map`, `@@index`, `@@unique`, `@@id`
- Optional `@relation("Name", ...)` to disambiguate multiple relations between the same two models
- Optional `@@index([..], name: "...")` and `@@unique([..], name: "...")` (also `map:` as alias) to override the auto-generated database identifier
- `@@map` on enums to override the database type name (Postgres only)
- Datasource and generator configuration

### Generated Client
- Full CRUD: `create`, `find_unique`, `find_first`, `find_many`, `update`, `delete`, `upsert`
- Dedup-on-write: `create().on_conflict_ignore()` → `Option<T>`
- Race-safe updates: `update_first(WhereInput, data)` for compare-and-swap transitions
- Batch operations: `create_many`, `update_many`, `delete_many`
- Type-safe filters: `equals`, `not`, `contains`, `starts_with`, `gt`, `lt`, `in`, `AND`, `OR`, `NOT`
- Nullable filters with `IS NULL` / `IS NOT NULL` for every scalar type
- Compound `@@unique([...])` keys materialize as `WhereUniqueInput` variants (usable by `upsert` as `ON CONFLICT` targets)
- Ordering, pagination (`skip`/`take`), counting
- Aggregates: `aggregate()` (avg/sum/min/max), and `group_by()` with bucketed `count`/`avg`/`sum`/`min`/`max` and a typed `having()` filter on the aggregate results
- Relation loading via `include()` with batched queries (no N+1)

### Migrations
- **Shadow database** (default): Replays migrations on a temp DB, introspects, diffs -- handles manual SQL edits correctly
- **Snapshot mode** (`--snapshot`): Offline diffing via JSON snapshots
- Commands: `migrate dev`, `migrate deploy`, `migrate status`
- Editable SQL migration files with checksum verification

### Database Support
| Feature | PostgreSQL | SQLite |
|---------|-----------|--------|
| Query execution | yes | yes |
| Code generation | yes | yes |
| Migrations | yes | yes |
| Shadow database | yes | yes |
| Introspection | yes | yes |
| `db pull` | yes | yes |

## Architecture

ferriorm is a Rust workspace with 6 crates following onion architecture:

```
ferriorm-cli          -> orchestrates everything
|-- ferriorm-parser   -> parses .ferriorm schema files
|-- ferriorm-codegen  -> generates Rust source code
|-- ferriorm-migrate  -> migration engine
\-- ferriorm-runtime  -> ships with user's app (DB client, filters, queries)
      \-- ferriorm-core -> pure domain types (zero external dependencies)
```

## CLI Reference

| Command | Description |
|---------|-------------|
| `ferriorm init` | Initialize a new ferriorm project |
| `ferriorm generate` | Generate Rust client from schema |
| `ferriorm migrate dev` | Create + apply migration + regenerate (development) |
| `ferriorm migrate deploy` | Apply pending migrations (production) |
| `ferriorm migrate status` | Show migration status |
| `ferriorm db pull` | Introspect database and generate schema |

## Status

ferriorm is in active development. Here is what's done and what's planned:

### Done
- [x] Schema parser with PEG grammar
- [x] Code generator (models, enums, filters, CRUD, query builders)
- [x] Runtime with PostgreSQL and SQLite support
- [x] All CRUD operations with exec()
- [x] Type-safe filters with AND/OR/NOT
- [x] Ordering and pagination
- [x] Relation loading with batched queries (include API)
- [x] Migration engine with schema diffing
- [x] Shadow database migration strategy
- [x] Database introspection and `db pull`
- [x] PostgreSQL support
- [x] SQLite support
- [x] Connection pooling configuration
- [x] Raw SQL escape hatch (pool access + zero-bind helpers)
- [x] `select()` for partial column loading
- [x] Aggregate queries (avg, sum, min, max)
- [x] Aggregate `group_by` (with `HAVING`)

### Planned
- [ ] Compile-time query verification (hybrid sqlx approach)
- [ ] MySQL support
- [ ] Middleware/hooks (beforeCreate, afterUpdate)
- [ ] Soft deletes
- [ ] Cursor-based pagination
- [ ] Schema formatting (`ferriorm format`)
- [ ] LSP for .ferriorm files
- [ ] Seeding support

## License

MIT OR Apache-2.0
