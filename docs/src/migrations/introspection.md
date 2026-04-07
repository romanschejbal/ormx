# Database Introspection

The `ferriorm db pull` command reverse-engineers a `schema.ferriorm` file from an existing database. This is the primary tool for adopting ferriorm in projects with an existing database.

## Basic Usage

```bash
DATABASE_URL="postgres://localhost/mydb" ferriorm db pull
```

This command:

1. Connects to the database specified by `DATABASE_URL`.
2. Reads the database's tables, columns, indexes, and foreign keys.
3. Generates (or overwrites) a `schema.ferriorm` file representing the current database state.

## Brownfield Adoption Workflow

If you have an existing database and want to start using ferriorm:

```bash
# 1. Pull the current schema from your database
ferriorm db pull

# 2. Review the generated schema.ferriorm
cat schema.ferriorm

# 3. Generate the Rust client
ferriorm generate

# 4. Create a baseline migration (marks current state as "already applied")
ferriorm migrate dev --name baseline

# 5. Start making changes to schema.ferriorm and migrating normally
```

## Backup

If a `schema.ferriorm` file already exists, `db pull` backs it up before overwriting. Look for a `.backup` file in the same directory if you need to recover the previous version.

## What Gets Introspected

| Database feature | Introspected? |
|---|---|
| Tables | Yes |
| Columns and types | Yes |
| Primary keys | Yes |
| Unique constraints | Yes |
| Foreign keys (relations) | Yes |
| Indexes | Yes |
| Default values | Yes |
| Enums (PostgreSQL) | Yes |
| Views | No |
| Triggers | No |
| Stored procedures | No |

## Type Mapping

Introspection maps database-native types to ferriorm schema types:

### PostgreSQL

| PostgreSQL type | Schema type |
|---|---|
| `text`, `varchar`, `char` | `String` |
| `integer`, `int4` | `Int` |
| `bigint`, `int8` | `BigInt` |
| `real`, `double precision` | `Float` |
| `boolean` | `Boolean` |
| `timestamp`, `timestamptz` | `DateTime` |
| `uuid` | `String` |
| User-defined enums | `enum` |

### SQLite

| SQLite type | Schema type |
|---|---|
| `TEXT` | `String` |
| `INTEGER` | `Int` |
| `REAL` | `Float` |
| `BOOLEAN`, `TINYINT(1)` | `Boolean` |
| `DATETIME`, `TIMESTAMP` | `DateTime` |

## Schema File Location

By default, `db pull` writes to `schema.ferriorm` in the current directory. Use `--schema` to specify a different path:

```bash
ferriorm db pull --schema path/to/schema.ferriorm
```

## Example Output

Given a PostgreSQL database with `users` and `posts` tables, `db pull` might generate:

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  output = "./src/generated"
}

model User {
  id        String   @id @default(uuid())
  email     String   @unique
  name      String?
  role      Role     @default(User)
  posts     Post[]
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@index([email])
  @@map("users")
}

model Post {
  id       String @id @default(uuid())
  title    String
  content  String?
  author   User   @relation(fields: [authorId], references: [id])
  authorId String

  @@index([authorId])
  @@map("posts")
}

enum Role {
  User
  Admin
  Moderator
}
```

> **Tip:** After introspection, review the generated schema and adjust field names, relation names, and model names to match your project's conventions. Then run `ferriorm generate` to produce the Rust client.
