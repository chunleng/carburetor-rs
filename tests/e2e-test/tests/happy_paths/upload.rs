use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;
use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::{all_clients, user_only};
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_upload_with_no_dirty_record() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a clean (non-dirty) user record
    let clean_user = user_only::FullUser {
        username: "clean_user".to_string(),
        first_name: Some("NoDirty".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 5, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-clean-1".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    diesel::insert_into(user_only::users::table)
        .values(&clean_user)
        .execute(&mut conn)
        .unwrap();

    // Retrieve upload request
    let (cutoff, upload_request) = user_only::retrieve_upload_request().unwrap();
    assert!(
        upload_request.user.is_empty(),
        "No dirty records should be present, even if clean records exist"
    );

    // Send to backend and get response
    let upload_response = backend
        .process_user_only_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert!(
        upload_response.user.is_empty(),
        "Backend should return empty response for no dirty records"
    );

    // Store upload response (should be a no-op, but should not error)
    user_only::store_upload_response(cutoff, upload_response).unwrap();
}

#[tokio::test]
async fn test_upload_with_inserted_dirty_record() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a user with dirty flag set to "insert"
    let dirty_user = user_only::FullUser {
        username: "new_user".to_string(),
        first_name: Some("NewUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 6, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-insert-1".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("insert".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{".insert_time": "{}"}}"#,
            carburetor::helpers::get_utc_now().to_rfc3339()
        ))
        .unwrap(),
    };
    diesel::insert_into(user_only::users::table)
        .values(&dirty_user)
        .execute(&mut conn)
        .unwrap();

    // Retrieve upload request
    let (cutoff, upload_request) = user_only::retrieve_upload_request().unwrap();
    assert_eq!(
        upload_request.user.len(),
        1,
        "Upload request should contain one dirty user"
    );

    // Verify the upload request contains the inserted user
    match &upload_request.user[0] {
        user_only::UploadRequestUser::Insert(insert_data) => {
            assert_eq!(insert_data.id, dirty_user.id);
            assert_eq!(insert_data.username, "new_user");
            assert_eq!(insert_data.first_name, Some("NewUser".to_string()));
            assert_eq!(insert_data.is_deleted, false);
        }
        _ => panic!("Expected Insert variant for newly inserted user"),
    }

    // Send to backend and get response
    let upload_response = backend
        .process_user_only_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(
        upload_response.user.len(),
        1,
        "Backend should respond with one user record"
    );

    // Verify the response is successful
    match &upload_response.user[0] {
        Ok(response_data) => {
            assert_eq!(response_data.id, dirty_user.id);
        }
        Err(e) => panic!("Upload should succeed, got error: {:?}", e),
    }

    // Store upload response (should clear dirty flag)
    user_only::store_upload_response(cutoff, upload_response).unwrap();

    // Verify dirty flag is cleared
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, dirty_user.id);
    assert_eq!(
        stored_users[0].dirty_flag, None,
        "Dirty flag should be cleared after successful upload"
    );
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        carburetor::serde_json::from_value(stored_users[0].column_sync_metadata.clone()).unwrap();
    assert_eq!(metadata.insert_time, None);
}

#[tokio::test]
async fn test_upload_with_updated_dirty_record() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // First, insert the user on the backend
    backend
        .test_helper_insert_user(
            ctx(),
            "user-update-1".to_string(),
            "original_user".to_string(),
            Some("OriginalUser".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
            carburetor::helpers::get_utc_now(),
            false,
        )
        .await
        .unwrap();

    let dirty_at = carburetor::helpers::get_utc_now().to_rfc3339();

    // Insert a user with dirty flag set to "update" with column-level metadata
    let dirty_user = user_only::FullUser {
        username: "updated_user".to_string(),
        first_name: Some("UpdatedUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 7, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        id: "user-update-1".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("update".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{
                "username": {{"dirty_at": "{}"}},
                "first_name": {{"dirty_at": "{}"}}
            }}"#,
            dirty_at, dirty_at
        ))
        .unwrap(),
    };
    diesel::insert_into(user_only::users::table)
        .values(&dirty_user)
        .execute(&mut conn)
        .unwrap();

    // Retrieve upload request
    let (cutoff, upload_request) = user_only::retrieve_upload_request().unwrap();
    assert_eq!(
        upload_request.user.len(),
        1,
        "Upload request should contain one dirty user"
    );

    // Verify the upload request contains the updated user
    match &upload_request.user[0] {
        user_only::UploadRequestUser::Update(update_data) => {
            assert_eq!(update_data.id, dirty_user.id);
            assert_eq!(update_data.username, Some("updated_user".to_string()));
            assert_eq!(
                update_data.first_name,
                Some(Some("UpdatedUser".to_string()))
            );
            assert_eq!(update_data.joined_on, None);
            assert_eq!(update_data.is_deleted, None);
        }
        _ => panic!("Expected Update variant for updated user"),
    }

    // Send to backend and get response
    let upload_response = backend
        .process_user_only_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(
        upload_response.user.len(),
        1,
        "Backend should respond with one user record"
    );

    // Verify the response is successful
    match &upload_response.user[0] {
        Ok(response_data) => {
            assert_eq!(response_data.id, dirty_user.id);
        }
        Err(e) => panic!("Upload should succeed, got error: {:?}", e),
    }

    // Store upload response (should clear dirty flag)
    user_only::store_upload_response(cutoff, upload_response).unwrap();

    // Verify dirty flag is cleared
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, dirty_user.id);
    assert_eq!(
        stored_users[0].dirty_flag, None,
        "Dirty flag should be cleared after successful upload"
    );
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        carburetor::serde_json::from_value(stored_users[0].column_sync_metadata.clone()).unwrap();
    assert_eq!(
        metadata.clone().data.unwrap().username.unwrap().dirty_at,
        None
    );
    assert!(
        metadata
            .clone()
            .data
            .unwrap()
            .username
            .unwrap()
            .column_last_synced_at
            .is_some()
    );
    assert_eq!(
        metadata.clone().data.unwrap().first_name.unwrap().dirty_at,
        None
    );
    assert!(
        metadata
            .clone()
            .data
            .unwrap()
            .first_name
            .unwrap()
            .column_last_synced_at
            .is_some()
    );
}

#[tokio::test]
async fn test_upload_update_message_matching_context() {
    let mut conn = get_clean_test_client_db().get_connection();

    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    backend
        .test_helper_insert_message(
            ctx(),
            "msg-update-1".to_string(),
            "user-1".to_string(),
            "Hello".to_string(),
            "World".to_string(),
            false,
        )
        .await
        .unwrap();

    let dirty_at = carburetor::helpers::get_utc_now().to_rfc3339();

    let dirty_message = all_clients::FullMessage {
        id: "msg-update-1".to_string(),
        recipient_id: "user-1".to_string(),
        subject: "Updated Subject".to_string(),
        body: "Updated Body".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("update".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{"subject": {{"dirty_at": "{}"}}, "body": {{"dirty_at": "{}"}}}}"#,
            dirty_at, dirty_at
        ))
        .unwrap(),
    };
    diesel::insert_into(all_clients::messages::table)
        .values(&dirty_message)
        .execute(&mut conn)
        .unwrap();

    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.message.len(), 1);

    let upload_response = backend
        .process_all_clients_upload_request(ctx(), upload_request, "user-1".to_string())
        .await
        .unwrap();

    assert_eq!(upload_response.message.len(), 1);
    match &upload_response.message[0] {
        Ok(r) => assert_eq!(r.id, dirty_message.id),
        Err(e) => panic!("Expected success, got error: {:?}", e),
    }

    all_clients::store_upload_response(cutoff, upload_response).unwrap();

    let stored: Vec<all_clients::FullMessage> = all_clients::messages::table
        .select(all_clients::FullMessage::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].dirty_flag, None);
}

#[tokio::test]
async fn test_upload_insert_message_matching_context() {
    let mut conn = get_clean_test_client_db().get_connection();

    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let dirty_message = all_clients::FullMessage {
        id: "msg-insert-1".to_string(),
        recipient_id: "user-1".to_string(),
        subject: "Hello".to_string(),
        body: "World".to_string(),
        last_synced_at: None,
        is_deleted: false,
        dirty_flag: Some("insert".to_string()),
        column_sync_metadata: carburetor::serde_json::from_str(&format!(
            r#"{{".insert_time": "{}"}}"#,
            carburetor::helpers::get_utc_now().to_rfc3339()
        ))
        .unwrap(),
    };
    diesel::insert_into(all_clients::messages::table)
        .values(&dirty_message)
        .execute(&mut conn)
        .unwrap();

    let (cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.message.len(), 1);

    let upload_response = backend
        .process_all_clients_upload_request(ctx(), upload_request, "user-1".to_string())
        .await
        .unwrap();

    assert_eq!(upload_response.message.len(), 1);
    match &upload_response.message[0] {
        Ok(r) => assert_eq!(r.id, dirty_message.id),
        Err(e) => panic!("Expected success, got error: {:?}", e),
    }

    all_clients::store_upload_response(cutoff, upload_response).unwrap();

    let stored: Vec<all_clients::FullMessage> = all_clients::messages::table
        .select(all_clients::FullMessage::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].dirty_flag, None);
}
