# Basic Feature

## Overview

Carburetor is a local-first framework that enables sync between a storage
backend and multiple frontend devices with LWW (Last Writes Win) Register.

The sync-config feature provides a function-like proc macro
`carburetor_sync_config!` that defines table configurations and multiple sync
groups. The same macro generates both backend code (PostgreSQL) and client code
(SQLite) depending on the feature flag used. This enables developers to:

- Define table structures once and reuse them across multiple groups
- Create different sync groups for different applications or use cases
- Synchronize different subsets of tables with different filtering per group
- Maintain independent sync tracking for each table within each group

## Core Implementation Library/Framework/Tool

| Library/Framework/Tool | Purpose |
|---|---|
| Diesel | Database abstraction and ORM for PostgreSQL (backend) and SQLite (client) |
| PostgreSQL | Relational database backend for storing CRDT data with timestamps |
| SQLite | Relational database for client-side local storage |
| Proc macro (syn, quote, proc-macro2) | Code generation for parsing config and group definitions and generating outputs |

## Feature Components

### Table Configuration

The `tables` block defines table structures that can be reused across multiple
groups. Tables are defined within the `carburetor_sync_config!` macro block:

```rust
tables {
    phone_book(plural = "phone_books") {
        #[id]
        id -> Text,
        name -> Text,
        phone_number -> Text,
        note -> Nullable<Text>,
        #[is_deleted]
        record_deleted -> Boolean,
        #[column_sync_metadata]
        sync_metadata -> Jsonb,
        #[last_synced_at]
        updated_at -> Timestamptz
    }
}
```

**Component**:
- table arguments
    * `plural` (Optional): Defaults to `[table_name]` + `s`
- table field: This is similar to PostgreSQL type in `diesel::table!`
- column attribute
    * `#[id]` (Optional): Determine the unique sync ID of the table. Defaults to
      `id` and only accepts `Text` for type.
    * `#[last_synced_at]` (Optional): Determine the time of update to the
      server. Defaults to `last_synced_at` and only accepts `Timestamptz` for
      type.
    * `#[is_deleted]` (Optional): Marks the soft-delete flag column.
      Records are not physically removed to preserve sync information. When
      downloading from clean state, deleted records are filtered out. Defaults
      to `is_deleted` and only accepts `Boolean` for type.
    * `#[dirty_flag]` (Client-only, Optional): Stores the dirty status of the
      row, so that the system can quickly retrieve the necessary rows for
      updating. When dirty, the row is marked as "insert" or "update".
    * `#[client_column_sync_metadata]` (Client-only, Optional): Marks the column
      that stores sync metadata. This prevents incoming updates from overwriting
      dirty columns and blocks updates with older timestamps than existing data.
      Defaults to `column_sync_metadata` and only accepts `Jsonb` for type.

**Backend Generated Outputs** (PostgreSQL):
- Diesel table schema definition
- Select/insert/update models (Queryable, Insertable, AsChangeset)

**Client Generated Outputs** (SQLite):
- Diesel table schema definition (converted to SQLite-compatible data types)
- Select/insert/update models (Queryable, Insertable, AsChangeset)

### Sync Group Configuration

The `groups` block defines one or more sync groups. Each group specifies:
- Which tables to include

```rust
sync_groups {
    group_a {
        user,
        game,
    }
}
```

**Backend Generated Outputs**:
- Group download function and request/response model

**Client Generated Outputs**:
- Group sync to local SQLite database function and request/response model
- Group upload to server function (for dirty local changes) and request/response
  model
- Table modification functions: `insert_<item>`, `update_<item>`,
  `delete_<item>`
- Query helper: `active_<plural_item>()` for filtering non-deleted records

### Client Data Modification Functions

The macro generates helper functions for client-side data operations. These
functions handle the dirty flag management automatically.

**Generated Functions**:
- `insert_<table>(record)` - Inserts a new record and marks it as dirty (INSERT)
- `update_<table>(record)` - Updates an existing record and marks it as dirty
  (UPDATE), as well as specifying which column is dirty
- `delete_<table>(id)` - Soft-deletes a record by setting `is_deleted` to true
  and marks as dirty (UPDATE)
- `active_<table>()` - Returns a Diesel query filtered to non-deleted records
  for easier querying

**Usage Notes**:
- All modification functions automatically manage the `dirty_flag` column
- The `delete_<table>` function performs soft deletion, preserving the record
  for sync purposes
- Use `active_<table>()` to query only non-deleted records in your application
  logic

## Challenges and Considerations

### Non-Atomic Group Queries

The group download function executes independent queries for each table. This
means:
- Each table query runs separately against the database
- There is no transaction wrapping all queries together
- Data consistency across tables is not guaranteed at query time

This design choice prioritizes simplicity and performance over strict atomicity.
Applications requiring foreign key integrity guarantees should implement
additional validation logic or use a future integrity-checking feature.

### Independent Sync Tracking

Each table maintains its own `last_synced_at` tracking within each group. The
client application must:
- Store separate sync timestamps for each table in each group it uses
- Pass the correct timestamp for each table when calling the group download
  function
- Update each timestamp independently based on the corresponding response

### Soft Deletion

- Deleted records are marked with the `is_deleted` flag rather than physically removed
- This preserves sync information and prevents re-syncing deleted items
- When downloading with `None` offset (initial sync), deleted records are
  automatically filtered out to reduce unnecessary data transfer
- Subsequent incremental syncs include deleted records to properly propagate
  deletions to the client
- Deleted records cannot be undo to reduce complexity reasons. In such case, the
  record should be recreated.

### Client Sync Management

**Column Sync Metadata**:
- The `column_sync_metadata` field tracks per-column update timestamps and dirty flags
- Prevents incoming updates from overwriting locally modified (dirty) columns
- Rejects updates with older timestamps than the existing column data
- Enables granular conflict resolution at the column level rather than row level

### Config vs Group Design Decisions

When designing your sync architecture, consider:

**Create separate configs when**:
- Tables are completely unrelated
- Different applications have no overlapping data needs
- You want complete isolation between different parts of your application

**Create multiple groups within one config when**:
- Different applications sync different subsets of the same tables
- You need different filtering on the same tables for different use cases
- You want to share table definitions but provide different sync behaviors

Example: A game application might use one config with three groups:
- Mobile app group (limited data, published content only)
- Web app group (full game data, no admin features)
- Admin dashboard group (complete access including audit logs)

### Time Synchronization Issues

**Backend and Postgres wall clock time difference**
- When we perform an incremental download in a distributed environment, correct
  timing is especially important, because a small difference can potentially
  mean missing records sent to the client.
- Backend server is not a reliable way of specifying time because if we have
  more than one backend server, the time will not be synchronized properly,
  therefore, we use Postgres time to handle the time for `last_synced_at`
  column.

**Client Metadata `dirty_at` time != `last_synced_at` time**
- Another thing to note is that while both `dirty_at` and `last_synced_at` uses
  UTC time, `dirty_at` is generated by client for client to keep track of upload
  related sync.
- Note that because `dirty_at` uses client's clock, there's a chance that data
  might not be uploaded if client's clock is changed. But this is a known
  decision because the chance of data loss is low and the current implementation
  is much easier to handle compared to changing `dirty_at` into a vector clock.
