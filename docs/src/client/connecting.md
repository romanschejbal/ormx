# Connecting to a Database

Ferriorm connects to your database through a generated `FerriormClient` that wraps a connection pool. The client auto-detects your database type from the connection URL.

## Basic Connection

```rust
use generated::FerriormClient;

let client = FerriormClient::connect("postgres://user:pass@localhost/mydb").await?;

// Use the client...

client.disconnect().await;
```

The URL scheme determines which driver is used:

| URL pattern | Database |
|---|---|
| `postgres://...` or `postgresql://...` | PostgreSQL |
| `sqlite:...`, `file:...`, or `*.db` | SQLite |

## Connection with Pool Configuration

For production deployments, configure the underlying connection pool using `PoolConfig`:

```rust
use generated::FerriormClient;
use ferriorm_runtime::client::PoolConfig;
use std::time::Duration;

let config = PoolConfig {
    max_connections: Some(20),
    min_connections: Some(5),
    idle_timeout: Some(Duration::from_secs(300)),
    max_lifetime: Some(Duration::from_secs(1800)),
    acquire_timeout: Some(Duration::from_secs(5)),
    ..Default::default()
};

let client = FerriormClient::connect_with_config(
    "postgres://user:pass@localhost/mydb",
    &config,
).await?;
```

### PoolConfig Options

| Field | Type | Description |
|---|---|---|
| `max_connections` | `Option<u32>` | Maximum number of connections in the pool. |
| `min_connections` | `Option<u32>` | Minimum idle connections to keep open. |
| `idle_timeout` | `Option<Duration>` | How long a connection can sit idle before being closed. |
| `max_lifetime` | `Option<Duration>` | Maximum lifetime of a connection before it is recycled. |
| `acquire_timeout` | `Option<Duration>` | Maximum time to wait when acquiring a connection. |

All fields default to `None`, which uses the sqlx defaults.

## Disconnecting

Always disconnect before your application exits to close the pool gracefully:

```rust
client.disconnect().await;
```

`disconnect()` consumes the client, so it cannot be used after disconnection.

## Accessing the Raw Pool

If you need the underlying sqlx pool for advanced operations, use the accessor methods:

```rust
// PostgreSQL
let pool: &sqlx::PgPool = client.pg_pool()?;

// SQLite
let pool: &sqlx::SqlitePool = client.sqlite_pool()?;
```

These return an error if the client is connected to a different database type than requested. See [Raw SQL](./raw-sql.md) for usage examples.

## Feature Flags

The database backends are controlled by Cargo feature flags on `ferriorm-runtime`:

- `postgres` -- enables PostgreSQL support
- `sqlite` -- enables SQLite support

Both can be enabled simultaneously. If neither is enabled, `connect()` returns an error.
