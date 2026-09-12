# Why some client migrations require a full table rebuild

Client auto-migration normally changes the local SQLite schema with small,
targeted statements: it creates missing tables and adds missing columns with
`ALTER TABLE ADD COLUMN`. But some schema changes cannot be expressed that way
in SQLite, and when they occur the migration falls back to rebuilding the entire
table from scratch. This document explains why that fallback exists, what it
does, and what protects your data while it runs.

## The trigger: SQLite cannot relax a column in place

The concrete case is a column that changes from `NOT NULL` to nullable. On the
backend, where the database is PostgreSQL, this is a one-line statement: `ALTER
TABLE ... ALTER COLUMN ... DROP NOT NULL`. SQLite has no equivalent. Its `ALTER
TABLE` supports only a handful of operations (renaming, adding columns, and
similar), and changing a column's nullability is not among them. As of writing,
this holds for SQLite 3.50.2 (the version bundled via `libsqlite3-sys` 0.35.0 in
this project's dependency tree); a future SQLite release could add such support,
at which point the rebuild would no longer be necessary.

The framework could simply reject such a change and report an error, but that
would break the promise auto-migration makes: the local schema is reconciled
with your declared schema at startup, without hand-written migration scripts.
Since SQLite offers no in-place way to honor the change, the only remaining path
is to recreate the table with the column already nullable.

## What a rebuild looks like

When the migration finds a declared column that exists in the database as `NOT
NULL` but is nullable in the schema, it rebuilds the table in place:

1. A new table `_carburetor_tmp` is created with the same columns, except the
   target columns are now nullable. Primary keys and defaults are preserved.
2. All data is copied from the original table into the temp table.
3. The original table is dropped, and the temp table is renamed to the original
   name.

The copy step copies every column the database actually has, not just the
columns in your declared schema. If the database contains columns the schema
does not declare, they survive the rebuild. Dropping undeclared columns would
silently destroy data the framework never promised to remove, so the rebuild is
deliberately conservative about what it touches.

## Safety nets around the rebuild

Two mechanisms make the rebuild safe to re-run and safe to fail:

**A stale temp table is dropped first.** If a previous migration run failed
partway through, it can leave `_carburetor_tmp` behind. The rebuild starts with
`DROP TABLE IF EXISTS _carburetor_tmp`, so a re-run succeeds instead of failing
on the `CREATE`. The temp table is an implementation detail; if you ever see one
in your database, it is safe to let the next migration run clean it up.

**The whole migration run is wrapped in a single transaction.** The rebuild
sequence (create, copy, drop, rename) is only safe if it is all-or-nothing: a
partial rebuild, say one that dropped the original table but failed before the
rename, would lose data. Because the entire run is transactional, a failure at
any point rolls back completely and leaves the database as it was.

## The tradeoff

A rebuild is heavier than an in-place alteration: it copies the full table, so
on a large table it costs time and disk space proportional to the table's size.
That cost is accepted because the alternative, rejecting the change, would push
hand-written migration scripts back onto you, which is exactly what
auto-migration exists to avoid. In practice the trigger is narrow: only a `NOT
NULL` to nullable change on an existing column forces a rebuild, so most schema
updates never pay this cost.

## Related

- [How to set up a client with auto-migration](../how-to/setup-client-auto-migration.md)
- [Why client migration type checking compares affinity classes](client-migration-type-affinity-check.md)
