# How to set up a client with auto-migration

## Goal

Set up auto-migration in your client code to be ready for syncing.

Your client app stores data in a local SQLite database. When you ship a new
version of your app with schema changes, existing installs still have the old
schema. Client auto-migration reconciles the local database with your declared
schema at startup, so every install (fresh or upgraded) ends up with the schema
your code expects, without shipping hand-written migration scripts.

## Prerequisites

- The `migration` feature enabled on the client build of `carburetor` (it is
  included in the `client` feature).

## Steps

### 1. Declare your schema

Define your tables and sync groups with `carburetor_sync_config!`:

<!-- TODO: link to reference on how to configure carburetor_sync_config! -->
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

### 2. Initialize the global config

Before any carburetor function runs, initialize the global config with your
database path:

```rust
use carburetor::config::{CarburetorGlobalConfig, initialize_carburetor_global_config};

initialize_carburetor_global_config(CarburetorGlobalConfig { database_path });
```

`initialize_carburetor_global_config` stores the config in a process-wide
static. It must be called exactly once: a second call panics. If you skip it,
carburetor falls back to the default path `./default.db`, so always initialize
it explicitly when your database lives anywhere else.

### 3. Run migrations at startup

Establish the connection, then call the generated `run_migrations` before any
other database or sync operation:

```rust
use diesel::prelude::*;

let mut connection =
    SqliteConnection::establish(&database_path).expect("Error connecting to database");

match schema::run_migrations(&mut connection) {
    Ok(()) => {}
    Err(Error::DatabaseWiped { source }) => {
        // The local database was unrecoverably out of sync with the declared
        // schema, so it was wiped and recreated. Inform the user that local
        // data was reset; the next download re-syncs everything from the
        // backend automatically.
        eprintln!("Local database was reset: {source}");
    }
    Err(err) => return Err(err),
}
```

On the first run, `run_migrations` creates every declared table (including the
client-only sync columns and the `carburetor_offsets` table used for incremental
sync). On later runs, it introspects the existing schema and reconciles it:
missing columns are added, and type, primary key, and nullability mismatches are
validated before any change is applied.

If a validation finds a mismatch that cannot be repaired in place,
`run_migrations` returns `Error::DatabaseWiped`: inform the user that local
data was reset. The next download re-syncs everything from the backend
automatically. See [why client migration resets the database to a clean
state](../explanation/client-migration-reset-to-clean-state.md) for details.

## Next steps

- After migration, the client is ready to sync. <!-- TODO: link to how to handle
  the sync response -->
- For why type mismatches are compared at the SQLite affinity level, see [client
  migration type affinity
  checking](../explanation/client-migration-type-affinity-check.md).
- For why some schema changes require a full table rebuild, see [client
  migration table rebuild](../explanation/client-migration-table-rebuild.md).
- Try it with the example client: the `simple_client` example is a working
  client with auto-migration wired up. Run it with:
  `CARGO_TARGET_DIR=target/client CARBURETOR_TARGET=client cargo run --example
  simple_client --features client`
