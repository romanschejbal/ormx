# Architecture

Ferriorm is organized as a Cargo workspace with six crates, following a layered architecture where each crate has a clear responsibility and minimal coupling.

## Crate Structure

```
ferriorm/
  crates/
    ferriorm-parser/       # Schema file parser
    ferriorm-core/         # Shared types and schema IR
    ferriorm-codegen/      # Rust code generator
    ferriorm-runtime/      # Runtime library (ships with your app)
    ferriorm-migrate/      # Migration engine
    ferriorm-cli/          # CLI binary
```

## Dependency Flow

```
                  ferriorm-cli
                 /      |      \
                v       v       v
    ferriorm-codegen  ferriorm-migrate
          |              |
          v              v
       ferriorm-core  ferriorm-core
          |              |
          v              v
      ferriorm-parser  ferriorm-parser
```

```
  [Your Application]
         |
         v
  [Generated Code]  --->  ferriorm-runtime
                              |
                              v
                            sqlx
```

## Crate Responsibilities

### ferriorm-parser

Parses `.ferriorm` schema files into an unvalidated AST. Handles the Prisma-like DSL syntax including datasource blocks, generator blocks, models, fields, attributes, and enums.

### ferriorm-core

Defines the validated intermediate representation (IR) of a schema. Transforms the raw AST from the parser into structured types: `Schema`, `Model`, `Field`, `Relation`, `Enum`, etc. This IR is consumed by both the code generator and the migration engine.

### ferriorm-codegen

Takes a `Schema` IR and generates Rust source files:
- Model structs with `sqlx::FromRow` derive
- Filter structs (`WhereInput`, `WhereUniqueInput`)
- Data structs (`CreateInput`, `UpdateInput`)
- Order enums (`OrderByInput`)
- Query builder structs (find, create, update, delete, aggregate)
- Select/Include types and relation loaders
- The `FerriormClient` wrapper
- Enum definitions

### ferriorm-runtime

The only crate that ships as a dependency of your application. Provides:
- `DatabaseClient` -- connection pool wrapper (PostgreSQL + SQLite)
- `PoolConfig` -- pool tuning options
- Filter types (`StringFilter`, `IntFilter`, `BoolFilter`, `DateTimeFilter`, `EnumFilter`, etc.)
- `SetValue<T>` -- update operation wrapper
- `SortOrder` -- ordering enum
- `run_transaction` / `TransactionClient` -- transaction support
- `FerriormError` -- unified error type
- Raw SQL execution helpers

### ferriorm-migrate

Manages database migrations:
- **Shadow database** strategy: creates a temp DB, replays migrations, introspects, diffs
- **Snapshot** strategy: local file-based schema diffing
- **SQL generation**: produces DDL for PostgreSQL and SQLite
- **Runner**: applies migrations, tracks state in `_ferriorm_migrations` table
- **Introspection**: reads database metadata to produce a schema IR

### ferriorm-cli

The `ferriorm` binary. Thin layer over the other crates:
- `init` -- scaffolds a new project
- `generate` -- calls parser + core + codegen
- `migrate dev` -- calls parser + core + migrate + codegen
- `migrate deploy` -- calls migrate (runner only)
- `migrate status` -- calls migrate (state reader)
- `db pull` -- calls migrate (introspection) + writes schema file

## Design Principles

**Code generation over reflection.** All query builders and type mappings are generated at build time. There are no runtime macros, trait-based query DSLs, or dynamic dispatch for query construction. The generated code is plain Rust that uses sqlx's `QueryBuilder` directly.

**sqlx as the foundation.** Ferriorm does not implement its own database driver or connection pool. It generates code that uses sqlx's `QueryBuilder`, `FromRow`, and pool types. You can always drop down to raw sqlx when needed.

**Schema as the source of truth.** The `schema.ferriorm` file is the single source of truth for your data model. Migrations, generated code, and type mappings are all derived from it.

For implementation details, see the source code in each crate's `src/` directory.
