# Migrations Overview

Migrations track incremental changes to your database schema over time. Each migration is a SQL file that transforms the database from one state to the next.

## Why Migrations?

Your `schema.ferriorm` file describes the **desired** state of the database. Migrations bridge the gap between the current database state and the desired state by generating the SQL `ALTER TABLE`, `CREATE TABLE`, and other DDL statements needed.

## Migration Strategies

Ferriorm supports two strategies for generating migration SQL:

### Shadow Database (Default)

1. Creates a temporary "shadow" database.
2. Replays all existing migrations against it.
3. Introspects the shadow database to get the "current" schema.
4. Diffs the current schema against your `schema.ferriorm` to produce the migration SQL.
5. Drops the shadow database.

This is the most reliable strategy and catches issues like migration drift. It requires a running database server with permissions to create/drop databases.

### Snapshot

1. Reads a local `.snapshot` file representing the last-known schema state.
2. Diffs the snapshot against your `schema.ferriorm`.
3. Writes the new migration SQL and updates the snapshot.

Use snapshot mode with `--snapshot` when you cannot connect to a database (CI, offline development, SQLite). No temporary database is created.

## Directory Structure

Migrations live in a `migrations/` directory next to your schema file:

```
project/
  schema.ferriorm
  migrations/
    20250315120000_init/
      migration.sql
    20250320143000_add_posts/
      migration.sql
    migration_lock.toml
```

- Each migration is a timestamped directory containing a `migration.sql` file.
- `migration_lock.toml` records the database provider to prevent accidentally running PostgreSQL migrations against SQLite.

## Migration SQL

Each `migration.sql` file contains raw SQL statements:

```sql
-- CreateTable
CREATE TABLE "users" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "email" TEXT NOT NULL,
    "name" TEXT,
    "role" TEXT NOT NULL DEFAULT 'user',
    "created_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updated_at" TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- CreateIndex
CREATE UNIQUE INDEX "users_email_key" ON "users"("email");
```

You can edit migration files after generation but before applying them.

## Checksum Verification

Every applied migration is recorded in a `_ferriorm_migrations` table in your database with a SHA-256 checksum of its SQL content. On subsequent runs, ferriorm verifies that previously applied migrations have not been modified. If a checksum mismatch is detected, the command fails with an error.

## Workflow Summary

| Stage | Command | Strategy |
|---|---|---|
| Development | `ferriorm migrate dev` | Shadow DB or snapshot |
| Production | `ferriorm migrate deploy` | Apply pending only |
| Status check | `ferriorm migrate status` | Read-only |
| Brownfield | `ferriorm db pull` | Introspection |

See the following pages for details on each stage.
