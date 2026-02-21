# Filter Row by Condition

## Overview

This feature introduces two mechanisms that together enable fine-grained,
context-aware access control over what data is synced between the backend and
clients.

The first mechanism is the `#[immutable]` column attribute, which marks
user-defined columns as read-only after their initial value is set. This
prevents any incoming update from overwriting those columns during sync.

The second mechanism is the `restrict_to` and `restrict_to_column` attributes
on sync group table entries. When applied, they narrow the scope of a sync group
so that only rows matching a given context value are visible to download and
writable during upload. The context is provided at runtime through a generated
`SyncContext` type, making the feature flexible enough to serve any row-level
filtering need — the most prominent use case being per-user data isolation in
authenticated applications.

## Core Implementation Library/Framework/Tool

This feature works with the same set of tools utilized in [basic
feature](./basic-feature.md).

## Feature Components

### Immutable Column Attribute

The `#[immutable]` attribute can be applied to any regular data column in a
table definition. It signals that once a row is created, that column's value
must never be changed by an incoming sync update.

```rust
tables {
    note(plural = "notes") {
        #[immutable]
        owner_id -> Text,
        content -> Text,
    }
}
```

**Constraints**:
- Can only be applied to non-special columns. The special columns (`#[id]`,
  `#[last_synced_at]`, `#[is_deleted]`, `#[dirty_flag]`,
  `#[client_column_sync_metadata]`) already have their mutability determined
  implicitly: `id` is always immutable, and the rest follow their own
  system-managed rules.
- Applying `#[immutable]` to a special column results in a compile-time error.

**Effect on sync**:
- On the backend, an `Update` request that includes a value for an immutable
  column will have that column silently ignored; the persisted value is never
  overwritten.
- The `AsChangeset` model generated for the table omits immutable columns, so
  they cannot participate in `UPDATE` statements.

### Sync Group Row Filtering

Two new attributes on sync group table entries control row-level access:

- `restrict_to`: The name of the field on the generated `SyncContext` struct
  that holds the filtering value.
- `restrict_to_column`: The table column whose value must equal the context
  field value for a row to be included.

Both attributes must be specified together; providing one without the other is a
compile-time error.

```rust
sync_groups {
    per_user_notes {
        note(
            restrict_to = $user_id,
            restrict_to_column = owner_id,
        )
    }
}
```

#### Generated `SyncContext`

When any table in a sync group declares `restrict_to`, the macro generates a
`SyncContext` struct for that group. Each unique `restrict_to` variable becomes
a field on the struct. The type of the field matches the Rust type of the
corresponding `restrict_to_column`.

For the example above, the generated struct would resemble:

```rust
pub struct SyncContext {
    pub user_id: String,
}
```

#### Effect on `process_download_request`

The generated `process_download_request` function gains an additional `context:
SyncContext` parameter. Internally, for each restricted table, an equality
filter is added to the query so that only rows where `<restrict_to_column> =
context.<restrict_to>` are returned.

```rust
// Generated signature (backend, with restriction)
pub fn process_download_request(
    request: Option<DownloadRequest>,
    context: SyncContext,
) -> carburetor::error::Result<DownloadResponse>
```

#### Effect on `process_upload_request`

The generated `process_upload_request` function also gains the `context:
SyncContext` parameter. Before applying any insert or update, the backend
validates that the value of the restricted column in the incoming record matches
the corresponding context field. Records that fail this check are rejected with
an appropriate error response for that row.

```rust
// Generated signature (backend, with restriction)
pub fn process_upload_request(
    request: UploadRequest,
    context: SyncContext,
) -> carburetor::error::Result<UploadResponse>
```

#### Groups without restrictions

When a sync group has no `restrict_to` / `restrict_to_column` attributes on any
of its tables, no `SyncContext` struct is generated for that group and the
signatures of `process_download_request` and `process_upload_request` remain
unchanged from the basic feature.

### Usage Example

A typical authenticated scenario: each user may only sync their own notes.

```rust
carburetor_sync_config! {
    tables {
        note(plural = "notes") {
            #[immutable]
            owner_id -> Text,
            content -> Text,
        }
    }
    sync_groups {
        per_user_notes {
            note(
                restrict_to = "user_id",
                restrict_to_column = "owner_id",
            )
        }
    }
}
```

On the backend, the caller constructs a `SyncContext` from the authenticated
session and passes it to the generated functions:

```rust
let context = per_user_notes::SyncContext {
    user_id: authenticated_user_id,
};

let response = per_user_notes::process_download_request(request, context)?;
```

Because `SyncContext` is a plain struct with no internal validation, any value
can be supplied. The application is responsible for populating it from a trusted
source such as a verified session token or middleware-resolved identity.

## Challenges and Considerations

### SyncContext Is Not Self-Enforcing

`SyncContext` carries no authentication logic itself. If an incorrect or
forged value is passed (for example, due to a missing authentication middleware
layer), the filter will silently allow or deny rows based on that wrong value.
Applications must ensure that the context is populated from a trusted,
server-side source before calling the generated functions.

### Immutability Is Enforced Only During Sync

The `#[immutable]` guarantee applies to the sync path. Direct database writes
that bypass the generated functions are not covered. Applications that allow
raw database access must enforce their own immutability constraints at the
database level (e.g., via triggers or application-layer guards) if that
protection is required outside of sync.

### Single Column Restriction per Table

Each `restrict_to` / `restrict_to_column` pair ties one context field to one
column for each table. The current design decision disables multiple condition
on a single table because it makes things complex and there aren't any strong
use case to support it right now.
