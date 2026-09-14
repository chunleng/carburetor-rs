# `restrict_to` and `restrict_to_column` sync group arguments

Filter a sync group's rows by a context value supplied at call time. Each table
entry in a sync group can declare a `restrict_to` / `restrict_to_column` pair;
the macro generates a `SyncContext` struct for the group and the generated
download and upload functions take it as a parameter.

## Signature

```rust
carburetor_sync_config! {
    sync_groups {
        per_user_notes {
            message(restrict_to = $user_id, restrict_to_column = recipient_id)
        }
    }
}
```

- `restrict_to = $name`: name of a context variable. The value must be prefixed
  with `$`.
- `restrict_to_column = column`: name of a column in the same table.

## Example Usage

```rust
carburetor_sync_config! {
    tables {
        message {
            #[id]
            id -> Text,
            recipient_id -> Text,
            body -> Text,
        }
    }
    sync_groups {
        per_user_notes {
            message(restrict_to = $user_id, restrict_to_column = recipient_id)
        }
    }
}
```

## Applicability

- Both arguments must be declared together. Declaring one without the other
  fails macro expansion.
- `restrict_to` must be a dollar-prefixed variable name. A plain identifier
  fails.
- `restrict_to_column` must be a plain column name. The column must exist in
  the same table and must be marked `#[immutable]`.
- One restriction pair per table entry. Declaring `restrict_to` or
  `restrict_to_column` twice on the same table entry, or passing an unknown
  argument, fails macro expansion. Multiple filter conditions per table are not
  supported.
- All `restrict_to` declarations referencing context variables of the same name
  in one sync group resolve to a single `SyncContext` field. Its type comes from
  the first referenced column's mapping; a type mismatch across referenced
  columns fails macro expansion with an error naming the context variable and
  both conflicting types.

## Effect on generated code

- `SyncContext`: one struct per sync group, generated only when at least one
  table in the group declares `restrict_to`. Fields are the unique context
  variables, typed through the referenced column's type mapping (`Text` ->
  `String`, `Integer` -> `i32`, `Nullable<T>` -> `Option<T>`, and so on). The
  struct derives `Debug`, `Clone`, `Serialize`, `Deserialize` and lives in the
  sync group's module, for example `per_user_notes::SyncContext`.
- `process_download_request`: gains a `context: &SyncContext` parameter. Each
  restricted table's query gains an equality filter on the referenced column,
  `col.eq(&context.field)`.
- `process_upload_request`: gains a `context: &SyncContext` parameter. For an
  incoming `Insert` record, the restricted column value in the incoming data is
  compared against the matching context field; for an `Update` record, the
  existing row's restricted column value in the database is compared instead (a
  missing existing row is rejected with `RecordNotFound`). A mismatching record
  is rejected per row with `UploadTableResponseError { id, code:
  UploadTableResponseErrorType::InsufficientPermission }`.
- Groups without any `restrict_to`: no `SyncContext` is generated and the
  download and upload function signatures are unchanged.

## Usage notes

`SyncContext` is a plain struct with no validation. The application must
populate it from a trusted source before calling the generated download and
upload functions.

Also, take note that the generated functions take the `SyncContext` by reference
to restrict what can be read from or write to the database:

```rust
let context = SyncContext { user_id: user_id };

// Returns only `message` rows where `recipient_id` equals `context.user_id`.
let response = process_download_request(request, &context);

// Rejects any `message` record whose `recipient_id` differs from
// `context.user_id` with `InsufficientPermission`.
let response = process_upload_request(upload_request, &context);
```
