# AGENTS.md

## Overview

**Carburetor** - Rust library for local-first applications with LWW CRDT
support. Provides timestamp-based incremental sync between PostgreSQL backend
and frontend devices.

**Key Concepts**: LWW CRDT (latest write wins) • Incremental sync • Code
generation via procedural macros • Offline-first with eventual consistency

**Tech Stack**: Rust 2024 • PostgreSQL/Diesel • Timestamp-based queries • Proc
macros (`syn`, `quote`)

## Workspace

```
carburetor-rs/
├── carburetor/                  # Runtime library
│   └── src/
│       ├── helpers/             # Utility modules
│       ├── config.rs            # Config singleton
│       ├── error.rs             # Error types
│       ├── models.rs            # DownloadTableResponse, sync models
│       └── lib.rs
├── carburetor-macro/            # Proc macro crate
│   └── src/
│       ├── parsers/             # Input parsing
│       ├── generators/          # Code generation
│       └── helpers/             # Shared macro utilities
├── docs/
│   └── features/
│       └── basic-feature.md     # Core feature documentation
├── examples/                    # Usage demos (simple-backend, simple-client)
└── tests/
    ├── e2e-test/                # E2E test suite (client feature, tarpc RPC client)
    ├── sample-test-backend/     # Standalone backend binary for E2E tests (tarpc RPC server)
    └── sample-test-core/        # Shared schema crate (supports both backend/client features)
```

## Module Details

### carburetor (Runtime)

- **config**: Global configuration singleton for database connections
- **error**: `Error` and `Result` types for the library
- **models**: `DownloadTableResponse` for sync payloads
- **helpers**: `get_connection()`, `get_utc_now()`, `CarburetorOffset`,
  `ClientSyncMetadata`, etc

### carburetor-macro (Proc Macro)

**parsers/**: Converts macro input into internal representations
**generators/**: Produces Rust code from parsed structures

## Sync Flow Architecture

**Two-way sync between PostgreSQL backend and SQLite clients:**

**Download (Backend → Client)**:
Backend generates `download_<group>(offsets)` function that queries each table
for records where `last_synced_at > offset`. Returns a `DownloadResponse` with
records and new offsets per table. Client calls `store_download_response()` to
merge server data into local SQLite DB using LWW conflict resolution.

**Upload (Client → Backend)**:
Client tracks dirty records using `dirty_flag` (INSERT/UPDATE) and
`client_column_sync_metadata` (per-column timestamps). Client calls
`upload_<group>()` to send dirty records to backend. Backend's
`upload_<group>(request)` applies LWW merge using column timestamps and returns
response. Client clears dirty flags after successful upload.

**Local Operations (Client)**:
Clients work offline using generated per-table functions: `insert_<table>()`,
`update_<table>()`, and `delete_<table>()` automatically set dirty flags.
`active_<plural>()` provides query helpers that filter out soft-deleted records.

## Commands

```bash
# Testing e2e
# Note: Tests share a single SQLite DB singleton; --test-threads=1 is required
# Note: Build sample-test-backend first so that it won't spend too much time
#       launching the server and cause test to fail
cargo build -p sample-test-backend --features=backend && cargo test -p e2e-test --features=client -- --test-threads=1

# Building & Running - Backend
cargo check --features backend
cargo run --example simple-backend --features backend
cargo expand --example simple-backend --features backend

# Building & Running - Client
cargo check --features client
cargo run --example simple-client --features client
cargo expand --example simple-client --features client
```

## Common Pitfalls

### Feature Flags

The `carburetor_sync_config` macro requires exactly one feature flag (`client`
or `backend`) to be enabled, never both simultaneously. Enabling both will
result in compilation errors.

### Soft Deletion

Records are never physically deleted. The `is_deleted` flag marks records as
deleted while preserving sync information. Always use `active_<plural>()` to
query non-deleted records in application logic.

### Time Synchronization

- **Backend**: Uses PostgreSQL server time for `last_synced_at` to avoid clock
  skew between multiple backend instances
- **Client**: Uses local device time for `dirty_at` in metadata. Clock changes
  on the device may cause sync issues (acceptable trade-off for simplicity)
- **Incremental sync**: Small timing differences can cause missed records; the
  system relies on PostgreSQL time as the source of truth

### Column-Level Conflict Resolution

The `client_column_sync_metadata` tracks per-column timestamps. During sync:
- Incoming updates with older timestamps than local data are rejected per-column
- Locally dirty columns are not overwritten by incoming server data
- This enables granular LWW at column level, not just row level

### Non-Atomic Group Queries

Download queries for each table in a group run independently (no transaction
wrapping). Foreign key integrity across tables is not guaranteed at query time.
The `cutoff_at` timestamp parameter helps mitigate this by ensuring all tables
in the sync group use the same time cutoff, reducing the window where foreign
key relationships might be temporarily broken during download.

## Documentation

- @docs/features/basic-feature.md - Core concepts, table configuration, sync
  groups, and design decisions
- @docs/development/e2e-testing.md - Test infrastructure, tarpc RPC
  architecture, and test isolation

## Guidelines

### Code Style

- Follow Rust 2024 edition idioms
- Use `syn` and `quote` for proc macro implementation
- Prefer explicit error handling with `carburetor::error::Result`

### Testing

- E2E tests use a tarpc RPC backend server + testcontainers PostgreSQL for isolated integration testing
The shared SQLite singleton requires sequential test execution (`--test-threads=1`)
- Examples serve as integration tests for end-to-end functionality

### Development Flow

When developing to add new features or resolve issues with the codebase:

1. Update parsers if new syntax is needed
2. Update generators for new code output
3. Note that both backend and client feature should individually compile and
   error free
4. Utilize carburetor helpers to reduce code generation
5. Update test case in `tests/e2e-test/`
6. Update documentation in `docs/features/`
