# Basic Feature

## Overview

Carburetor is a local-first framework that enables sync between a storage
backend and multiple frontend devices with LWW (Last Writes Win) Register.

It provides sync-config feature provides a function-like proc macro
`carburetor_sync_config!` that defines table configurations and multiple sync
groups. This enables developers to:

- Define table structures once and reuse them across multiple groups
- Create different sync groups for different applications or use cases
- Synchronize different subsets of tables with different filtering per group
- Maintain independent sync tracking for each table within each group

## Core Implementation Library/Framework/Tool

| Library/Framework/Tool | Purpose |
|---|---|
| Diesel | Database abstraction and ORM for PostgreSQL queries |
| PostgreSQL | Relational database backend for storing CRDT data with timestamps |
| Proc macro (syn, quote, proc-macro2) | Code generation for parsing config and group definitions and generating outputs |

## Feature Components

### Table Configuration

The `tables` block defines table structures that can be reused across multiple
groups. Each struct receives the standard `#[carburetor]` macro expansion:

```rust
tables {
    phone_book(plural = "phone_books") {
        #[id]
        id -> Text,
        name -> Text,
        phone_number -> Text,
        note -> Nullable<Text>,
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
      `id`
    * `#[last_synced_at]` (Optional): Determine the time of update to the
      server. Defaults to `last_synced_at`

**Generated outputs per table**:
- Diesel table schema definition
- Select/insert/update models (Queryable, Insertable, AsChangeset)

### Sync Group Definitions

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



**Generated outputs per group**:
- Group download function and response model

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
