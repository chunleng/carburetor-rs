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
├── carburetor/          # Runtime: config, errors, sync models, DB helpers
├── carburetor-macro/    # Proc macro: parsers, generators, trybuild tests
├── examples/            # simple_backend.rs - complete usage demo
└── docs/                # basic-feature.md, sync-group.md (unimplemented)
```

**carburetor**: Config singleton • Error types • `DownloadSyncResponse` •
`get_connection`, `get_utc_now`

**carburetor-macro**: `#[carburetor]` attribute generates Diesel schemas, model
structs (Queryable/Insertable/AsChangeset), and sync functions

## Commands

```bash
# Testing
cargo test --package carburetor-macro --features=backend

# Building
cargo check --features backend
cargo run --example simple-backend --features backend
cargo expand --package carburetor-example
```

## Guidelines

1. Comments explain "why", not "what"
2. Files end with newline
3. Use workspace dependencies
