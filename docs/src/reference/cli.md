# CLI Reference

The `ferriorm` CLI manages code generation, migrations, and database introspection.

## Global Options

| Flag | Default | Description |
|---|---|---|
| `--schema <path>` | `schema.ferriorm` | Path to the schema file |
| `--version` | | Print version |
| `--help` | | Print help |

## Commands

### `ferriorm init`

Initialize a new ferriorm project. Creates a `schema.ferriorm` file with a datasource and generator block.

```bash
ferriorm init
ferriorm init --provider sqlite
```

| Flag | Default | Description |
|---|---|---|
| `--provider <provider>` | `postgresql` | Database provider: `postgresql` or `sqlite` |

**Generated files:**
- `schema.ferriorm` -- starter schema with datasource configuration

---

### `ferriorm generate`

Generate the Rust client code from the schema file. Reads `schema.ferriorm` and writes generated modules to the output directory specified in the `generator` block.

```bash
ferriorm generate
ferriorm generate --schema path/to/schema.ferriorm
```

| Flag | Default | Description |
|---|---|---|
| `--schema <path>` | `schema.ferriorm` | Path to the schema file |

**Generated files:**
- `mod.rs` -- module declarations and re-exports
- `client.rs` -- `FerriormClient` with model accessors
- `enums.rs` -- Rust enums for schema enums
- `<model>.rs` -- one file per model with struct, filters, CRUD builders, etc.

---

### `ferriorm migrate dev`

Create a new migration, apply it to the development database, and regenerate the client. This is the primary command during development.

```bash
ferriorm migrate dev --name init
ferriorm migrate dev --name add_posts --snapshot
```

| Flag | Default | Description |
|---|---|---|
| `--name <name>` | (auto) | Migration name (used in the directory name) |
| `--snapshot` | `false` | Use snapshot strategy instead of shadow database |
| `--schema <path>` | `schema.ferriorm` | Path to the schema file |

**What it does:**
1. Diffs the current database state (or snapshot) against `schema.ferriorm`
2. Generates `migrations/<timestamp>_<name>/migration.sql`
3. Applies the migration to the database
4. Regenerates the Rust client

**Environment variables:**
- `DATABASE_URL` -- required (unless using `--snapshot` with SQLite)

---

### `ferriorm migrate deploy`

Apply all pending migrations to the database. Used in production and CI/CD pipelines. Never generates new migrations.

```bash
ferriorm migrate deploy
```

| Flag | Default | Description |
|---|---|---|
| `--schema <path>` | `schema.ferriorm` | Path to the schema file |

**What it does:**
1. Reads the `_ferriorm_migrations` table
2. Verifies checksums of applied migrations
3. Applies pending migrations in order
4. Records each applied migration

**Environment variables:**
- `DATABASE_URL` -- required

---

### `ferriorm migrate status`

Show the status of all migrations: which are applied, which are pending, and whether any checksums are mismatched.

```bash
ferriorm migrate status
```

| Flag | Default | Description |
|---|---|---|
| `--schema <path>` | `schema.ferriorm` | Path to the schema file |

**Environment variables:**
- `DATABASE_URL` -- required

---

### `ferriorm db pull`

Introspect an existing database and generate a `schema.ferriorm` file from its current state. Used for brownfield adoption.

```bash
ferriorm db pull
```

| Flag | Default | Description |
|---|---|---|
| `--schema <path>` | `schema.ferriorm` | Path to write the schema file |

**What it does:**
1. Connects to the database
2. Reads tables, columns, indexes, foreign keys, and enums
3. Writes a `schema.ferriorm` file (backs up existing file if present)

**Environment variables:**
- `DATABASE_URL` -- required

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `DATABASE_URL` | For most commands | Database connection URL |
| `RUST_LOG` | No | Controls log verbosity (e.g., `RUST_LOG=debug`) |

## Exit Codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | Error (migration failure, connection error, parse error, etc.) |
