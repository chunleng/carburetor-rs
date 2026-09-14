# `tables` block of `carburetor_sync_config!`

Declares the tables available for syncing, their columns, and the special sync
columns.

## Signature

```rust
carburetor_sync_config! {
    tables {
        // `plural` is optional; defaults to the table name plus `s`
        table_name(plural = "plural_table_name") {
            // Special columns, shown with their default column names and
            // types. Each is auto-generated if not declared explicitly.
            #[id] id -> Text,
            #[last_synced_at] last_synced_at -> Timestamptz,
            #[is_deleted] is_deleted -> Bool,
            #[dirty_flag] dirty_flag -> Nullable<Text>,
            #[client_column_sync_metadata] column_sync_metadata -> Jsonb,
            column_name -> Type,
        }
    }
}
```

## Example Usage

```rust
carburetor_sync_config! {
    tables {
        // `person` would default to "persons"; override with the
        // irregular plural
        person(plural = "people") {
            name -> Text,
        }
        // `user` uses the default plural "users"
        user {
            #[id] uuid -> Text, // explicitly rename `id` to `uuid`
            username -> Text,
        }
    }
}
```

## Applicability

### Table declaration

```rust
table_name(plural = "plural_table_name") {
  ...
}
```

- `table_name`: table name, written as a bare identifier
  - Declares one table available for syncing
- `plural` arg:
  - Primarily for the database table name, and generated function naming
  - Arguments: `plural` (optional)
  - Default: `plural` defaults to the table name plus `s`

### Column syntax

```rust
#[attribute] column_name -> Type
```

- `#[attribute]`: optional; only used for the five special column attributes
  (`#[id]`, `#[last_synced_at]`, `#[is_deleted]`, `#[dirty_flag]`,
  `#[client_column_sync_metadata]`). Plain data columns are written without any
  attribute
- Name: column name, written as a bare identifier
- Type: one of the supported column types: `Text`, `SmallInt`, `Integer`,
  `Serial`, `BigInt`, `BigSerial`, `Float`, `Double`, `Bool`, `Timestamp`,
  `Timestamptz`, `Date`, `Time`, `Jsonb`, or `Nullable<T>` of any of these.
  `Serial` and `BigSerial` are aliases for `Integer` and `BigInt`. Other types
  fail parsing
- Default: none; columns must be declared explicitly
- Description: declared in diesel `table!` style, `name -> Type`, comma
  separated
- Errors: duplicate column names fail, including collisions with the five
  auto-generated special columns (`id`, `last_synced_at`, `is_deleted`,
  `dirty_flag`, `column_sync_metadata`)

### Special columns

Each special column is auto-generated with its default column name if not
declared explicitly.

| Name | Type | Default Generated Column Name | Managed By | Description |
| --- | --- | --- | --- | --- |
| `#[id]` | `Text` | `id` | Backend and client | The unique sync ID column identifying a row across backend and clients |
| `#[last_synced_at]` | `Timestamptz` | `last_synced_at` | Backend only | Server-side update timestamp; synced down to clients, never modified by the client |
| `#[is_deleted]` | `Bool` | `is_deleted` | Backend and client | Soft-delete flag. Records are never physically deleted; see [soft deletion](../../explanation/soft-deletion.md) |
| `#[dirty_flag]` | `Nullable<Text>` | `dirty_flag` | Client only | Dirty marker recording why a row is dirty. Default value `None` |
| `#[client_column_sync_metadata]` | `Jsonb` | `column_sync_metadata` | Client only | Per-column sync metadata used for column-level conflict resolution |

## Effect on generated code

Each declared table generates per-table functions on the client and the backend.
These functions exist only for tables that are also assigned to a sync group;
see [sync group](sync-group.md).

### Client-side

Local operations (all set the dirty flag so the change is picked up by the
next upload):

- `insert_<table>()`: inserts a record locally
- `update_<table>()`: updates a record locally
- `delete_<table>()`: soft-deletes a record locally
- `active_<plural>()`: query helper that filters out soft-deleted records

Sync helpers:

- `sync_<table>()`: merges a downloaded table's records into the local DB with
  LWW conflict resolution
- `retrieve_<table>_upload_data()`: gathers dirty records for the upload request
- `process_<table>_upload_response()`: applies the upload response and clears
  dirty flags

### Backend-side

- `download_<table>()`: queries rows with `last_synced_at` greater than the
  given offset and returns a `DownloadTableResponse`
- `process_upload_request_<table>()`: applies uploaded records with an LWW merge
  using column timestamps

## Usage notes

- Tables are assigned to sync groups with the `sync_groups` block; see
  [sync group](sync-group.md)

### Anti-pattern: declaring a special column by name without its attribute

```rust
user {
    id -> Text,
}
```

Without `#[id]`, the macro auto-generates `#[id] id -> Text`, which collides
with the explicitly declared `id` column and fails with a duplicate column
error. If the `id` column is intentionally not the sync ID, rename the default
`#[id] id -> Text` to something else, e.g. `#[id] row_id -> Text`.
