# `sync_groups` block of `carburetor_sync_config!`

## Signature

```rust
carburetor_sync_config! {
    sync_groups {
        group_name {
            table1,
            table2,
        },
        another_group {
            table2,
        },
    }
}
```

## Example Usage

```rust
carburetor_sync_config! {
    tables {
        user { /* table definition */ },
        post { /* table definition */ },
    }
    sync_groups {
        user_data {
            user,
            post,
        },
    }
}
```

Each group is declared as `group_name { table1, table2, ... }`: an identifier
followed by a brace block of table names, comma-separated. Each entry is a
table name from the `tables` block; a table may appear in multiple groups.

## Applicability

- Each block can be defined at most once. A second `sync_groups` block fails
  macro expansion with `` `sync_groups` can only be defined once ``.
- Each sync group (sync client) must target its own data source. Running two
  groups' `run_migrations` against one database is not the intended design.

## Effect on generated code

The `sync_groups` block is what triggers the sync functions to appear. Each
group generates the group-level sync functions: on the client,
`retrieve_download_request`, `store_download_response`, `retrieve_upload_request`,
and `store_upload_response`; on the backend, `process_download_request` and
`process_upload_request`. It also generates the per-table functions and their
request and response models. Without a group, a table declared in `tables` gets
none of these.

Each group name becomes a module (`pub mod <group_name>`) that contains the
generated functions and models for the group's tables. Details of those outputs
are documented in [table.md](table.md).

With the `migration` feature enabled, each group also generates a
`run_migrations` function on the client, inside the group's module. It covers
only the tables in that group, plus `carburetor_offsets`.

## Usage notes

- Declare every table in the `tables` block before referencing it in a group;
  ordering inside the macro matters. Known issue: making block order irrelevant
  is tracked in
  [chunleng/carburetor-rs#32](https://github.com/chunleng/carburetor-rs/issues/32).
- A table can belong to several groups; each group syncs it independently.
- Give each group its own database and call its `run_migrations` against that
  database only:

  ```rust
  // One database per sync group
  user_data::run_migrations(&user_db_conn)?;
  admin_data::run_migrations(&admin_db_conn)?;
  ```
