use diesel::{QueryableByName, RunQueryDsl};
use e2e_test::get_clean_test_client_db;

#[derive(Debug, QueryableByName)]
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

#[tokio::test]
async fn test_clean_migration_creates_all_tables() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    let users = get_columns(&mut conn, "users");
    assert_eq!(users.len(), 12);
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

    let messages = get_columns(&mut conn, "messages");
    assert_eq!(messages.len(), 9);
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

    let offsets = get_columns(&mut conn, "carburetor_offsets");
    assert_eq!(offsets.len(), 2);
    assert_column(&offsets, "table_name", "TEXT", true, true, None);
    assert_column(&offsets, "cutoff_at", "TIMESTAMPTZ", true, false, None);
}

/// Recreate `users` with only NOT NULL no-default columns, omitting every
/// addable column (nullable or has a SQL default). After migration the table
/// should gain all 6 omitted columns with correct attributes.
#[tokio::test]
async fn test_existing_table_missing_columns_gets_added() {
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
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    let before = get_columns(&mut conn, "users");
    assert_eq!(before.len(), 6, "table should start with 6 columns");

    sample_test_core::schema::run_migrations(&mut conn).unwrap();

    let after = get_columns(&mut conn, "users");
    assert_eq!(
        after.len(),
        12,
        "table should have 12 columns after migration"
    );

    // Verify the added columns have correct attributes
    assert_column(&after, "first_name", "TEXT", false, false, None);
    assert_column(&after, "nickname", "TEXT", false, false, None);
    assert_column(&after, "priority", "INTEGER", true, false, Some("0"));
    assert_column(
        &after,
        "preferences",
        "TEXT",
        false,
        false,
        Some("'no preference'"),
    );
    assert_column(&after, "last_synced_at", "TIMESTAMPTZ", false, false, None);
    assert_column(&after, "dirty_flag", "TEXT", false, false, None);
}

#[derive(Debug, QueryableByName)]
struct UserRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    username: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    first_name: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    nickname: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    priority: i32,
}

/// Create `users` with `first_name` and `nickname` as NOT NULL (both declared
/// nullable in the schema). Migration should rebuild the table once, relaxing
/// both columns to nullable, and preserve all existing row data.
#[tokio::test]
async fn test_multiple_columns_relaxed_in_single_rebuild() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    diesel::sql_query("DROP TABLE users")
        .execute(&mut conn)
        .unwrap();
    diesel::sql_query(
        "CREATE TABLE users (\
         id TEXT PRIMARY KEY NOT NULL, \
         username TEXT NOT NULL, \
         first_name TEXT NOT NULL, \
         joined_on DATE NOT NULL, \
         created_at TIMESTAMPTZ NOT NULL, \
         nickname TEXT NOT NULL, \
         priority INTEGER NOT NULL DEFAULT 0, \
         preferences TEXT DEFAULT 'no preference', \
         last_synced_at TIMESTAMPTZ, \
         is_deleted BOOLEAN NOT NULL, \
         dirty_flag TEXT, \
         column_sync_metadata JSON NOT NULL)",
    )
    .execute(&mut conn)
    .unwrap();

    diesel::sql_query(
        "INSERT INTO users \
         (id, username, first_name, joined_on, created_at, nickname, priority, is_deleted, column_sync_metadata) \
         VALUES ('user-1', 'alice', 'Alice', '2024-01-15', '2024-01-15T10:00:00Z', 'Alice A', 5, 0, '{}')",
    )
    .execute(&mut conn)
    .unwrap();

    let before = get_columns(&mut conn, "users");
    assert_column(&before, "first_name", "TEXT", true, false, None);
    assert_column(&before, "nickname", "TEXT", true, false, None);

    sample_test_core::schema::run_migrations(&mut conn).unwrap();

    let after = get_columns(&mut conn, "users");
    assert_eq!(after.len(), 12, "table should still have 12 columns");
    assert_column(&after, "id", "TEXT", true, true, None);
    assert_column(&after, "first_name", "TEXT", false, false, None);
    assert_column(&after, "nickname", "TEXT", false, false, None);

    let rows: Vec<UserRow> = diesel::sql_query(
        "SELECT username, first_name, nickname, priority FROM users WHERE id = 'user-1'",
    )
    .load(&mut conn)
    .unwrap();
    assert_eq!(rows.len(), 1, "row should be preserved through rebuild");
    assert_eq!(rows[0].username, "alice");
    assert_eq!(rows[0].first_name, "Alice");
    assert_eq!(rows[0].nickname.as_deref(), Some("Alice A"));
    assert_eq!(rows[0].priority, 5);
}
