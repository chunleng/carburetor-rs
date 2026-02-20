use carburetor::models::UploadTableResponseErrorType;
use diesel::RunQueryDsl;
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::all_clients;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_upload_update_record_not_on_backend() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let dirty_at = carburetor::helpers::get_utc_now().to_rfc3339();

    // Insert a local record with dirty_flag = "update" but it doesn't exist on backend
    let dirty_user = all_clients::FullUser {
        id: "user-nonexistent-1".to_string(),
        username: "ghost_user".to_string(),
        first_name: Some("Ghost".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("update".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{"username": {{"dirty_at": "{}"}}}}"#,
            dirty_at
        ))
        .unwrap(),
    };
    diesel::insert_into(all_clients::users::table)
        .values(&dirty_user)
        .execute(&mut conn)
        .unwrap();

    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();

    assert_eq!(upload_response.user.len(), 1);
    match &upload_response.user[0] {
        Err(e) => {
            assert_eq!(e.id, dirty_user.id);
            assert_eq!(e.code, UploadTableResponseErrorType::RecordNotFound);
        }
        Ok(_) => panic!("Expected error for updating non-existent backend record"),
    }

    all_clients::store_upload_response(cutoff, upload_response).unwrap();
}

#[tokio::test]
async fn test_upload_insert_record_already_exists_on_backend() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Pre-insert the record on the backend
    backend
        .test_helper_insert_user(
            ctx(),
            "user-duplicate-1".to_string(),
            "existing_user".to_string(),
            Some("Existing".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            carburetor::helpers::get_utc_now(),
            false,
        )
        .await
        .unwrap();

    // Now try to insert the same record from client
    let dirty_user = all_clients::FullUser {
        id: "user-duplicate-1".to_string(),
        username: "existing_user".to_string(),
        first_name: Some("Existing".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("insert".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{".insert_time": "{}"}}"#,
            carburetor::helpers::get_utc_now().to_rfc3339()
        ))
        .unwrap(),
    };
    diesel::insert_into(all_clients::users::table)
        .values(&dirty_user)
        .execute(&mut conn)
        .unwrap();

    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();

    assert_eq!(upload_response.user.len(), 1);
    match &upload_response.user[0] {
        Err(e) => {
            assert_eq!(e.id, dirty_user.id);
            assert_eq!(e.code, UploadTableResponseErrorType::RecordAlreadyExists);
        }
        Ok(_) => panic!("Expected error for inserting already-existing backend record"),
    }

    all_clients::store_upload_response(cutoff, upload_response).unwrap();
}
