# Enums

An `enum` block defines a set of named constants. Enums can be used as field types in your models, giving you type-safe, exhaustive variants in both your Rust code and your database.

## Syntax

```prisma
enum Role {
  User
  Admin
  Moderator
}
```

## Naming

- The enum name must be **PascalCase** (e.g., `Role`, `PostStatus`).
- Variant names must be **PascalCase** (e.g., `User`, `Admin`, `Draft`, `Published`).

## Using Enums in Models

Reference an enum by name as a field type, just like a scalar type:

```prisma
enum PostStatus {
  Draft
  Published
  Archived
}

model Post {
  id     String     @id @default(uuid())
  status PostStatus @default(Draft)
}
```

The `@default` attribute accepts a bare variant name (without quotes) to set the default value.

## Generated Rust Code

Each enum in the schema generates a Rust enum with the following derives:

```rust
#[derive(Debug, Clone, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
pub enum Role {
    User,
    Admin,
    Moderator,
}
```

The `sqlx::Type` derive enables the enum to be read from and written to the database directly via sqlx. `Serialize` and `Deserialize` (from serde) enable JSON serialization.

## Database Representation

### PostgreSQL

In PostgreSQL, ferriorm creates a **custom enum type** using `CREATE TYPE`:

```sql
CREATE TYPE "Role" AS ENUM ('User', 'Admin', 'Moderator');
```

Columns with this type store the variant as the enum's internal representation, which is compact and enforces that only valid variants are stored.

### SQLite

SQLite does not have native enum types. Enum values are stored as **`TEXT`** -- the variant name is stored as a plain string (e.g., `'Admin'`). Validation happens at the application level through the generated Rust enum.

## Multiple Enums

You can define as many enums as you need:

```prisma
enum Role {
  User
  Admin
  Moderator
}

enum PostStatus {
  Draft
  Published
  Archived
}

model User {
  id   String @id @default(uuid())
  role Role   @default(User)
}

model Post {
  id     String     @id @default(uuid())
  status PostStatus @default(Draft)
}
```

## Optional Enum Fields

Enum fields can be made optional with `?`, just like scalar fields:

```prisma
model User {
  id   String @id
  role Role?
}
```

This generates `Option<Role>` in Rust and allows `NULL` in the database column.
