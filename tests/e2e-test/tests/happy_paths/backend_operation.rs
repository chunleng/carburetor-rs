use carburetor::chrono::NaiveDate;
use diesel::{Connection, QueryableByName, RunQueryDsl, sql_query};
use e2e_test::TestBackendHandle;
use sample_test_core::backend_service::TestBackendClient;
use tarpc::context::current as ctx;

async fn insert_dummy_user(backend: &TestBackendClient, id: &str, is_deleted: bool) {
    backend
        .test_helper_insert_user(
            ctx(),
            id.to_string(),
            "username".to_string(),
            None,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            carburetor::helpers::get_utc_now(),
            is_deleted,
            None,
            None,
            None,
        )
        .await
        .unwrap();
}

#[derive(Debug, QueryableByName)]
struct UserRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    id: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    priority: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    preferences: Option<String>,
}

/// When inserting with `None` for columns that have SQL DEFAULT clauses,
/// PostgreSQL applies the default value. Querying the backend DB directly
/// confirms the defaults are stored, not NULL.
#[tokio::test]
async fn test_sql_default_applied_on_insert() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    insert_dummy_user(&backend, "a", false).await;

    let database_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&database_url).unwrap();

    let stored_users: Vec<UserRow> =
        sql_query("SELECT id, priority, preferences FROM users WHERE id = 'a'")
            .load(&mut conn)
            .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, "a");
    assert_eq!(stored_users[0].priority, 0);
    assert_eq!(
        stored_users[0].preferences,
        Some("no preference".to_string())
    );
}
