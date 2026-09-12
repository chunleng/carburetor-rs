use carburetor::chrono::NaiveDate;
use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::{backend_service::TestBackendClient, schema::user_only};
use tarpc::context::current as ctx;

async fn insert_dummy_user(backend: &TestBackendClient, id: &str) {
    backend
        .test_helper_insert_user(
            ctx(),
            id.to_string(),
            "username".to_string(),
            None,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            carburetor::helpers::get_utc_now(),
            false,
            None,
            None,
            None,
        )
        .await
        .unwrap();
}

async fn download_users(backend: &TestBackendClient) -> user_only::DownloadResponse {
    let req = user_only::retrieve_download_request().unwrap();
    carburetor::serde_json::from_str(
        &backend
            .process_user_only_download_request(
                ctx(),
                carburetor::serde_json::to_string(&req).unwrap(),
            )
            .await
            .unwrap(),
    )
    .unwrap()
}

fn stored_users(conn: &mut diesel::SqliteConnection) -> Vec<user_only::FullUser> {
    user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(conn)
        .unwrap()
}

/// The reset after an unrecoverable migration failure drops local data and `carburetor_offsets`.
/// The next download must therefore start from zero offsets and return every backend row, restoring
/// the full dataset rather than skipping rows the client believes it already has.
#[tokio::test]
async fn test_reset_dropped_offsets_cause_full_resync() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    insert_dummy_user(&backend, "a").await;
    insert_dummy_user(&backend, "b").await;
    insert_dummy_user(&backend, "c").await;

    let res = download_users(&backend).await;
    assert_eq!(res.user.data.len(), 3);
    user_only::store_download_response(res).unwrap();
    assert_eq!(stored_users(&mut conn).len(), 3);

    // Drift users (username INTEGER) so migration fails unrecoverably and
    // the reset wipes local data and offsets.
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
    let err = result.unwrap_err();
    assert!(
        matches!(err, carburetor::error::Error::DatabaseWiped { .. }),
        "expected Error::DatabaseWiped, got: {:?}",
        err
    );
    assert_eq!(
        stored_users(&mut conn).len(),
        0,
        "local data should be wiped by the reset"
    );

    // Re-migration succeeds on the fresh schema.
    sample_test_core::schema::run_migrations(&mut conn)
        .expect("re-migration should succeed on fresh schema");

    // Offsets were dropped with the reset, so the download returns every
    // backend row again (full re-sync, not incremental).
    let res = download_users(&backend).await;
    assert_eq!(
        res.user.data.len(),
        3,
        "download should return all rows after reset dropped offsets"
    );
    user_only::store_download_response(res).unwrap();

    let users = stored_users(&mut conn);
    assert_eq!(users.len(), 3, "all users should be restored");
    assert!(users.iter().any(|u| u.id == "a"));
    assert!(users.iter().any(|u| u.id == "b"));
    assert!(users.iter().any(|u| u.id == "c"));
}
