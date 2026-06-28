# Column Default Values

## Overview

The `#[default]` column attribute specifies default values for columns in
`carburetor_sync_config!` (see [basic feature](./basic-feature.md)). It serves
three purposes:

1. **Defining where default values come from** — Rust defaults supply a Rust
   expression embedded in generated insert code, so the field is populated in
   Rust wherever inserts execute (client or server). SQL defaults tell the
   framework the database will supply a value, so generated insert models omit
   the column (`Option<T>`) and leave it to the database.
2. **Database DDL defaults** — SQL defaults with the `migration` feature provide
   a `DEFAULT …` clause in the DDL, so the database engine supplies the value
   when a row is inserted without that column.
3. **Sync lifecycle support** — Defaults supply initial values at specific
   points in the sync lifecycle. Exactly when a default fires depends on the
   column's role in syncing (see [When Default Values Are
   Applied](#when-default-values-are-applied)).

Each column accepts **at most one** `#[default]` attribute — either `rust` or
`sql`. Multiple `#[default]` attributes on the same column produce a
compile-time error.

The behavior of `sql` defaults depends on whether the `migration` Cargo feature
is enabled:

- **With `migration`** — The variant must be specified explicitly so the
  migration system knows what `DEFAULT …` clause to emit.
- **Without `migration`** — No variant is needed; the attribute serves as a
  marker that the column has a SQL default, since you manage the schema
  yourself.

## Core Implementation Library/Framework/Tool

This feature uses the same tooling listed in [basic
feature](./basic-feature.md#core-implementation-libraryframeworktool).

## Feature Components

### When Default Values Are Applied

When a default fires depends on the column's role in the sync lifecycle.
Columns fall into three categories:

#### Synced Columns

Regular data columns participate fully in both upload and download — their
values always travel with the record. Defaults for these columns apply only
when the client creates a new record locally (via the generated
`insert_<table>` function), so they are never needed during sync itself.

#### Client-Managed Columns

`#[dirty_flag]` and `#[client_column_sync_metadata]` exist only on the client.
The backend has no knowledge of them, so they never appear in upload or
download payloads. Their defaults apply in two situations:

- **Local insert** — When the client creates a new record, these columns are
  populated from their defaults, just like synced columns.
- **Download sync (new record)** — When a record arrives from the backend for
  the first time, the client must populate these columns itself. Defaults fill
  this gap.

On **download sync (existing record)**, these columns are **preserved** — the
incoming data does not include them, so they are left untouched rather than
reset to their default. This ensures client-side state (e.g., dirty tracking)
survives across sync cycles.

These columns always carry a built-in default (see [Interaction with Existing
Column Attributes](#interaction-with-existing-column-attributes)); a custom
`#[default]` cannot be applied to them.

#### Backend-Managed Columns

`#[last_synced_at]` is downloaded to the client but never uploaded. When the
client uploads a new record, the backend applies the default to populate the
column.

This category is a **special case** that is not yet fully generalized. The
open question is whether a backend-managed default should apply on **every**
upload write (as `#[last_synced_at]` does, to track the last sync time) or
only on **new-record** uploads. Until this is resolved, defaults for
backend-managed columns are tied to their specific built-in semantics rather
than exposed through `#[default]`.

### Rust Default

A Rust default embeds a Rust expression into generated insert code via `quote!`,
so the field is populated in Rust wherever the insert executes (client or
server). The expression must be valid Rust that resolves to the column's Rust
type.

#### Syntax

```
#[default(rust = "<Rust expression>")]
```

#### Example

```rust
tables {
    config(plural = "configs") {
        #[id]
        id -> Text,
        #[default(rust = "carburetor::serde_json::from_str(\"{}\").unwrap()")]
        settings -> Jsonb,
    }
}
```

When the client inserts a `config` row, `settings` defaults to `{}`.

### SQL Default with migration feature

<!-- TODO: Add link to migration feature when document is ready -->
When the `migration` Cargo feature is enabled, the framework generates DDL to
create tables and columns. The variant tells the migration system exactly what
`DEFAULT …` clause to emit.

#### Syntax

```
#[default(sql = <variant>)]
```

#### Supported Variants

| Variant | PostgreSQL DDL | SQLite DDL | Description |
|---|---|---|---|
| `Null` | `DEFAULT NULL` | `DEFAULT NULL` | SQL `NULL`. For nullable columns. |
| `EmptyJson` | `DEFAULT '{}'::jsonb` | `DEFAULT '{}'` | Empty JSON object. For `Jsonb` or `Nullable<Jsonb>` columns. |
| `Now` | varies by type (see below) | varies by type (see below) | Current date/time. For `Timestamptz`, `Timestamp`, `Date`, `Time`, and their nullable counterparts. |
| `Text("<value>")` | `DEFAULT '<value>'` | `DEFAULT '<value>'` | Literal text string. For `Text` or `Nullable<Text>` columns. |
| `Number(<value>)` | `DEFAULT <value>` | `DEFAULT <value>` | Numeric literal (integer or float). For numeric columns and their nullable counterparts. |

`Now` DDL varies by column type:

| Column type | PostgreSQL DDL | SQLite DDL |
|---|---|---|
| `Timestamptz` | `DEFAULT now()` | `DEFAULT (datetime('now'))` |
| `Timestamp` | `DEFAULT now()` | `DEFAULT (datetime('now'))` |
| `Date` | `DEFAULT CURRENT_DATE` | `DEFAULT (date('now'))` |
| `Time` | `DEFAULT CURRENT_TIME` | `DEFAULT (time('now'))` |

#### Example

```rust
tables {
    event(plural = "events") {
        #[id]
        id -> Text,
        #[default(sql = Now)]
        created_at -> Timestamptz,
        #[default(sql = EmptyJson)]
        metadata -> Jsonb,
        #[default(sql = Text("pending"))]
        status -> Text,
        #[default(sql = Number(0))]
        priority -> Integer,
        #[default(sql = Null)]
        note -> Nullable<Text>,
    }
}
```


### SQL Default without migration feature

Without `migration`, you manage the database schema yourself and no DDL is
generated, so no variant is needed. `#[default(sql)]` serves as a **marker**
that the column has a SQL default — it tells the framework the database will
supply a value, so generated insert functions may omit the column.

#### Syntax

```
#[default(sql)]
```

#### Example

```rust
tables {
    event(plural = "events") {
        #[id]
        id -> Text,
        #[default(sql)]
        created_at -> Timestamptz,
        #[default(sql)]
        metadata -> Jsonb,
    }
}
```

You must ensure the database schema defines appropriate defaults (e.g.
`DEFAULT now()` for `created_at`, `DEFAULT '{}'::jsonb` for `metadata`) when
creating or altering tables.

### Interaction with Existing Column Attributes

Several built-in column attributes (documented in [basic feature](./basic-feature.md#table-configuration)) already carry implicit defaults:

| Attribute | Implicit SQL default |
|---|---|
| `#[last_synced_at]` | `Now` |
| `#[dirty_flag]` | `Null` |
| `#[client_column_sync_metadata]` | `EmptyJson` |

Applying `#[default]` to these columns is a **compile-time error** — their defaults are intrinsic. Regular data columns (`#[id]`, `#[is_deleted]`, and unmarked columns) accept `#[default]`.

## Challenges and Considerations

### Type Compatibility

When the `migration` feature is enabled, the SQL default variant must be
compatible with the column's Diesel type:

- `Now` — with `Timestamptz`, `Nullable<Timestamptz>`, `Timestamp`, `Nullable<Timestamp>`, `Date`, `Nullable<Date>`, `Time`, or `Nullable<Time>` columns
- `EmptyJson` — only with `Jsonb` or `Nullable<Jsonb>` columns
- `Text(…)` — only with `Text` or `Nullable<Text>` columns
- `Number(…)` — only with numeric columns (`Integer`, `Nullable<Integer>`, `BigInt`, `Nullable<BigInt>`, `Float`, `Nullable<Float>`, etc.)
- `Null` — only with `Nullable<…>` columns

Mismatches produce a compile-time error.

### Diesel Limitations

The initial variants (`Null`, `EmptyJson`, `Now`, `Text`, `Number`) cover common cases. Complex defaults (e.g. UUID generation, custom functions) can be added as variants in the future.
