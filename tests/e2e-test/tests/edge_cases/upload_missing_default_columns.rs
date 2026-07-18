use diesel::{Connection, QueryableByName, RunQueryDsl, sql_query};
use e2e_test::TestBackendHandle;
use tarpc::context::current as ctx;

#[derive(Debug, QueryableByName)]
#[allow(dead_code)]
struct UserRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    id: String,
    #[diesel(sql_type = diesel::sql_types::Timestamptz)]
    created_at: carburetor::chrono::DateTimeUtc,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    nickname: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    priority: i32,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
    preferences: Option<String>,
}

/// Old clients that predate default columns send upload JSON without
/// `created_at`, `nickname`, `priority`, or `preferences`. The backend must
/// accept the incomplete JSON and apply defaults: Rust defaults in code,
/// SQL defaults via the database.
#[tokio::test]
async fn test_upload_insert_omitting_default_columns() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Capture time before upload to verify created_at default
    let before = carburetor::helpers::get_utc_now();

    // Raw JSON simulating an old client that doesn't know about
    // created_at, nickname, priority, or preferences — omits them entirely.
    let upload_json = r#"{
        "user": [
            {
                "Insert": {
                    "username": "old_client_user",
                    "first_name": "OldClient",
                    "joined_on": "2025-01-01",
                    "id": "user-old-1",
                    "is_deleted": false
                }
            }
        ]
    }"#;

    let upload_response: sample_test_core::schema::user_only::UploadResponse =
        carburetor::serde_json::from_str(
            &backend
                .process_user_only_upload_request(ctx(), upload_json.to_string())
                .await
                .unwrap(),
        )
        .unwrap();

    assert_eq!(upload_response.user.len(), 1);
    assert!(
        upload_response.user[0].is_ok(),
        "Upload should succeed even with missing default columns"
    );

    let database_url = backend.test_helper_get_database_url(ctx()).await.unwrap();
    let mut conn = diesel::PgConnection::establish(&database_url).unwrap();

    // TODO: In the future, we need to test two different client/server
    // versions and ensure additional records returned by the server are
    // not lost in the older local client by storing them unstructuredly.
    // This test will be useful to capture that behavior.
    let stored: Vec<UserRow> =
        sql_query("SELECT id, created_at, nickname, priority, preferences FROM users WHERE id = 'user-old-1'")
            .load(&mut conn)
            .unwrap();

    assert_eq!(stored.len(), 1);
    // Rust default applied by backend conversion (unwrap_or_else)
    assert!(
        stored[0].created_at >= before,
        "created_at should default to current time when omitted by old client"
    );
    assert_eq!(
        stored[0].nickname,
        Some("default_nickname".to_string()),
        "nickname should get Rust default when omitted by old client"
    );
    // SQL DEFAULT applied by database
    assert_eq!(
        stored[0].priority, 0,
        "priority should get SQL default when omitted by old client"
    );
    assert_eq!(
        stored[0].preferences,
        Some("no preference".to_string()),
        "preferences should get SQL default when omitted by old client"
    );
}
