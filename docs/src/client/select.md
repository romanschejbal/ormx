# Select (Partial Loading)

Use `.select()` to fetch only specific columns from a table. This reduces data transfer and can improve performance for tables with large or many columns.

## Basic Select

```rust
use generated::user::UserSelect;

let users = client
    .user()
    .find_many(UserWhereInput::default())
    .select(UserSelect {
        id: true,
        email: true,
        ..Default::default()
    })
    .exec()
    .await?;

for u in &users {
    println!("id={:?}, email={:?}", u.id, u.email);
}
```

## UserSelect Struct

Each model generates a select struct with a `bool` field per column:

```rust
#[derive(Debug, Clone, Default)]
pub struct UserSelect {
    pub id: bool,
    pub email: bool,
    pub name: bool,
    pub role: bool,
    pub created_at: bool,
    pub updated_at: bool,
}
```

Set fields to `true` to include them in the query. If no fields are set to `true`, all columns are selected (equivalent to `SELECT *`).

## UserPartial Return Type

When using `.select()`, the return type changes from `Model` to `ModelPartial`, where every field is `Option<T>`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserPartial {
    pub id: Option<String>,
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: Option<Role>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

Fields that were not selected will be `None`. Fields that were selected will be `Some(value)` (or `None` only if the database value is NULL).

## Select with Find Unique

```rust
let user = client
    .user()
    .find_unique(UserWhereUniqueInput::Email("alice@example.com".into()))
    .select(UserSelect {
        id: true,
        email: true,
        role: true,
        ..Default::default()
    })
    .exec()
    .await?;

if let Some(u) = user {
    println!("Role: {:?}", u.role.unwrap());
}
```

Returns `Option<UserPartial>`.

## Select with Find First

```rust
let user = client
    .user()
    .find_first(UserWhereInput::default())
    .select(UserSelect {
        email: true,
        ..Default::default()
    })
    .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
    .exec()
    .await?;
```

Returns `Option<UserPartial>`.

## Select with Pagination

`.select()` queries support the same `.order_by()`, `.skip()`, and `.take()` modifiers:

```rust
let page = client
    .user()
    .find_many(UserWhereInput::default())
    .select(UserSelect {
        id: true,
        email: true,
        created_at: true,
        ..Default::default()
    })
    .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
    .skip(0)
    .take(20)
    .exec()
    .await?;
```

## When to Use Select

- **Large text/blob columns** -- skip them when listing records.
- **API responses** -- return only the fields your endpoint needs.
- **Performance** -- fewer columns means less data over the wire and less deserialization work.

> **Note:** `.select()` and `.include()` cannot be combined on the same query. Use `.include()` when you need related records, or run separate queries.
