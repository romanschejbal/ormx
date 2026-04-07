# Raw SQL

Ferriorm provides escape hatches for executing raw SQL when the generated query builders are not sufficient.

## When to Use Raw SQL

- Complex joins or subqueries not expressible with the query builder
- Database-specific features (CTEs, window functions, full-text search)
- Performance-critical queries that benefit from hand-tuned SQL
- Bulk operations beyond what `create_many`/`update_many` support

## Zero-Bind Queries

For simple queries without parameters, use the `raw_fetch_*` and `raw_execute_*` methods directly on the client.

### PostgreSQL

```rust
use generated::user::User;

// Fetch all rows
let users: Vec<User> = client
    .client()
    .raw_fetch_all_pg("SELECT * FROM users ORDER BY created_at DESC")
    .await?;

// Fetch exactly one row
let user: User = client
    .client()
    .raw_fetch_one_pg("SELECT * FROM users LIMIT 1")
    .await?;

// Fetch an optional row
let user: Option<User> = client
    .client()
    .raw_fetch_optional_pg("SELECT * FROM users WHERE email = 'alice@example.com'")
    .await?;

// Execute without returning rows (returns rows affected)
let rows_affected: u64 = client
    .client()
    .raw_execute_pg("DELETE FROM users WHERE role = 'user'")
    .await?;
```

### SQLite

The SQLite variants work identically:

```rust
let users: Vec<User> = client
    .client()
    .raw_fetch_all_sqlite("SELECT * FROM users ORDER BY created_at DESC")
    .await?;

let rows: u64 = client
    .client()
    .raw_execute_sqlite("UPDATE users SET role = 'admin' WHERE id = '123'")
    .await?;
```

### Return Type Requirement

The generic type `T` must implement `sqlx::FromRow` for the corresponding database row type. All ferriorm-generated model structs (`User`, `Post`, etc.) implement this automatically.

For custom result types, derive `sqlx::FromRow`:

```rust
#[derive(sqlx::FromRow)]
struct UserCount {
    role: String,
    count: i64,
}

let counts: Vec<UserCount> = client
    .client()
    .raw_fetch_all_pg("SELECT role, COUNT(*) as count FROM users GROUP BY role")
    .await?;
```

## Parameterized Queries with sqlx

For queries with user-provided values (to prevent SQL injection), access the underlying sqlx pool and use sqlx's query API directly.

### PostgreSQL

```rust
let pool = client.pg_pool()?;

// Parameterized SELECT
let users: Vec<User> = sqlx::query_as::<_, User>(
    "SELECT * FROM users WHERE email = $1 AND role = $2"
)
    .bind("alice@example.com")
    .bind("admin")
    .fetch_all(pool)
    .await?;

// Parameterized INSERT
let result = sqlx::query(
    "INSERT INTO users (id, email, name, role, created_at, updated_at) \
     VALUES ($1, $2, $3, $4, NOW(), NOW())"
)
    .bind(uuid::Uuid::new_v4().to_string())
    .bind("bob@example.com")
    .bind("Bob")
    .bind("user")
    .execute(pool)
    .await?;

println!("Inserted {} rows", result.rows_affected());
```

### SQLite

```rust
let pool = client.sqlite_pool()?;

let users: Vec<User> = sqlx::query_as::<_, User>(
    "SELECT * FROM users WHERE email = ?1"
)
    .bind("alice@example.com")
    .fetch_all(pool)
    .await?;
```

> **Note:** PostgreSQL uses `$1, $2, ...` for bind parameters. SQLite uses `?1, ?2, ...` or just `?`.

## Complex Query Example

Using a CTE to rank users by post count:

```rust
#[derive(sqlx::FromRow)]
struct UserWithPostCount {
    email: String,
    post_count: i64,
    rank: i64,
}

let pool = client.pg_pool()?;

let ranked: Vec<UserWithPostCount> = sqlx::query_as::<_, UserWithPostCount>(
    "WITH user_posts AS (
        SELECT u.email, COUNT(p.id) as post_count
        FROM users u
        LEFT JOIN posts p ON p.author_id = u.id
        GROUP BY u.email
    )
    SELECT email, post_count,
           RANK() OVER (ORDER BY post_count DESC) as rank
    FROM user_posts"
)
    .fetch_all(pool)
    .await?;
```

## Mixing Raw SQL and Query Builders

You can use raw SQL for reads and the generated client for writes (or vice versa) within the same application. Both operate on the same connection pool.

```rust
// Complex read with raw SQL
let top_users: Vec<User> = client
    .client()
    .raw_fetch_all_pg(
        "SELECT u.* FROM users u \
         JOIN posts p ON p.author_id = u.id \
         GROUP BY u.id \
         HAVING COUNT(p.id) > 5 \
         ORDER BY COUNT(p.id) DESC"
    )
    .await?;

// Type-safe update with the query builder
for user in &top_users {
    client
        .user()
        .update(
            UserWhereUniqueInput::Id(user.id.clone()),
            UserUpdateInput {
                role: Some(SetValue::Set(Role::Moderator)),
                ..Default::default()
            },
        )
        .exec()
        .await?;
}
```
