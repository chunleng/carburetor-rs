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
