# Aggregates

Ferriorm supports aggregate operations (`min`, `max`, `avg`, `sum`) on model fields. The operations are type-checked at compile time based on the field's data type.

## Basic Usage

```rust
use generated::user::UserAggregateField;
use generated::user::filter::UserWhereInput;

let result = client
    .user()
    .aggregate(UserWhereInput::default())
    .min(UserAggregateField::CreatedAt)
    .max(UserAggregateField::CreatedAt)
    .exec()
    .await?;

println!("Earliest user: {:?}", result.min_created_at);
println!("Latest user: {:?}", result.max_created_at);
```

## API

Start an aggregate query with `.aggregate(where_input)`, chain operations, then call `.exec()`:

```rust
client
    .model()
    .aggregate(WhereInput::default())
    .min(field)
    .max(field)
    .avg(field)   // numeric fields only
    .sum(field)   // numeric fields only
    .exec()
    .await?
```

At least one operation must be specified or `.exec()` returns an error.

## UserAggregateField

Each model generates an enum listing the fields that support aggregation:

```rust
pub enum UserAggregateField {
    CreatedAt,
    UpdatedAt,
}
```

### Which operations are allowed?

| Field type | `min` | `max` | `avg` | `sum` |
|---|---|---|---|---|
| `Int`, `BigInt`, `Float` | Yes | Yes | Yes | Yes |
| `DateTime` | Yes | Yes | No | No |
| `String`, `Boolean`, `Enum` | No | No | No | No |

Calling `avg()` or `sum()` on a non-numeric field will panic at runtime with a clear message.

## UserAggregateResult

The return type contains an `Option` field for each possible operation+field combination:

```rust
pub struct UserAggregateResult {
    pub min_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub max_created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub min_updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub max_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

Fields that were not requested in the query will be `None` (via `#[sqlx(default)]`).

For models with numeric fields, you would also see fields like `avg_age`, `sum_score`, etc.

## Aggregate with Filters

Pass a `WhereInput` to compute aggregates over a subset of records:

```rust
use generated::user::filter::UserWhereInput;
use ferriorm_runtime::filter::EnumFilter;
use generated::Role;

let result = client
    .user()
    .aggregate(UserWhereInput {
        role: Some(EnumFilter {
            equals: Some(Role::Admin),
            ..Default::default()
        }),
        ..Default::default()
    })
    .min(UserAggregateField::CreatedAt)
    .max(UserAggregateField::CreatedAt)
    .exec()
    .await?;

println!("First admin created: {:?}", result.min_created_at);
```

## Numeric Aggregate Example

For a model with numeric fields (e.g., `Order` with a `total` field of type `Float`):

```rust
let stats = client
    .order()
    .aggregate(OrderWhereInput {
        status: Some(EnumFilter {
            equals: Some(OrderStatus::Completed),
            ..Default::default()
        }),
        ..Default::default()
    })
    .sum(OrderAggregateField::Total)
    .avg(OrderAggregateField::Total)
    .min(OrderAggregateField::Total)
    .max(OrderAggregateField::Total)
    .exec()
    .await?;

println!("Revenue: {:?}", stats.sum_total);
println!("Average order: {:?}", stats.avg_total);
```
