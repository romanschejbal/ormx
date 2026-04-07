# Schema Overview

The `.ferriorm` schema file is the central configuration for your ferriorm project. It serves as the single source of truth for your database structure, describing your data models, their fields, relationships, and how they map to both your Rust code and the underlying database.

When you run the ferriorm code generator, it reads this schema file and produces type-safe Rust structs, query builders, and migration SQL -- all derived from your schema definition.

## File Format

A `.ferriorm` schema file is a plain-text file (by convention named `schema.ferriorm`) that uses a declarative, block-based syntax. The syntax is intentionally similar to Prisma so that developers familiar with that ecosystem can adopt ferriorm with minimal friction.

## Top-Level Blocks

A schema file consists of four kinds of top-level blocks:

### 1. Datasource

Configures which database you are connecting to. Exactly one `datasource` block is required.

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}
```

See [Datasource](./datasource.md) for full details.

### 2. Generator

Controls where and how the generated Rust code is emitted. You may define one or more `generator` blocks.

```prisma
generator client {
  output = "./src/generated"
}
```

See [Generator](./generator.md) for full details.

### 3. Enum

Defines a set of named constants that can be used as field types in your models.

```prisma
enum Role {
  User
  Admin
  Moderator
}
```

See [Enums](./enums.md) for full details.

### 4. Model

Describes a database table and its columns, including relationships to other models.

```prisma
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
```

See [Models](./models.md), [Fields & Types](./fields-and-types.md), [Attributes](./attributes.md), and [Relations](./relations.md) for full details.

## Comments

Line comments start with `//` and continue to the end of the line:

```prisma
// This is a comment
model User {
  id String @id  // inline comment
}
```

## Whitespace

Whitespace (spaces and tabs) is insignificant except as a separator between tokens. Newlines separate fields and block attributes within model and enum bodies.

## Complete Example

Below is a full schema file showing all block types working together:

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}

generator client {
  output = "./src/generated"
}

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
  id        String   @id @default(uuid())
  email     String   @unique
  name      String?
  role      Role     @default(User)
  posts     Post[]
  profile   Profile?
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@index([email])
  @@map("users")
}

model Profile {
  id     String  @id @default(uuid())
  bio    String?
  avatar String?
  user   User    @relation(fields: [userId], references: [id])
  userId String  @unique

  @@map("profiles")
}

model Post {
  id        String     @id @default(uuid())
  title     String
  content   String?
  published Boolean    @default(false)
  status    PostStatus @default(Draft)
  author    User       @relation(fields: [authorId], references: [id])
  authorId  String
  createdAt DateTime   @default(now())
  updatedAt DateTime   @updatedAt

  @@index([authorId])
  @@map("posts")
}
```
