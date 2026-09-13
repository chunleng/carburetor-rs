# Tutorial: Configure sync end-to-end

## Goal

By the end of this tutorial you will have a minimal local-first app consisting of two binaries:

- a **backend** that stores data in PostgreSQL, and
- a **client** that works offline against a local SQLite database,

both generated from a single `carburetor_sync_config!` schema declaration. You will run one full sync round trip: a record created offline on the client is uploaded to the backend, then downloaded back, with last-write-wins conflict resolution handled for you.

## Prerequisites

- Rust MSRV 1.88 or newer
- A running PostgreSQL server you can connect to
- The `psql` and `sqlite3` command line tools
- No prior carburetor experience is assumed. Basic familiarity with Rust and SQL is enough.

## Step 1: Create the project

Carburetor generates different code for the backend (PostgreSQL) and the client (SQLite), so the cleanest setup is a Cargo workspace with one crate per side:

```text
my-app/
├── Cargo.toml          # workspace file
├── sync/               # created later: stand-in for the network
├── backend/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       └── schema.rs
└── client/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        └── schema.rs
```

Create the directories and the workspace `Cargo.toml`:

```toml
# my-app/Cargo.toml
[workspace]
resolver = "2"
members = ["backend", "client"]
```

## Step 2: Add dependencies

The backend crate uses PostgreSQL, the client crate uses SQLite. Note the extra SQLite feature `returning_clauses_for_sqlite_3_35`, which carburetor's client code requires.

```toml
# my-app/backend/Cargo.toml
[package]
name = "my-app-backend"
version = "0.1.0"
edition = "2024"

[dependencies]
carburetor = "0.2.0"
diesel = { version = "2.2", features = ["postgres"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

```toml
# my-app/client/Cargo.toml
[package]
name = "my-app-client"
version = "0.1.0"
edition = "2024"

[dependencies]
carburetor = "0.2.0"
diesel = { version = "2.2", features = ["sqlite", "returning_clauses_for_sqlite_3_35"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

The same macro generates backend or client code depending on the `CARBURETOR_TARGET` environment variable, which is read at **compile time** (it defaults to `backend`). Pass it inline on every command that builds the client crate:

```bash
CARBURETOR_TARGET=client cargo build -p my-app-client
```

> The crate does not compile yet: `src/main.rs` is created in Step 5. You will run this command from Step 5 on.

## Step 3: Declare the schema

Create the identical `schema.rs` in both `backend/src/` and `client/src/`:

```rust
use carburetor::prelude::*;

carburetor_sync_config! {
    tables {
        user {
            username -> Text,
            first_name -> Nullable<Text>,
            joined_on -> Date,
        }
    }
    sync_groups {
        all_clients {
            user
        }
    }
}
```

Two things to know about this declaration:

- You only declare your **data columns**. Carburetor automatically adds the sync bookkeeping columns (`id`, `last_synced_at`, `is_deleted`, `dirty_flag`, `column_sync_metadata`) to every table. You will see them again in the next step, because your SQL tables must contain them.
- `sync_groups` defines a named group, `all_clients`, that syncs the `user` table. The group name becomes the module your sync functions live in.

The full syntax for tables, columns, and sync groups is covered in the reference: [table](../reference/sync-config/table.md) and [sync-group](../reference/sync-config/sync-group.md).

## Step 4: Create the tables

In this tutorial, you create the database tables yourself with SQL. Carburetor can also create and migrate them automatically; see the how-to guides. Either way, the tables must match the declared schema exactly (column names, types, and nullability are validated during sync).

Create the database and table on the backend side (PostgreSQL):

```bash
psql "postgres://postgres:password@localhost:5432/postgres" -c "CREATE DATABASE my_app"
psql "postgres://postgres:password@localhost:5432/my_app" -c "
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    first_name TEXT,
    joined_on DATE NOT NULL,
    last_synced_at TIMESTAMPTZ NOT NULL,
    is_deleted BOOLEAN NOT NULL
);"
```

Create the tables on the client side (SQLite). Run this from the `my-app/` root, the same directory you will run the client binary from, so the database file lands where the client expects it:

```bash
sqlite3 default.db "
CREATE TABLE users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    first_name TEXT,
    joined_on DATE NOT NULL,
    last_synced_at TIMESTAMPTZ,
    is_deleted BOOLEAN NOT NULL,
    dirty_flag TEXT,
    column_sync_metadata JSON NOT NULL
);
CREATE TABLE carburetor_offsets (
    table_name TEXT PRIMARY KEY,
    cutoff_at TIMESTAMPTZ NOT NULL
);"
```

Differences worth noticing:

- The backend table has no `dirty_flag` or `column_sync_metadata`. Those track client-side sync state and never exist on the backend.
- `last_synced_at` is `NOT NULL` on the backend but nullable on the client.
- The client needs the extra `carburetor_offsets` table, which stores per-table download offsets.

## Step 5: Initialize the global config

Each binary must call `initialize_carburetor_global_config` exactly once at startup (a second call panics). The backend config takes a `database_url`, the client config takes a `database_path`.

```rust
// my-app/backend/src/main.rs
mod schema;

use carburetor::config::{initialize_carburetor_global_config, CarburetorGlobalConfig};
use diesel::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/my_app".to_string());
    let _connection = diesel::pg::PgConnection::establish(&database_url)
        .expect("Error connecting to PostgreSQL");
    initialize_carburetor_global_config(CarburetorGlobalConfig { database_url });

    // Sync code comes in the next step
    Ok(())
}
```

```rust
// my-app/client/src/main.rs
mod schema;

use carburetor::config::{initialize_carburetor_global_config, CarburetorGlobalConfig};
use diesel::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_path =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./default.db".to_string());
    let _connection = diesel::sqlite::SqliteConnection::establish(&database_path)
        .expect("Error connecting to SQLite");
    initialize_carburetor_global_config(CarburetorGlobalConfig { database_path });

    // Sync code comes in the next step
    Ok(())
}
```

Run both binaries once to verify the setup compiles and connects:

```bash
cargo run -p my-app-backend
CARBURETOR_TARGET=client cargo run -p my-app-client
```

Both should exit cleanly.

## Step 6: Run the first sync

The generated sync functions work on serde-serializable request/response types, so the transport between client and backend is your choice (HTTP, WebSocket, anything). In this tutorial we use JSON files in a `sync/` directory as a stand-in for the network, so you can see every message in plain text.

Create the directory:

```bash
mkdir sync   # from my-app/
```

Replace the client `main.rs` with a two-mode program: `push` sends local changes up, `pull` receives backend changes down.

```rust
// my-app/client/src/main.rs
mod schema;

use carburetor::chrono::{DateTimeUtc, NaiveDate};
use carburetor::config::{initialize_carburetor_global_config, CarburetorGlobalConfig};
use diesel::prelude::*;

use crate::schema::all_clients;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_path =
        std::env::var("DATABASE_PATH").unwrap_or_else(|_| "./default.db".to_string());
    let mut connection = diesel::sqlite::SqliteConnection::establish(&database_path)
        .expect("Error connecting to SQLite");
    initialize_carburetor_global_config(CarburetorGlobalConfig { database_path });

    match std::env::args().nth(1).as_deref() {
        Some("push") => push(),
        Some("pull") => pull(&mut connection),
        other => {
            eprintln!("usage: my-app-client push|pull (got {other:?})");
            std::process::exit(1);
        }
    }
}

fn push() -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all("sync")?;

    // Make a local change while offline. Dirty flags are set automatically.
    all_clients::insert_user(all_clients::InsertUser {
        username: "alice".to_string(),
        first_name: Some("Alice".to_string()),
        joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    })?;

    // Ask the backend what to upload and what to download
    let (cutoff_at, upload_request) = all_clients::retrieve_upload_request()?;
    let download_request = all_clients::retrieve_download_request()?;

    // "Send" them to the backend
    std::fs::write("sync/cutoff_at.json", serde_json::to_string(&cutoff_at)?)?;
    std::fs::write("sync/upload_request.json", serde_json::to_string(&upload_request)?)?;
    std::fs::write("sync/download_request.json", serde_json::to_string(&download_request)?)?;
    println!("requests written to sync/");
    Ok(())
}

fn pull(connection: &mut diesel::sqlite::SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
    // "Receive" the backend's responses and store them. This is where
    // last-write-wins merging into the local database happens.
    let cutoff_at: DateTimeUtc =
        serde_json::from_str(&std::fs::read_to_string("sync/cutoff_at.json")?)?;
    let upload_response: all_clients::UploadResponse =
        serde_json::from_str(&std::fs::read_to_string("sync/upload_response.json")?)?;
    all_clients::store_upload_response(cutoff_at, upload_response)?;

    let download_response: all_clients::DownloadResponse =
        serde_json::from_str(&std::fs::read_to_string("sync/download_response.json")?)?;
    all_clients::store_download_response(download_response)?;

    // Verify: list non-deleted users
    let users = all_clients::active_users()
        .select(all_clients::FullUser::as_select())
        .load::<all_clients::FullUser>(connection)?;
    println!("{users:#?}");
    Ok(())
}
```

Replace the backend `main.rs` with a program that processes both requests:

```rust
// my-app/backend/src/main.rs
mod schema;

use carburetor::config::{initialize_carburetor_global_config, CarburetorGlobalConfig};
use diesel::prelude::*;

use crate::schema::all_clients;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:password@localhost:5432/my_app".to_string());
    let _connection = diesel::pg::PgConnection::establish(&database_url)
        .expect("Error connecting to PostgreSQL");
    initialize_carburetor_global_config(CarburetorGlobalConfig { database_url });

    // Upload: apply the changes the client sent up
    let upload_request: all_clients::UploadRequest =
        serde_json::from_str(&std::fs::read_to_string("sync/upload_request.json")?)?;
    let upload_response = all_clients::process_upload_request(upload_request)?;
    std::fs::write("sync/upload_response.json", serde_json::to_string(&upload_response)?)?;

    // Download: answer "what changed since my last sync?"
    let download_request: Option<all_clients::DownloadRequest> = std::fs::read_to_string(
        "sync/download_request.json",
    )
    .ok()
    .map(|s| serde_json::from_str::<Option<all_clients::DownloadRequest>>(&s))
    .transpose()?
    .flatten();
    let download_response = all_clients::process_download_request(download_request)?;
    std::fs::write("sync/download_response.json", serde_json::to_string(&download_response)?)?;

    println!("responses written to sync/");
    Ok(())
}
```

Now run the round trip, in this order:

```bash
# 1. From my-app/: client makes a local change and writes its requests
CARBURETOR_TARGET=client cargo run -p my-app-client -- push

# 2. Backend applies the upload and answers the download
cargo run -p my-app-backend

# 3. Client stores the responses and prints the merged result
CARBURETOR_TARGET=client cargo run -p my-app-client -- pull
```

Run the three commands. You should see output like this:

```text
$ CARBURETOR_TARGET=client cargo run -p my-app-client -- push
requests written to sync/

$ cargo run -p my-app-backend
responses written to sync/

$ CARBURETOR_TARGET=client cargo run -p my-app-client -- pull
[
    FullUser {
        id: "user-3f2a1c-1737000000",
        username: "alice",
        first_name: Some(
            "Alice",
        ),
        joined_on: 2025-01-01,
        ...
    },
]
```

## Expected result

If everything is configured correctly:

- `push` prints `requests written to sync/` and you can open `sync/upload_request.json` to see the dirty `alice` record.
- `pull` prints one user record with `username: "alice"`. That record was created offline in SQLite, uploaded into PostgreSQL, and downloaded back, so it has round-tripped through the backend.
- In PostgreSQL, `SELECT username, is_deleted FROM users;` shows the `alice` row.
- Running `pull` again without new backend changes downloads nothing new: download offsets prevent re-downloading known records. Note that every `push` run creates another offline record (each insert gets a fresh `id`), so repeated push/pull cycles keep adding rows; that is expected local-first behavior.

## Troubleshooting

- **Panic on startup: config initialized twice.** `initialize_carburetor_global_config` must be called exactly once per process.
- **Client fails to compile with PostgreSQL types or missing functions.** `CARBURETOR_TARGET` is read at compile time and defaults to `backend`. Every command that builds the client crate must pass it inline: `CARBURETOR_TARGET=client cargo ...`. A plain `cargo run -p my-app-client` compiles the client crate as a backend.
- **Sync errors about missing or mismatched columns.** Your SQL tables must match the declared schema exactly, including the auto-added bookkeeping columns and their nullability. Compare against Step 4.
- **SQLite errors about missing feature.** The client crate needs `diesel/returning_clauses_for_sqlite_3_35`.
- **`pull` cannot find `sync/upload_response.json`.** The backend step was skipped or run from a different working directory. The `sync/` paths are relative to where you run each binary.

## FAQ

- **Where does the real network fit?** Every request and response type (`DownloadRequest`, `DownloadResponse`, `UploadRequest`, `UploadResponse`) implements serde's `Serialize` and `Deserialize`. Replace the JSON file handoff with your transport of choice; the four function calls stay the same.
- **What are the extra columns for?** `id`, `last_synced_at`, `is_deleted`, `dirty_flag`, and `column_sync_metadata` are the sync bookkeeping that powers soft deletion, incremental download, and column-level last-write-wins merging. See [soft deletion](../explanation/soft-deletion.md) and [time sync and clocks](../explanation/time-sync-and-clocks.md) for why they exist.

## Next steps

- Enable automatic schema migration: [set up server auto-migration](../how-to/setup-server-auto-migration.md) and [set up client auto-migration](../how-to/setup-client-auto-migration.md)
- Learn the full `carburetor_sync_config!` syntax: [table reference](../reference/sync-config/table.md) and [sync-group reference](../reference/sync-config/sync-group.md)
- Understand the design: [why sync groups exist](../explanation/sync-groups.md)
