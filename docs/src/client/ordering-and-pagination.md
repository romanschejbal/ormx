# Ordering & Pagination

## Ordering

Use `.order_by()` to sort results. Each model generates a `ModelOrderByInput` enum with a variant for every field.

```rust
use generated::user::order::UserOrderByInput;
use ferriorm_runtime::prelude::SortOrder;

let users = client
    .user()
    .find_many(UserWhereInput::default())
    .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
    .exec()
    .await?;
```

### SortOrder

| Variant | SQL |
|---|---|
| `SortOrder::Asc` | `ASC` |
| `SortOrder::Desc` | `DESC` |

### Multiple Order Clauses

Chain multiple `.order_by()` calls. They are applied in order (first call is the primary sort):

```rust
let users = client
    .user()
    .find_many(UserWhereInput::default())
    .order_by(UserOrderByInput::Role(SortOrder::Asc))
    .order_by(UserOrderByInput::Email(SortOrder::Asc))
    .exec()
    .await?;
// SQL: ORDER BY "role" ASC, "email" ASC
```

### Available Fields

Every column in the model has a corresponding variant:

```rust
pub enum UserOrderByInput {
    Id(SortOrder),
    Email(SortOrder),
    Name(SortOrder),
    Role(SortOrder),
    CreatedAt(SortOrder),
    UpdatedAt(SortOrder),
}
```

## Pagination

Use `.skip(n)` and `.take(n)` for offset-based pagination. Both accept `i64` values.

| Method | SQL | Description |
|---|---|---|
| `.take(n)` | `LIMIT n` | Maximum number of records to return |
| `.skip(n)` | `OFFSET n` | Number of records to skip |

### Basic Pagination

```rust
// Page 1: first 10 records
let page1 = client
    .user()
    .find_many(UserWhereInput::default())
    .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
    .take(10)
    .exec()
    .await?;

// Page 2: next 10 records
let page2 = client
    .user()
    .find_many(UserWhereInput::default())
    .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
    .skip(10)
    .take(10)
    .exec()
    .await?;
```

### Paginated List Helper

A common pattern for API endpoints:

```rust
async fn list_users(
    client: &FerriormClient,
    page: i64,
    page_size: i64,
) -> Result<(Vec<User>, i64), FerriormError> {
    let filter = UserWhereInput::default();

    let total = client
        .user()
        .count(filter.clone())
        .exec()
        .await?;

    let users = client
        .user()
        .find_many(filter)
        .order_by(UserOrderByInput::CreatedAt(SortOrder::Desc))
        .skip((page - 1) * page_size)
        .take(page_size)
        .exec()
        .await?;

    Ok((users, total))
}
```

### Ordering with Select

Ordering works with `.select()` queries too:

```rust
let partials = client
    .user()
    .find_many(UserWhereInput::default())
    .select(UserSelect { id: true, email: true, ..Default::default() })
    .order_by(UserOrderByInput::Email(SortOrder::Asc))
    .skip(0)
    .take(50)
    .exec()
    .await?;
```

> **Note:** Always include an `order_by` clause when paginating. Without it, the database does not guarantee a stable ordering, and pages may contain duplicate or missing records.
