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

/// Omit `username` (NOT NULL, no default) from the existing table. Migration
/// must error naming the column and table, and must not add any columns.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("username"),
        "error should mention column 'username': {}",
        err_msg
    );
    assert!(
        err_msg.contains("users"),
        "error should mention table 'users': {}",
        err_msg
    );

    // No columns should have been added — validation runs before any DDL
    let after = get_columns(&mut conn, "users");
    assert_eq!(
        after.len(),
        5,
        "no columns should be added when migration fails"
    );
}

/// Create the `users` table with `username` declared as INTEGER instead of
/// TEXT. SQLite affinity differs (INTEGER vs TEXT), so migration must fail
/// naming the column, table, and types. No DDL should be applied.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("type mismatch"),
        "error should mention type mismatch: {}",
        err_msg
    );
    assert!(
        err_msg.contains("username"),
        "error should mention column 'username': {}",
        err_msg
    );
    assert!(
        err_msg.contains("users"),
        "error should mention table 'users': {}",
        err_msg
    );
    assert!(
        err_msg.contains("TEXT"),
        "error should show declared type/affinity: {}",
        err_msg
    );
    assert!(
        err_msg.contains("INTEGER"),
        "error should show DB type/affinity: {}",
        err_msg
    );

    // No columns should have been added — validation runs before any DDL
    let after = get_columns(&mut conn, "users");
    assert_eq!(
        after.len(),
        6,
        "no columns should be added when migration fails"
    );
}

/// Create the `users` table with an extra NOT NULL column (`extra_required`)
/// that has no default and is not in the schema. Migration must fail naming
/// the column and table. No DDL should be applied.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("extra_required"),
        "error should mention column 'extra_required': {}",
        err_msg
    );
    assert!(
        err_msg.contains("users"),
        "error should mention table 'users': {}",
        err_msg
    );
    assert!(
        err_msg.contains("NOT NULL"),
        "error should mention NOT NULL: {}",
        err_msg
    );

    // No columns should have been added — validation runs before any DDL
    let after = get_columns(&mut conn, "users");
    assert_eq!(
        after.len(),
        7,
        "no columns should be added when migration fails"
    );
}

/// Users is missing `first_name` (nullable, re-addable). Messages has `subject`
/// as INTEGER instead of TEXT (affinity mismatch). Users migration succeeds
/// (first_name re-added), then messages fails. The whole transaction rolls
/// back, so first_name must still be missing from users.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("type mismatch"),
        "error should mention type mismatch: {}",
        err_msg
    );
    assert!(
        err_msg.contains("subject"),
        "error should mention column 'subject': {}",
        err_msg
    );

    // first_name was re-added by the users migration, but the messages
    // migration failure rolled back the entire transaction; first_name must
    // still be gone
    let users_after = get_columns(&mut conn, "users");
    assert!(
        !users_after.iter().any(|c| c.name == "first_name"),
        "first_name should still be missing after rollback"
    );
    assert_eq!(
        users_after.len(),
        11,
        "users should still have 11 columns after rollback"
    );
}

/// Messages is missing `notes` (nullable, re-addable). Users has `priority` as
/// TEXT instead of INTEGER (affinity mismatch). For the client, users always
/// migrates before messages, so users fails first and messages never runs.
/// Notes was never re-added, so this assertion is trivially true. However,
/// together with the test above, this guarantees that regardless of migration
/// order, at least one test exercises actual rollback.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("type mismatch"),
        "error should mention type mismatch: {}",
        err_msg
    );
    assert!(
        err_msg.contains("priority"),
        "error should mention column 'priority': {}",
        err_msg
    );

    // notes was never re-added (users fails before messages runs), so it must
    // still be missing
    let messages_after = get_columns(&mut conn, "messages");
    assert!(
        !messages_after.iter().any(|c| c.name == "notes"),
        "notes should still be missing after failed migration"
    );
    assert_eq!(
        messages_after.len(),
        8,
        "messages should still have 8 columns after failed migration"
    );
}

/// Create users with `id` as a non-PK column (all other columns correct).
/// Migration must fail with "primary key mismatch" mentioning "id". No DDL
/// should be applied.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("primary key mismatch"),
        "error should mention primary key mismatch: {}",
        err_msg
    );
    assert!(
        err_msg.contains("id"),
        "error should mention column 'id': {}",
        err_msg
    );

    // No DDL should have been applied — id should still not be a PK
    let after = get_columns(&mut conn, "users");
    let id_col = after.iter().find(|c| c.name == "id").unwrap();
    assert_eq!(
        id_col.pk, 0,
        "id should still not be a primary key after failed migration"
    );
    assert_eq!(
        after.len(),
        12,
        "no columns should be added or removed when migration fails"
    );
}

/// Create users with `username` as nullable (schema declares NOT NULL).
/// Migration must fail with "nullability mismatch" mentioning "username".
/// No DDL should be applied.
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
    assert!(result.is_err(), "migration should fail");

    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nullability mismatch"),
        "error should mention nullability mismatch: {}",
        err_msg
    );
    assert!(
        err_msg.contains("username"),
        "error should mention column 'username': {}",
        err_msg
    );

    // No DDL should have been applied — username should still be nullable
    let after = get_columns(&mut conn, "users");
    let username_col = after.iter().find(|c| c.name == "username").unwrap();
    assert_eq!(
        username_col.notnull, 0,
        "username should still be nullable after failed migration"
    );
    assert_eq!(
        after.len(),
        12,
        "no columns should be added or removed when migration fails"
    );
}
