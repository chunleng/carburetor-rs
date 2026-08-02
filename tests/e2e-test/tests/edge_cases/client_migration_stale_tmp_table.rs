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

/// A failed prior rebuild may leave a stale `_carburetor_tmp` table behind.
/// The next migration that needs to relax NOT NULL constraints must clear
/// the stale temp table and succeed, rather than failing on
/// "table _carburetor_tmp already exists".
#[tokio::test]
async fn test_stale_tmp_table_does_not_block_rebuild() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    // Set up users with first_name and nickname as NOT NULL (both declared
    // nullable in the schema), so migration triggers a table rebuild.
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

    // Simulate a leftover temp table from a crashed/failed prior rebuild
    diesel::sql_query("CREATE TABLE _carburetor_tmp (id TEXT PRIMARY KEY NOT NULL)")
        .execute(&mut conn)
        .unwrap();

    sample_test_core::schema::run_migrations(&mut conn)
        .expect("migration should succeed despite stale _carburetor_tmp table");

    // The stale temp table should be gone (renamed to users)
    let tmp_columns = get_columns(&mut conn, "_carburetor_tmp");
    assert!(
        tmp_columns.is_empty(),
        "_carburetor_tmp should not exist after migration"
    );

    // NOT NULL should have been relaxed on first_name and nickname
    let after = get_columns(&mut conn, "users");
    assert_eq!(
        after.len(),
        12,
        "users should have 12 columns after migration"
    );
    let first_name = after.iter().find(|c| c.name == "first_name").unwrap();
    assert_eq!(first_name.notnull, 0, "first_name should be nullable");
    let nickname = after.iter().find(|c| c.name == "nickname").unwrap();
    assert_eq!(nickname.notnull, 0, "nickname should be nullable");
}
