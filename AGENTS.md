# AGENTS.md

## Overview

Rust LWW CRDT lib for local-first apps. See Cargo.toml + README.md.

## Test Structure

E2E tests in `tests/e2e-test/tests/` follow pattern: `edge_cases/` (special
conditions), `happy_paths/` (normal ops), `unhappy_paths/` (error handling). New
tests → match existing folder structure. Uses `sample-test-backend/` (RPC
server) + `sample-test-core/` (shared schema).

## Sync Flow Architecture

**Two-way sync: PostgreSQL backend ↔ SQLite clients**

**Download (Backend → Client)**:
Backend generates `download_<group>(offsets)` → queries each table for
`last_synced_at > offset`. Returns `DownloadResponse` with records + new offsets
per table. Client calls `store_download_response()` → merges server data into
local SQLite DB using LWW conflict resolution.

**Upload (Client → Backend)**:
Client tracks dirty records via `dirty_flag` (INSERT/UPDATE) +
`client_column_sync_metadata` (per-column timestamps). Client calls
`upload_<group>()` → sends dirty records to backend. Backend's
`upload_<group>(request)` applies LWW merge using column timestamps → returns
response. Client clears dirty flags after successful upload.

**Local Operations (Client)**:
Clients work offline using generated per-table functions: `insert_<table>()`,
`update_<table>()`, `delete_<table>()` automatically set dirty flags.
`active_<plural>()` provides query helpers that filter out soft-deleted records.

## Commands

### Test & Check

```bash
# E2E testing
# Tests share a single SQLite DB guarded by a mutex; tests run in parallel.
# Build sample-test-backend first: tests spawn the pre-built binary directly
cargo build -p sample-test-backend && CARBURETOR_TARGET=client cargo test -p e2e-test

# Backend
cargo build -p carburetor --features=diesel/postgres

# Client
CARBURETOR_TARGET=client cargo build -p carburetor --features=diesel/sqlite --features=migration
```

### Other Useful Commands

```bash
# Backend
cargo run --example simple-backend --features backend
cargo expand --example simple-backend --features backend

# Client
CARBURETOR_TARGET=client cargo run --example simple-client --features client
CARBURETOR_TARGET=client cargo expand --example simple-client --features client
```

## Common Pitfalls

### Soft Deletion

Records never physically deleted. `is_deleted` flag marks records as deleted
while preserving sync info. Always use `active_<plural>()` to query non-deleted
records.

### Time Synchronization

- **Backend**: Uses PostgreSQL server time for `last_synced_at` → avoids clock
  skew between backend instances
- **Client**: Uses local device time for `dirty_at` in metadata. Clock changes
  on device → sync issues (acceptable trade-off)
- **Incremental sync**: Small timing differences → missed records; PostgreSQL
  time = source of truth

### Column-Level Conflict Resolution

`client_column_sync_metadata` tracks per-column timestamps. During sync:
- Incoming updates with older timestamps than local data → rejected per-column
- Locally dirty columns → not overwritten by incoming server data
- Enables granular LWW at column level, not just row level

### Test DB Mutex Ordering

Tests share a single SQLite DB guarded by a `std::sync::Mutex`. When a test uses
both the backend and the client DB, **start the backend server before acquiring
the DB lock**. Otherwise the test holds the lock during container startup,
serializing other tests that are waiting for the DB:

```rust
let backend_server = TestBackendHandle::start();
let backend = backend_server.client().await;
let db = get_clean_test_client_db();
let mut conn = db.get_connection();
```

### Non-Atomic Group Queries

Download queries for each table in group run independently (no transaction). FK
integrity across tables not guaranteed at query time. `cutoff_at` timestamp
parameter ensures all tables in sync group use same time cutoff → reduces window
where FK relationships temporarily broken during download.

## Macro Code Generation & Helpers

`carburetor-macro` generates sync code inline via `quote!`. Logic embedded in a
`quote!` block is hard to read and untestable. **The purpose of
`carburetor/src/helpers/` is to host runtime helper functions that generated code
calls, so that complex logic moves out of `quote!` into ordinary, testable Rust
functions.** Extracting logic here is the primary way to simplify code generation
— when a `quote!` block becomes verbose or repeats across generators, move that
logic into a helper and replace the inline code with a call to it.
For a step-by-step example of adding and using a helper, see
[`.agent/macro-helper.md`](.agent/macro-helper.md).
