# Introduction

ferriorm is a **schema-first ORM for Rust**, inspired by [Prisma](https://www.prisma.io/). You define your data model once in a `.ferriorm` schema file, and ferriorm generates type-safe Rust code -- structs, query builders, filters, and a database client -- so you never write boilerplate by hand.

## Why ferriorm?

Most Rust ORMs require you to manually define structs, derive traits, write migrations, and wire it all together. ferriorm flips that around:

- **Schema-first** -- A single `.ferriorm` file is your source of truth for models, relations, and database mapping.
- **Type-safe code generation** -- The generated Rust client catches errors at compile time. No runtime surprises.
- **Zero boilerplate** -- No derive macros to remember, no manual struct definitions, no hand-written SQL for basic CRUD.
- **Automatic migrations** -- ferriorm diffs your schema against the database and generates SQL migrations for you.
- **Multi-database** -- PostgreSQL and SQLite are supported out of the box.

## How it works

The workflow has three steps: **define**, **generate**, **use**.

**1. Define your schema**

```prisma
// schema.ferriorm
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
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@map("users")
}
```

**2. Generate and migrate**

```bash
ferriorm migrate dev --name init
```

This creates a SQL migration, applies it to your database, and generates the Rust client code into `src/generated/`.

**3. Use the generated client**

```rust
mod generated;

use generated::FerriormClient;
use ferriorm_runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = FerriormClient::connect("postgres://localhost/mydb").await?;

    // Create a user
    let user = client.user().create(generated::user::data::UserCreateInput {
        email: "alice@example.com".into(),
        name: Some("Alice".into()),
        id: None,
        created_at: None,
    }).exec().await?;

    println!("Created user: {} (id={})", user.email, user.id);

    // Find users by filter
    let users = client.user()
        .find_many(generated::user::filter::UserWhereInput {
            email: Some(StringFilter {
                contains: Some("@example.com".into()),
                ..Default::default()
            }),
            ..Default::default()
        })
        .order_by(generated::user::order::UserOrderByInput::CreatedAt(SortOrder::Desc))
        .take(10)
        .exec().await?;

    println!("Found {} users", users.len());
    Ok(())
}
```

Every query is fully typed. If you rename a field in your schema and regenerate, the Rust compiler tells you exactly where your code needs to change.

## What's next?

Head to [Installation](./getting-started/installation.md) to set up ferriorm, then follow the [Quick Start](./getting-started/quick-start.md) tutorial to build your first project.
