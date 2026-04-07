# Quick Start

This tutorial walks you through creating a project with ferriorm from scratch. By the end you will have a schema, a database migration, and working Rust code that creates and queries data.

## Prerequisites

- Rust toolchain installed ([rustup.rs](https://rustup.rs/))
- `ferriorm-cli` installed (see [Installation](./installation.md))
- A running PostgreSQL instance (or SQLite -- just adjust the provider)

## 1. Create a new Rust project

```bash
cargo new my-app
cd my-app
```

Add the required dependencies to your `Cargo.toml` (see [Installation](./installation.md) for the full list).

## 2. Initialize ferriorm

```bash
ferriorm init --provider postgresql
```

This creates a `schema.ferriorm` file in your project root with a starter template.

## 3. Define your schema

Open `schema.ferriorm` and replace its contents with:

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
  createdAt DateTime @default(now())
  updatedAt DateTime @updatedAt

  @@map("users")
}
```

This defines a single `User` model mapped to a `users` table. The `id` is an auto-generated UUID, `email` is required and unique, and `name` is optional.

## 4. Set your database URL

Export the connection string for your database:

```bash
# PostgreSQL
export DATABASE_URL="postgres://user:password@localhost:5432/my_app"

# SQLite
export DATABASE_URL="sqlite://./dev.db"
```

Make sure the database exists before continuing. For PostgreSQL:

```bash
createdb my_app
```

## 5. Create and apply the migration

```bash
ferriorm migrate dev --name init
```

This command does three things:

1. **Diffs** your schema against the current database state
2. **Creates** a SQL migration file in `migrations/0001_init/migration.sql`
3. **Applies** the migration and **generates** the Rust client into `src/generated/`

You should see output confirming the migration was applied and the client was generated.

## 6. Write your application

Replace `src/main.rs` with:

```rust
mod generated;

use generated::FerriormClient;
use ferriorm_runtime::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    // Connect to the database
    let client = FerriormClient::connect(&database_url).await?;

    // Create a user
    let user = client.user().create(generated::user::data::UserCreateInput {
        email: "alice@example.com".into(),
        name: Some("Alice".into()),
        id: None,
        created_at: None,
    }).exec().await?;

    println!("Created user: {} (id={})", user.email, user.id);

    // Find the user by unique field
    let found = client.user()
        .find_unique(generated::user::filter::UserWhereUniqueInput::Email(
            "alice@example.com".into(),
        ))
        .exec()
        .await?;

    if let Some(u) = found {
        println!("Found user: {} (name={:?})", u.email, u.name);
    }

    // List all users
    let all_users = client.user()
        .find_many(generated::user::filter::UserWhereInput::default())
        .exec()
        .await?;

    println!("Total users: {}", all_users.len());

    // Update the user
    let updated = client.user()
        .update(
            generated::user::filter::UserWhereUniqueInput::Email(
                "alice@example.com".into(),
            ),
            generated::user::data::UserUpdateInput {
                name: Some(Some("Alice Smith".into())),
                ..Default::default()
            },
        )
        .exec()
        .await?;

    println!("Updated name to: {:?}", updated.name);

    // Delete the user
    client.user()
        .delete(generated::user::filter::UserWhereUniqueInput::Email(
            "alice@example.com".into(),
        ))
        .exec()
        .await?;

    println!("User deleted.");

    // Disconnect
    client.disconnect().await;
    Ok(())
}
```

## 7. Run it

```bash
cargo run
```

You should see output like:

```
Created user: alice@example.com (id=550e8400-e29b-41d4-a716-446655440000)
Found user: alice@example.com (name=Some("Alice"))
Total users: 1
Updated name to: Some("Alice Smith")
User deleted.
```

## What just happened?

You went from an empty project to a working database application without writing a single SQL query or struct definition by hand. ferriorm generated all the types, filters, and query builders from your schema.

## Next steps

- Learn about the [Project Structure](./project-structure.md) ferriorm creates
- Explore the full [Schema Reference](../schema/overview.md) to add relations, enums, and more
- See all available [CRUD Operations](../client/crud.md) in the client API
