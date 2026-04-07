# Relations (Include)

Ferriorm can load related records alongside the primary query using `.include()`. Related records are fetched in a single batched query to avoid the N+1 problem.

## Basic Include

Given this schema:

```prisma
model User {
  id    String @id @default(uuid())
  email String @unique
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

Load users with their posts:

```rust
use generated::user::UserInclude;

let users = client
    .user()
    .find_many(UserWhereInput::default())
    .include(UserInclude {
        posts: true,
        ..Default::default()
    })
    .exec()
    .await?;

for u in &users {
    println!(
        "{} has {} posts",
        u.data.email,
        u.posts.as_ref().map(|p| p.len()).unwrap_or(0)
    );
}
```

## UserInclude Struct

Each model generates an include struct with a `bool` field for every relation:

```rust
#[derive(Debug, Clone, Default)]
pub struct UserInclude {
    pub posts: bool,
    // pub profile: bool, // if the model has a profile relation
}
```

Set a field to `true` to load that relation. Fields left `false` (or defaulted) are not loaded.

## UserWithRelations Struct

When using `.include()`, the return type changes from `Vec<Model>` to `Vec<ModelWithRelations>`:

```rust
pub struct UserWithRelations {
    pub data: User,                      // the base record
    pub posts: Option<Vec<Post>>,        // None if not included
}
```

- `data` contains all scalar fields of the parent record.
- Each relation field is `Option<Vec<RelatedModel>>` for one-to-many relations, and `Option<RelatedModel>` for one-to-one.
- The value is `None` when the relation was not included, and `Some(...)` when it was (even if the list is empty).

## Include with Find Unique

`.include()` also works with `find_unique`:

```rust
let user = client
    .user()
    .find_unique(UserWhereUniqueInput::Email("alice@example.com".into()))
    .include(UserInclude {
        posts: true,
        ..Default::default()
    })
    .exec()
    .await?;

if let Some(u) = user {
    println!("Found {} with {} posts", u.data.email, u.posts.unwrap().len());
}
```

The return type is `Option<UserWithRelations>`.

## How Batched Loading Works

Ferriorm avoids the N+1 query problem by loading relations in two steps:

1. **Primary query**: Fetch all parent records with a single `SELECT`.
2. **Relation query**: Collect all parent IDs, then fetch related records with a single `SELECT ... WHERE foreign_key IN (id1, id2, ...)` query.
3. **Assembly**: Match related records to their parents using a `HashMap`.

This means loading 100 users with their posts executes exactly 2 SQL queries, regardless of how many users or posts exist.

## Example: Load Posts with Author

Relations work from either side:

```rust
use generated::post::PostInclude;

let posts = client
    .post()
    .find_many(PostWhereInput::default())
    .include(PostInclude {
        author: true,
        ..Default::default()
    })
    .exec()
    .await?;

for p in &posts {
    if let Some(author) = &p.author {
        println!("{} by {}", p.data.title, author.email);
    }
}
```

> **Note:** Nested includes (e.g., loading a user's posts and each post's comments) are not yet supported. Use separate queries or raw SQL for deeply nested data.
