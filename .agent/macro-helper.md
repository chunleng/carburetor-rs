# Adding a Macro Helper

Detailed guide for moving complex logic out of generated code (see
`AGENTS.md` → "Macro Code Generation & Helpers" for the rationale).

## How generated code calls helpers

Generated code references helpers by their public path
`carburetor::helpers::<module>::<fn>`, which resolves in the consuming crate at
compile time (generated code always depends on the `carburetor` crate). Gate a
helper with `#[cfg(for_client)]` or `#[cfg(for_backend)]` to match where the
generator that uses it runs.

## Worked example

A pattern repeated in the generated upload code is checking whether a record is
marked dirty for upload:

```rust
// carburetor-macro/src/generators/upload/functions.rs — logic inlined in quote!:
quote! {
    match record.#dirty_flag_column {
        Some(ref f) if
            f == &carburetor::helpers::client_sync_metadata::DirtyFlag::Insert.to_string() ||
            f == &carburetor::helpers::client_sync_metadata::DirtyFlag::Update.to_string() => { /* ... */ }
        _ => {}
    }
}
```

Move that logic into a helper so the generator emits a short call instead.

### Step 1 — Write the helper. `carburetor/src/helpers/dirty_flag.rs`:

```rust
use crate::helpers::client_sync_metadata::DirtyFlag;

/// True if the dirty flag marks the record as needing upload.
pub fn is_dirty(flag: &Option<String>) -> bool {
    flag.as_deref().map_or(false, |f| {
        f == DirtyFlag::Insert.to_string() || f == DirtyFlag::Update.to_string()
    })
}
```

### Step 2 — Register the module in `carburetor/src/helpers/mod.rs`:

```rust
#[cfg(for_client)]
pub mod dirty_flag;
```

### Step 3 — Reference it from the macro generator:

```rust
// carburetor-macro/src/generators/upload/functions.rs — generated code delegates to the helper:
quote! {
    if carburetor::helpers::dirty_flag::is_dirty(&record.#dirty_flag_column) {
        /* ... */
    }
}
```
