# `#[immutable]` column attribute

Marks a data column as read-only after its initial value. Incoming sync updates
never overwrite the column.

## Signature

```rust
#[immutable]
```

## Example Usage

```rust
carburetor_sync_config! {
    tables {
        user {
            username -> Text,
            #[immutable]
            created_by -> Text,      // valid: non-special data column
            display_name -> Text,    // mutable: no attribute
        }
    }
}
```

## Applicability

- `#[immutable]` applies only to non-special data columns. Applying it to a
  column declared with `#[id]`, `#[last_synced_at]`, `#[is_deleted]`,
  `#[dirty_flag]`, or `#[client_column_sync_metadata]` fails compilation

## Effect on generated code

- Backend: immutable columns are omitted from the generated `AsChangeset`
  implementation, so backend-side updates through the generated code cannot
  modify them.
- Upload: immutable columns are excluded from upload changesets; a client never
  sends new values for them.
- Client local updates: immutable columns are skipped by client local-update
  metadata tracking; local updates do not record sync metadata for them.

Example: with `created_by` marked `#[immutable]`, an upload of a `user` row
whose `display_name` changed sends only `display_name` (plus sync metadata
columns); `created_by` is never included.

## Usage notes

Immutability is enforced on the sync path only (upload changesets, backend
changesets, client metadata tracking). Direct database writes are not
restricted.
