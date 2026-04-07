# Production Deployment

The `ferriorm migrate deploy` command applies pending migrations to a production database. Unlike `migrate dev`, it never generates new migrations or resets data.

## Basic Usage

```bash
DATABASE_URL="postgres://prod-host/mydb" ferriorm migrate deploy
```

This command:

1. Reads the `_ferriorm_migrations` table to determine which migrations have already been applied.
2. Verifies checksums of previously applied migrations.
3. Applies any pending migrations in chronological order.
4. Records each applied migration with its checksum.

## Checksum Verification

Every migration is stored with a SHA-256 checksum. If a previously applied migration's SQL file has been modified since it was applied, `migrate deploy` will fail with a checksum mismatch error.

This prevents accidental corruption of the migration history. If you see this error:

```
Error: Migration checksum mismatch for 20250315120000_init.
The migration has been modified after it was applied.
```

**Do not** edit the migration file to fix it. Instead, investigate why it changed (accidental edit, line-ending differences, etc.) and restore the original content.

## CI/CD Integration

### Docker Example

```dockerfile
FROM rust:1.80 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/myapp /usr/local/bin/
COPY --from=builder /app/migrations/ /app/migrations/
COPY --from=builder /app/schema.ferriorm /app/

# Run migrations before starting the application
CMD ferriorm migrate deploy --schema /app/schema.ferriorm && myapp
```

### GitHub Actions Example

```yaml
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install ferriorm CLI
        run: cargo install ferriorm-cli

      - name: Run migrations
        env:
          DATABASE_URL: ${{ secrets.DATABASE_URL }}
        run: ferriorm migrate deploy

      - name: Deploy application
        run: # your deploy step
```

### General Guidelines

- Run `migrate deploy` **before** deploying new application code.
- If migrations fail, the application deployment should be aborted.
- Each migration runs in its own transaction (for databases that support transactional DDL).
- Migrations are applied in filename order (chronological by timestamp).

## Rollback Strategy

Ferriorm does not provide automatic rollback commands. To revert a migration:

1. Write a new migration that undoes the changes:

   ```bash
   # In development
   ferriorm migrate dev --name revert_add_posts
   ```

2. Edit the generated `migration.sql` to contain the reverse DDL:

   ```sql
   -- Manual rollback
   DROP TABLE IF EXISTS "posts";
   ```

3. Deploy:

   ```bash
   ferriorm migrate deploy
   ```

**Best practices for safe deployments:**
- Make migrations backward-compatible when possible (add columns as nullable, avoid renaming).
- Test migrations against a staging database before production.
- Back up your database before applying migrations.

## Schema File Location

By default, `migrate deploy` looks for `schema.ferriorm` in the current directory. Use `--schema` to specify a different path:

```bash
ferriorm migrate deploy --schema /path/to/schema.ferriorm
```

The `migrations/` directory is resolved relative to the schema file location.
