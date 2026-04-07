# Datasource

The `datasource` block tells ferriorm which database to connect to and what dialect to use for SQL generation and migrations.

## Syntax

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}
```

## Rules

- **Exactly one** `datasource` block is allowed per schema file.
- The block name (e.g., `db`) is an identifier you choose; it does not affect behavior.
- Two fields are required: `provider` and `url`.

## `provider`

Specifies the database engine. The value is a case-insensitive string.

| Value | Database |
|---|---|
| `"postgresql"` or `"postgres"` | PostgreSQL |
| `"sqlite"` | SQLite |

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}
```

```prisma
datasource db {
  provider = "sqlite"
  url      = "file:./dev.db"
}
```

## `url`

The database connection string. It can be specified in two ways:

### Environment variable (recommended)

Use the `env()` function to read the URL from an environment variable at runtime. This keeps credentials out of your source files.

```prisma
url = env("DATABASE_URL")
```

### String literal

Provide the URL directly as a quoted string. This is convenient for local SQLite databases or quick prototyping, but should be avoided for production configurations that contain credentials.

```prisma
url = "file:./dev.db"
```

### Connection string formats

**PostgreSQL:**

```
postgresql://user:password@localhost:5432/mydb
```

**SQLite:**

```
file:./dev.db
```

## Example

```prisma
datasource db {
  provider = "postgresql"
  url      = env("DATABASE_URL")
}
```

At runtime, ferriorm resolves `env("DATABASE_URL")` by reading the `DATABASE_URL` environment variable. If the variable is not set, the client will return an error at connection time.
