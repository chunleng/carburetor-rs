# Auto Migration — Server (PostgreSQL)

## Overview

The server-side auto-migration feature provides automatic database schema
synchronization for PostgreSQL based on the table definitions in
`carburetor_sync_config!`. When invoked, it introspects the current database
state and reconciles it with the declared schema, creating missing tables and
columns as needed.

The feature follows a **non-destructive, append-only** migration strategy. This
approach prioritizes safety — existing data and manual modifications are never
at risk, but destructive operations (column removal, renames) remain manual.
This ensures that:

- Existing data is never lost due to schema evolution
- Manual database modifications (custom columns, indexes, etc.) are preserved
- Applications can extend the schema with additional non-Carburetor-managed
  columns without conflict

## Core Implementation

| Tool | Purpose |
|---|---|
| Diesel | Introspect existing schema; execute DDL |
| PostgreSQL | Schema introspection via `information_schema` |
| Proc macro | Generate migration logic at compile time |

The proc macro uses `syn`, `quote`, and `proc-macro2` for code generation.

## Feature Components

### Schema Introspection

The auto-migration system queries PostgreSQL:

- `information_schema.tables` to check if tables exist
- `information_schema.columns` to retrieve column definitions for existing
  tables
- Column types are compared with declared schema types (with appropriate type
  mapping)

### Migration Strategy

Carburetor's schema definitions are the source of truth — the migration
checks the database against them.

**Table does not exist** — The table is created with all columns declared
in the schema.

**Table exists** — The existing table is kept and updated:
- Missing columns are added to the table:
  - **Non-nullable column**: Must have a SQL default value, specified via the
    [`#[default]`](../column-default.md) attribute. The column is added as
    `NOT NULL DEFAULT ...`, satisfying the constraint immediately. If no default
    is provided, the migration fails — adding a `NOT NULL` column without a
    default to a table with existing rows is not supported.
  - **Nullable column**: Can be added without special handling, as this is
    allowed by PostgreSQL.
- Existing columns with a different type than the schema declaration cause the
  migration to fail with an error. The mismatch is caught during schema
  introspection, before any DDL is executed — the database is untouched.
- Existing columns that are `NOT NULL` in the database but declared nullable
  in the schema are converted to nullable via
  `ALTER COLUMN ... DROP NOT NULL`. This handles the mandatory→optional
  evolution: when a previously required column becomes optional in the schema,
  the migration relaxes the constraint automatically. The operation is
  idempotent — already-nullable columns are left untouched, so re-running the
  migration is a no-op. The reverse direction (optional→mandatory,
  `ADD NOT NULL`) is not supported because existing rows with `NULL` values
  would violate the new constraint.
- Columns not in the schema are left untouched, with validation to catch
  cases where non-null columns without defaults could cause issues for the
  framework

Each table migration is wrapped in a transaction (PostgreSQL: `BEGIN` /
`COMMIT`). If a migration fails, the transaction prevents partial state. The
function returns a `Result` type for the caller to handle failures.

### Generated Migration Function

The macro generates a migration function for the backend:

```rust
pub fn run_migrations(conn: &mut PgConnection) -> diesel::result::Result<()>
```

The function is called at application startup to synchronize the schema.

### Column Attribute Handling

Special columns defined with attributes must be created correctly on the
server:

- `#[id]` column: `TEXT PRIMARY KEY`
- `#[last_synced_at]` column: `TIMESTAMPTZ NOT NULL DEFAULT now()`
- `#[is_deleted]` column: `BOOLEAN`
- `#[column_sync_metadata]` column: `JSONB`
- `#[backend_only]` columns: Created on the server using PostgreSQL-specific
  types when appropriate
- `#[client_only]` columns: **Not created** on server-side migrations

## Challenges and Considerations

The append-only design introduces trade-offs. This section documents known
limitations and the rationale behind out-of-scope features.

### Index and Constraint Management

The initial implementation focuses on tables and columns only — indexes,
foreign keys, and other constraints are not managed:

- Manual indexes are preserved
- Foreign key and unique constraints must be added manually

Future enhancements may add support for constraint management, but MVP excludes
them to keep complexity manageable.

### Breaking Schema Changes

Auto-migration **cannot handle** certain schema changes:

- Renaming a column: Creates a new column, leaving the old one orphaned
- Changing column type incompatibly: Leaves the old column unchanged,
  causing potential application errors
- Removing a column: Not supported by design

Applications requiring these changes must:
1. Perform manual database migration outside of auto-migration
2. Update the schema definition after manual migration is complete
3. Re-run auto-migration to sync any remaining differences

### Multiple Application Instances

In distributed systems with multiple backend instances running concurrently:

- Multiple instances may attempt migration simultaneously
- PostgreSQL may require advisory locks
- Use `IF NOT EXISTS` and `ADD COLUMN IF NOT EXISTS` for idempotent
  concurrent execution

### Test Database Setup

Current test setup uses manual table creation. After auto-migration is
implemented:

- Tests should call `run_migrations()` instead of manual DDL
- Tests requiring a clean slate can drop tables before calling migrations
- Tests can verify migration idempotency (calling it twice should succeed)
