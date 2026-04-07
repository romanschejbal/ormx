# Project Structure

After running `ferriorm init` and `ferriorm migrate dev`, your project will have the following layout:

```
my-project/
├── Cargo.toml
├── schema.ferriorm        ← your schema
├── migrations/            ← auto-generated SQL
│   └── 0001_init/
│       ├── migration.sql
│       └── _schema_snapshot.json
└── src/
    ├── main.rs
    └── generated/         ← auto-generated Rust code
        ├── mod.rs
        ├── client.rs
        ├── user.rs
        └── enums.rs
```

## schema.ferriorm

The schema file is the single source of truth for your data model. It defines:

- **Datasource** -- which database provider to use and how to connect
- **Generator** -- where to output the generated Rust code
- **Models** -- your tables, fields, relations, and indexes
- **Enums** -- shared enum types used across models

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  output = "./src/generated"
}

model User {
  id    String @id @default(uuid())
  email String @unique
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

You edit this file, then run `ferriorm migrate dev` to sync everything.

## migrations/

Each migration lives in a numbered directory. ferriorm creates these automatically when you run `ferriorm migrate dev --name <name>`.

```
migrations/
├── 0001_init/
│   ├── migration.sql           ← the SQL that was applied
│   └── _schema_snapshot.json   ← schema state after this migration
└── 0002_add_posts/
    ├── migration.sql
    └── _schema_snapshot.json
```

- **migration.sql** -- The SQL statements that create or alter tables. You can review and edit these before applying. ferriorm tracks checksums to detect manual edits.
- **_schema_snapshot.json** -- A JSON snapshot of the schema at this point in time. Used by the snapshot migration strategy for offline diffing.

> **Tip:** Commit the entire `migrations/` directory to version control. These files are how your teammates and production environments apply the same schema changes.

## src/generated/

This directory contains the Rust code that ferriorm generates from your schema. **Do not edit these files** -- they are overwritten every time you run `ferriorm generate` or `ferriorm migrate dev`.

| File | Contents |
|------|----------|
| `mod.rs` | Re-exports the client and all model/enum modules |
| `client.rs` | The `FerriormClient` struct with `connect()`, `disconnect()`, and accessor methods for each model |
| `user.rs` | `User` struct, `UserCreateInput`, `UserUpdateInput`, `UserWhereInput`, `UserWhereUniqueInput`, `UserOrderByInput`, `UserInclude`, and the `UserActions` query builder |
| `post.rs` | Same pattern for the `Post` model |
| `enums.rs` | Rust enums corresponding to any `enum` blocks in your schema |

Each model file follows the same structure:

```rust
// In generated/user.rs (simplified)

pub struct User {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    // ...
}

pub mod data {
    pub struct UserCreateInput { /* required + optional fields */ }
    pub struct UserUpdateInput { /* all fields optional */ }
}

pub mod filter {
    pub struct UserWhereInput { /* filterable fields */ }
    pub enum UserWhereUniqueInput { /* unique fields */ }
}

pub mod order {
    pub enum UserOrderByInput { /* orderable fields */ }
}
```

To use the generated code, add `mod generated;` at the top of your `main.rs` (or `lib.rs`).

## Typical workflow

1. Edit `schema.ferriorm`
2. Run `ferriorm migrate dev --name describe_the_change`
3. The CLI diffs, generates a migration, applies it, and regenerates `src/generated/`
4. Your Rust code gets compile-time errors if anything changed -- fix and continue

## Next steps

- See the full [Schema Reference](../schema/overview.md) for all available model attributes and field types
- Learn about [CRUD Operations](../client/crud.md) available on the generated client
