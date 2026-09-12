use diesel::{QueryableByName, RunQueryDsl};
use e2e_test::get_clean_test_client_db;

#[derive(Debug, QueryableByName)]
#[allow(dead_code)]
struct PragmaColumnInfo {
    #[diesel(sql_type = diesel::sql_types::Text)]
    name: String,
    #[diesel(column_name = "type", sql_type = diesel::sql_types::Text)]
    col_type: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    notnull: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    dflt_value: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    pk: i32,
}

fn get_columns(conn: &mut diesel::SqliteConnection, table: &str) -> Vec<PragmaColumnInfo> {
    diesel::sql_query(format!("PRAGMA table_info({})", table))
        .load(conn)
        .unwrap()
}

fn assert_column(
    columns: &[PragmaColumnInfo],
    name: &str,
    col_type: &str,
    notnull: bool,
    pk: bool,
    default: Option<&str>,
) {
    let col = columns.iter().find(|c| c.name == name).unwrap();
    assert_eq!(col.col_type, col_type, "column {} type", name);
    assert_eq!(col.notnull, notnull as i32, "column {} notnull", name);
    assert_eq!(col.pk, pk as i32, "column {} pk", name);
    assert_eq!(
        col.dflt_value.as_deref(),
        default,
        "column {} default",
        name
    );
}

/// After a failed migration the client resets to a clean state: managed tables
/// are dropped and recreated from the declared schema.
fn assert_declared_users_schema(conn: &mut diesel::SqliteConnection) {
    let users = get_columns(conn, "users");
    assert_eq!(users.len(), 12, "users should be recreated with 12 columns");
    assert_column(&users, "id", "TEXT", true, true, None);
    assert_column(&users, "username", "TEXT", true, false, None);
    assert_column(&users, "first_name", "TEXT", false, false, None);
    assert_column(&users, "joined_on", "DATE", true, false, None);
    assert_column(&users, "created_at", "TIMESTAMPTZ", true, false, None);
    assert_column(&users, "nickname", "TEXT", false, false, None);
    assert_column(&users, "priority", "INTEGER", true, false, Some("0"));
    assert_column(
        &users,
        "preferences",
        "TEXT",
        false,
        false,
        Some("'no preference'"),
    );
    assert_column(&users, "last_synced_at", "TIMESTAMPTZ", false, false, None);
    assert_column(&users, "is_deleted", "BOOLEAN", true, false, None);
    assert_column(&users, "dirty_flag", "TEXT", false, false, None);
    assert_column(&users, "column_sync_metadata", "JSON", true, false, None);
}

fn assert_declared_messages_schema(conn: &mut diesel::SqliteConnection) {
    let messages = get_columns(conn, "messages");
    assert_eq!(
        messages.len(),
        9,
        "messages should be recreated with 9 columns"
    );
    assert_column(&messages, "id", "TEXT", true, true, None);
    assert_column(&messages, "recipient_id", "TEXT", true, false, None);
    assert_column(&messages, "subject", "TEXT", true, false, None);
    assert_column(&messages, "body", "TEXT", true, false, None);
    assert_column(&messages, "notes", "TEXT", false, false, None);
    assert_column(
        &messages,
        "last_synced_at",
        "TIMESTAMPTZ",
        false,
        false,
        None,
    );
    assert_column(&messages, "is_deleted", "BOOLEAN", true, false, None);
    assert_column(&messages, "dirty_flag", "TEXT", false, false, None);
    assert_column(&messages, "column_sync_metadata", "JSON", true, false, None);
}

/// A failed migration that triggered the clean-state reset must surface
/// `Error::DatabaseWiped` so callers can detect that local data was lost,
/// with the original migration error preserved as the source.
fn assert_wiped_migration_error(
    result: Result<(), carburetor::error::Error>,
    expected_in_source: &[&str],
) {
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("database was wiped"),
        "error should signal the database was wiped: {}",
        err
    );
    match err {
        carburetor::error::Error::DatabaseWiped { source } => {
            let source_msg = source.to_string();
            for expected in expected_in_source {
                assert!(
                    source_msg.contains(expected),
                    "source error should mention '{}': {}",
                    expected,
                    source_msg
                );
            }
        }
        other => panic!("expected Error::DatabaseWiped, got: {:?}", other),
    }
}

/// Omit `username` (NOT NULL, no default) from the existing table. Migration
/// must error naming the column and table, then reset the DB to a clean
/// state: `users` is recreated from the declared schema.
#[tokio::test]
async fn test_existing_table_missing_non_nullable_without_default_errors() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         is_deleted BOOLEAN NOT NULL, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["username", "users", "no default specified"]);

    assert_declared_users_schema(&mut conn);
}

/// Create the `users` table with `username` declared as INTEGER instead of
/// TEXT. SQLite affinity differs (INTEGER vs TEXT), so migration must fail
/// naming the column, table, and types, then reset the DB to a clean state.
#[tokio::test]
async fn test_type_mismatch_affinity_fails() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username INTEGER NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         is_deleted BOOLEAN NOT NULL, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(
        result,
        &["type mismatch", "username", "users", "TEXT", "INTEGER"],
    );

    assert_declared_users_schema(&mut conn);
}

/// Create the `users` table with an extra NOT NULL column (`extra_required`)
/// that has no default and is not in the schema. Migration must fail naming
/// the column and table, then reset the DB to a clean state.
#[tokio::test]
async fn test_extra_not_null_column_without_default_fails() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username TEXT NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         is_deleted BOOLEAN NOT NULL, \
         column_sync_metadata JSON NOT NULL, \
         extra_required TEXT NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["extra_required", "users", "NOT NULL"]);

    assert_declared_users_schema(&mut conn);
}

/// Users is missing `first_name` (nullable, re-addable). Messages has `subject`
/// as INTEGER instead of TEXT (affinity mismatch). Users migration succeeds
/// (first_name re-added), then messages fails. The failure triggers a
/// clean-state reset, so both tables are recreated from the declared schema.
#[tokio::test]
async fn test_partial_migration_rolls_back_all_changes() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Users without first_name (nullable, re-addable by migration)
    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username TEXT NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         nickname TEXT, \
         priority INTEGER NOT NULL DEFAULT 0, \
         preferences TEXT DEFAULT 'no preference', \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    // Messages with subject as INTEGER (affinity mismatch: TEXT vs INTEGER)
    diesel::sql_query("DROP TABLE messages")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE messages (\
         id TEXT PRIMARY KEY NOT NULL, \
         recipient_id TEXT NOT NULL, \
         subject INTEGER NOT NULL, \
         body TEXT NOT NULL, \
         notes TEXT, \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["type mismatch", "subject", "messages"]);

    assert_declared_users_schema(&mut conn);
    assert_declared_messages_schema(&mut conn);
}

/// Messages is missing `notes` (nullable, re-addable). Users has `priority` as
/// TEXT instead of INTEGER (affinity mismatch). For the client, users always
/// migrates before messages, so users fails first and messages never runs.
/// The failure triggers a clean-state reset, so both tables are recreated
/// from the declared schema regardless of which table failed.
#[tokio::test]
async fn test_partial_migration_rolls_back_all_changes_reversed() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Messages without notes (nullable, re-addable by migration)
    diesel::sql_query("DROP TABLE messages")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE messages (\
         id TEXT PRIMARY KEY NOT NULL, \
         recipient_id TEXT NOT NULL, \
         subject TEXT NOT NULL, \
         body TEXT NOT NULL, \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    // Users with priority as TEXT (affinity mismatch: INTEGER vs TEXT)
    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username TEXT NOT NULL, \
         first_name TEXT, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         nickname TEXT, \
         priority TEXT NOT NULL DEFAULT 0, \
         preferences TEXT DEFAULT 'no preference', \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["type mismatch", "priority", "users"]);

    assert_declared_users_schema(&mut conn);
    assert_declared_messages_schema(&mut conn);
}

/// Create users with `id` as a non-PK column (all other columns correct).
/// Migration must fail with "primary key mismatch" mentioning "id", then
/// reset the DB to a clean state.
#[tokio::test]
async fn test_primary_key_mismatch_fails() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT NOT NULL, \
         username TEXT NOT NULL, \
         first_name TEXT, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         nickname TEXT, \
         priority INTEGER NOT NULL DEFAULT 0, \
         preferences TEXT DEFAULT 'no preference', \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["primary key mismatch", "id", "users"]);

    assert_declared_users_schema(&mut conn);
}

/// Create users with `username` as nullable (schema declares NOT NULL).
/// Migration must fail with "nullability mismatch" mentioning "username",
/// then reset the DB to a clean state.
#[tokio::test]
async fn test_nullable_tightening_fails() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username TEXT, \
         first_name TEXT, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         nickname TEXT, \
         priority INTEGER NOT NULL DEFAULT 0, \
         preferences TEXT DEFAULT 'no preference', \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);
    assert_wiped_migration_error(result, &["nullability mismatch", "username", "users"]);

    assert_declared_users_schema(&mut conn);
}

/// A raw SQLite error (e.g. database locked) must NOT trigger the clean-state reset: only schema
/// (migration) errors wipe the DB. Hold a write lock from a second connection so the migration's
/// CREATE TABLE fails with SQLITE_BUSY (diesel sets no busy timeout, so it fails immediately), then
/// assert the error surfaces as Error::Database and the schema is untouched.
#[tokio::test]
async fn test_database_error_does_not_trigger_reset() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Force a write during migration: users is missing and must be created.
    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();

    // Data whose survival proves no wipe happened.
    diesel::sql_query(
        "INSERT INTO messages \
         (id, recipient_id, subject, body, is_deleted, column_sync_metadata) \
         VALUES ('msg-1', 'user-1', 'hi', 'hello', 0, '{}')",
    )
    .execute(&mut conn)
    .unwrap();

    let mut locker = carburetor::helpers::get_connection().unwrap();
    diesel::sql_query("BEGIN IMMEDIATE")
        .execute(&mut locker)
        .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);

    // Release the lock before asserting: the DB is shared across tests.
    diesel::sql_query("ROLLBACK").execute(&mut locker).unwrap();
    drop(locker);

    match result {
        // Database errors surface as Error::Unhandled (with context), never as
        // Error::DatabaseWiped: only schema (migration) errors trigger the reset.
        Err(carburetor::error::Error::Unhandled { .. }) => {}
        other => panic!(
            "expected non-wiped error (Unhandled), got: {:?}",
            other.map_err(|e| e.to_string())
        ),
    }

    // No reset happened: users is still missing, messages data intact.
    assert!(
        get_columns(&mut conn, "users").is_empty(),
        "users should still be missing (no reset)"
    );
    let messages: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
        "COUNT(*) FROM messages",
    ))
    .get_result(&mut conn)
    .unwrap();
    assert_eq!(messages, 1, "messages data should survive (no wipe)");
}

/// A view occupying a table's namespace blocks the fresh run's CREATE TABLE
/// for that name. Drift `users` so the reset runs; the reset drops the view
/// named `messages`, the fresh run recreates `messages` as a table, and the
/// result is `Error::DatabaseWiped` with the original drift as the source.
#[tokio::test]
async fn test_reset_drops_blocking_views() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Drift users: username as INTEGER (affinity mismatch) fails validation.
    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username INTEGER NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         is_deleted BOOLEAN NOT NULL, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    // A view named `messages` occupies the table's namespace.
    diesel::sql_query("DROP TABLE messages")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("CREATE VIEW messages AS SELECT id FROM users")
        .execute(&mut conn)
        .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);

    assert_wiped_migration_error(result, &["type mismatch"]);

    // The view no longer occupies the `messages` namespace; `messages` exists
    // again as a table.
    let view_count: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
        "COUNT(*) FROM sqlite_master WHERE type = 'view' AND name = 'messages'",
    ))
    .get_result(&mut conn)
    .unwrap();
    assert_eq!(
        view_count, 0,
        "the namespace-occupying view should be dropped by the reset"
    );
    assert!(
        !get_columns(&mut conn, "messages").is_empty(),
        "messages should be recreated as a table"
    );
}

/// An index occupying a table's namespace blocks the fresh run's CREATE TABLE
/// for that name. Drift `users` so the reset runs; the reset drops the index
/// named `messages` (created on `users` while `messages` is absent, since an
/// index and a table cannot share a name), the fresh run recreates `messages`
/// as a table, and the result is `Error::DatabaseWiped` with the original
/// drift as the source.
#[tokio::test]
async fn test_reset_drops_blocking_indexes() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Drift users: username as INTEGER (affinity mismatch) fails validation.
    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username INTEGER NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         is_deleted BOOLEAN NOT NULL, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    // An index named `messages` occupies the table's namespace. It can only
    // be created while no table or view named `messages` exists.
    diesel::sql_query("DROP TABLE messages")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query("CREATE INDEX messages ON users (id)")
        .execute(&mut conn)
        .unwrap();

    let result = sample_test_core::schema::run_migrations(&mut conn);

    assert_wiped_migration_error(result, &["type mismatch"]);

    assert!(
        !get_columns(&mut conn, "messages").is_empty(),
        "messages should be recreated as a table"
    );
}
