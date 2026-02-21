use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::all_clients;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_upload_insert_and_update_between_retrieve_and_store() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a user — dirty_flag="insert"
    let inserted = all_clients::insert_user(all_clients::InsertUser {
        username: "user_v1".to_string(),
        first_name: Some("V1".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
    })
    .unwrap();

    // Retrieve upload request — cutoff captured here
    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    // Update the user AFTER cutoff — simulates mutation between retrieve and store
    all_clients::update_user(all_clients::UpdateUser {
        id: inserted.id.clone(),
        username: Some("user_v2".to_string()),
        first_name: None,
        joined_on: None,
    })
    .unwrap();

    // Send original upload request to backend (backend processes the insert)
    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);
    assert!(upload_response.user[0].is_ok());

    // Store response — insert_time clears (was before cutoff), but username dirty_at is after
    // cutoff so dirty_flag should become "update" rather than None
    all_clients::store_upload_response(cutoff, upload_response).unwrap();

    let users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(
        users[0].dirty_flag.as_deref(),
        Some("update"),
        "dirty_flag should be 'update' because the post-cutoff update is still pending"
    );
    assert_eq!(users[0].username, "user_v2");
}

#[tokio::test]
async fn test_upload_update_and_update_same_column_between_retrieve_and_store() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Seed backend and client with an already-synced user
    backend
        .test_helper_insert_user(
            ctx(),
            "user-edge-1".to_string(),
            "original".to_string(),
            Some("Original".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            false,
        )
        .await
        .unwrap();

    let before_seed = backend
        .test_helper_get_user(ctx(), "user-edge-1".to_string())
        .await
        .unwrap()
        .last_synced_at;

    let synced_user = all_clients::FullUser {
        id: "user-edge-1".to_string(),
        username: "original".to_string(),
        first_name: Some("Original".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        last_synced_at: Some(before_seed),
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    diesel::insert_into(all_clients::users::table)
        .values(&synced_user)
        .execute(&mut conn)
        .unwrap();

    // First update — dirty_at for username = T0
    all_clients::update_user(all_clients::UpdateUser {
        id: "user-edge-1".to_string(),
        username: Some("updated_v1".to_string()),
        first_name: None,
        joined_on: None,
    })
    .unwrap();

    // Retrieve upload request — cutoff = T1 (> T0)
    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    // Second update to same column AFTER cutoff — dirty_at for username = T2 (> T1)
    all_clients::update_user(all_clients::UpdateUser {
        id: "user-edge-1".to_string(),
        username: Some("updated_v2".to_string()),
        first_name: None,
        joined_on: None,
    })
    .unwrap();

    // Send first update to backend
    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);
    assert!(upload_response.user[0].is_ok());

    // Store response — username dirty_at T2 > cutoff T1, so it must remain dirty
    all_clients::store_upload_response(cutoff, upload_response).unwrap();

    let users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(users.len(), 1);
    assert_eq!(
        users[0].dirty_flag.as_deref(),
        Some("update"),
        "dirty_flag should remain 'update' because the second update is still pending"
    );
    assert_eq!(users[0].username, "updated_v2");
}
