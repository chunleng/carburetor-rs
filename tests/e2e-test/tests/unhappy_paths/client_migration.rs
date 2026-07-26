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
