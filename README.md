# Carburetor

Rust library for building local-first app with LWW register CRDT.

## Features

TODO

needs cargo crate:
- diesel = { features = ["postgres", "chrono"] }
- serde = { features = ["derive"] } -> Resolvable with crate renaming

- Minimum diesel version: 2.3.0: need `#[diesel(skip_update)]` `jsonb` type

## Flow

Download:

1. Client: retrieve_download_request
2. serde across network
3. Backend: process_download_request
4. serde across network
5. Client: store_download_response

Upload:

1. Client: retrieve_upload_request
2. serde across network
3. Backend: process_upload_request
4. serde across network
5. Client: store_upload_response

### Workaround

- `rust-analyzer` picks up and unify features of Cargo.toml
