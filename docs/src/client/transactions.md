# Transactions

Ferriorm supports database transactions through the `run_transaction` function. A transaction groups multiple operations into an atomic unit -- either all succeed (commit) or all are rolled back.

## Basic Usage

```rust
use ferriorm_runtime::transaction::run_transaction;

let new_user = run_transaction(client.client(), |tx| async move {
    // ... perform operations using tx ...
    // Return the result and the tx client
    Ok((result, tx))
}).await?;
```

## How It Works

1. `run_transaction` begins a database transaction.
2. Your closure receives a `TransactionClient` wrapping the transaction handle.
3. On `Ok((value, tx))` -- the transaction is committed and `value` is returned.
4. On `Err(e)` -- the `TransactionClient` is dropped, which triggers an automatic rollback via sqlx.

## TransactionClient

`TransactionClient` is an enum mirroring `DatabaseClient`:

```rust
pub enum TransactionClient {
    Postgres(sqlx::Transaction<'static, sqlx::Postgres>),
    Sqlite(sqlx::Transaction<'static, sqlx::Sqlite>),
}
```

It provides `commit()` and `rollback()` methods, but you typically do not call them directly -- `run_transaction` handles this based on your closure's return value.

## Complete Example

Transfer a post from one user to another, ensuring both the update and a log entry succeed atomically:

```rust
use ferriorm_runtime::transaction::run_transaction;

let post_id = "some-post-id";
let new_author_id = "new-author-id";

let updated_post = run_transaction(client.client(), |tx| async move {
    // For now, use the raw sqlx transaction handle for queries
    match tx {
        ferriorm_runtime::transaction::TransactionClient::Postgres(mut pg_tx) => {
            // Update the post's author
            let post: Post = sqlx::query_as::<_, Post>(
                "UPDATE posts SET author_id = $1, updated_at = NOW() \
                 WHERE id = $2 RETURNING *"
            )
                .bind(new_author_id)
                .bind(post_id)
                .fetch_one(&mut *pg_tx)
                .await
                .map_err(FerriormError::from)?;

            // Insert an audit log entry
            sqlx::query(
                "INSERT INTO audit_log (action, entity_id, timestamp) \
                 VALUES ($1, $2, NOW())"
            )
                .bind("transfer_post")
                .bind(post_id)
                .execute(&mut *pg_tx)
                .await
                .map_err(FerriormError::from)?;

            // Return the result and wrap the transaction back
            let tx_client = ferriorm_runtime::transaction::TransactionClient::Postgres(pg_tx);
            Ok((post, tx_client))
        }
        _ => Err(FerriormError::Connection("Expected PostgreSQL".into())),
    }
}).await?;

println!("Transferred post: {}", updated_post.title);
```

## Error Handling and Rollback

If any operation in the closure fails, return an `Err` and the transaction rolls back automatically:

```rust
let result = run_transaction(client.client(), |tx| async move {
    match tx {
        ferriorm_runtime::transaction::TransactionClient::Postgres(mut pg_tx) => {
            // This succeeds
            sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
                .bind("id-1")
                .bind("alice@example.com")
                .execute(&mut *pg_tx)
                .await
                .map_err(FerriormError::from)?;

            // This fails (duplicate email) -- entire transaction rolls back
            sqlx::query("INSERT INTO users (id, email) VALUES ($1, $2)")
                .bind("id-2")
                .bind("alice@example.com")
                .execute(&mut *pg_tx)
                .await
                .map_err(FerriormError::from)?;

            let tx_client = ferriorm_runtime::transaction::TransactionClient::Postgres(pg_tx);
            Ok(((), tx_client))
        }
        _ => Err(FerriormError::Connection("Expected PostgreSQL".into())),
    }
}).await;

match result {
    Ok(_) => println!("Committed"),
    Err(e) => println!("Rolled back: {e}"),
}
```

## Function Signature

```rust
pub async fn run_transaction<F, Fut, T>(
    client: &DatabaseClient,
    f: F,
) -> Result<T, FerriormError>
where
    F: FnOnce(TransactionClient) -> Fut,
    Fut: Future<Output = Result<(T, TransactionClient), FerriormError>>;
```

Key points:
- The closure must return `(T, TransactionClient)` on success so the transaction can be committed.
- `T` can be any type -- it is the value returned to the caller after commit.
- The closure takes ownership of `TransactionClient` and must return it.
