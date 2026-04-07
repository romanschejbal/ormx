# Development Workflow

The `ferriorm migrate dev` command is the primary tool for evolving your schema during development. It generates a new migration, applies it, and regenerates the Rust client.

## Basic Usage

```bash
# Edit your schema.ferriorm, then run:
ferriorm migrate dev --name add_posts_table
```

This command:

1. Detects schema changes by comparing your `schema.ferriorm` against the current database state.
2. Generates a new `migration.sql` file in `migrations/<timestamp>_<name>/`.
3. Applies the migration to your development database.
4. Regenerates the Rust client code.

## Full Workflow

```bash
# 1. Edit the schema
vim schema.ferriorm

# 2. Generate and apply the migration
ferriorm migrate dev --name add_user_roles

# 3. Your generated client is updated automatically
cargo build
```

## The `--name` Flag

The `--name` flag is optional but recommended. It gives the migration a descriptive name:

```bash
ferriorm migrate dev --name init
# Creates: migrations/20250315120000_init/migration.sql

ferriorm migrate dev --name add_email_index
# Creates: migrations/20250320143000_add_email_index/migration.sql
```

Without `--name`, a default name is used.

## Editing Migrations After Generation

After `migrate dev` generates the SQL, you can edit `migration.sql` before the next deploy. This is useful for:

- Adding data migrations (`INSERT`, `UPDATE`)
- Renaming columns (ferriorm generates drop + add by default)
- Adding database-specific features (triggers, functions)

> **Warning:** Do not edit migrations that have already been applied to other environments. Checksum verification will fail.

## Shadow Database Strategy (Default)

By default, `migrate dev` uses a shadow database:

```
Your DB (current state)
        |
        v
Shadow DB (replay all migrations) --> Introspect --> Diff against schema.ferriorm
        |                                                    |
        v                                                    v
   (dropped)                                        migration.sql
```

**Requirements:**
- A running database server (PostgreSQL or SQLite)
- Permissions to create and drop temporary databases
- The `DATABASE_URL` environment variable must be set

The shadow database is created with a `_shadow` suffix, used temporarily, and dropped automatically.

## Snapshot Strategy

For environments where you cannot create a shadow database, use the `--snapshot` flag:

```bash
ferriorm migrate dev --name add_posts --snapshot
```

This uses a local `.snapshot` file instead of a temporary database. The snapshot records the schema state after the last migration.

**When to use snapshot mode:**
- Offline development without a database connection
- CI environments without database access
- SQLite projects (simpler setup)

**Trade-off:** Snapshot mode cannot detect drift between your migrations and the actual database state. The shadow database strategy is more reliable.

## Checking Migration Status

See which migrations have been applied and which are pending:

```bash
ferriorm migrate status
```

This reads the `_ferriorm_migrations` table in your database and compares it to the `migrations/` directory.

## Example: Adding a Field

Starting schema:

```prisma
model User {
  id    String @id @default(uuid())
  email String @unique
  @@map("users")
}
```

Add a `name` field:

```prisma
model User {
  id    String  @id @default(uuid())
  email String  @unique
  name  String?
  @@map("users")
}
```

Run the migration:

```bash
ferriorm migrate dev --name add_user_name
```

Generated `migration.sql`:

```sql
-- AlterTable
ALTER TABLE "users" ADD COLUMN "name" TEXT;
```

## Example: Adding a Relation

Add a `Post` model related to `User`:

```prisma
model User {
  id    String  @id @default(uuid())
  email String  @unique
  name  String?
  posts Post[]
  @@map("users")
}

model Post {
  id       String @id @default(uuid())
  title    String
  author   User   @relation(fields: [authorId], references: [id])
  authorId String
  @@map("posts")
}
```

```bash
ferriorm migrate dev --name add_posts
```

Generated `migration.sql`:

```sql
-- CreateTable
CREATE TABLE "posts" (
    "id" TEXT NOT NULL PRIMARY KEY,
    "title" TEXT NOT NULL,
    "author_id" TEXT NOT NULL,
    CONSTRAINT "posts_author_id_fkey" FOREIGN KEY ("author_id") REFERENCES "users"("id")
);

-- CreateIndex
CREATE INDEX "posts_author_id_idx" ON "posts"("author_id");
```

## Resetting the Database

If your development database gets into a bad state, you can reset it by dropping and recreating it manually, then running:

```bash
ferriorm migrate dev
```

This replays all migrations from scratch.
