use diesel::{QueryDsl, RunQueryDsl, SelectableHelper};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::user_only;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_upload_insert_then_download() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a user on client using the generated function
    let inserted_user = user_only::insert_user(user_only::InsertUser {
        username: "sync_user".to_string(),
        first_name: Some("SyncUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 8, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
    })
    .unwrap();

    // Upload the dirty record to backend
    let (upload_cutoff, upload_request) = user_only::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_user_only_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);

    user_only::store_upload_response(upload_cutoff, upload_response).unwrap();

    // Verify dirty flag is cleared
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].dirty_flag, None);

    let download_request = user_only::retrieve_download_request().unwrap();
    let download_response = backend
        .process_user_only_download_request(ctx(), download_request)
        .await
        .unwrap();
    assert_eq! {download_response.user.data.len(), 1};

    user_only::store_download_response(download_response).unwrap();

    // Verify the record was properly merged and last_synced_at is now set
    let final_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(final_users.len(), 1);
    assert_eq!(final_users[0].id, inserted_user.id);
    assert_eq!(final_users[0].username, "sync_user");
    assert_eq!(final_users[0].dirty_flag, None);
    assert!(
        final_users[0].last_synced_at.is_some(),
        "last_synced_at should be set after download merge"
    );
}

#[tokio::test]
async fn test_upload_update_then_download() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a user on the backend
    backend
        .test_helper_insert_user(
            ctx(),
            "user-sync-2".to_string(),
            "original_user".to_string(),
            Some("OriginalUser".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
            carburetor::helpers::get_utc_now(),
            false,
        )
        .await
        .unwrap();

    let before_insert = backend
        .test_helper_get_user_last_synced_at(ctx(), "user-sync-2".to_string())
        .await
        .unwrap();

    // Seed the client with the user as if it was already downloaded (record + offset)
    let synced_user = user_only::FullUser {
        id: "user-sync-2".to_string(),
        username: "original_user".to_string(),
        first_name: Some("OriginalUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
        created_at: carburetor::helpers::get_utc_now(),
        last_synced_at: Some(before_insert),
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    diesel::insert_into(user_only::users::table)
        .values(&synced_user)
        .execute(&mut conn)
        .unwrap();

    carburetor::helpers::carburetor_offset::upsert_offset(&mut conn, "users", before_insert)
        .unwrap();

    // Update the user on the client using the generated function
    let updated_user = user_only::update_user(user_only::UpdateUser {
        id: "user-sync-2".to_string(),
        username: Some("updated_user".to_string()),
        first_name: Some(Some("UpdatedUser".to_string())),
        joined_on: None,
    })
    .unwrap();
    assert_eq!(updated_user.dirty_flag.as_deref(), Some("update"));

    // Upload the dirty update to backend
    let (upload_cutoff, upload_request) = user_only::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_user_only_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);

    user_only::store_upload_response(upload_cutoff, upload_response).unwrap();

    let download_request = user_only::retrieve_download_request().unwrap();
    let download_response = backend
        .process_user_only_download_request(ctx(), download_request)
        .await
        .unwrap();
    assert_eq! {download_response.user.data.len(), 1};

    user_only::store_download_response(download_response).unwrap();

    // Verify the record was properly merged and last_synced_at is updated
    let final_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(final_users.len(), 1);
    assert_eq!(final_users[0].id, "user-sync-2");
    assert_eq!(final_users[0].username, "updated_user");
    assert_eq!(final_users[0].first_name, Some("UpdatedUser".to_string()));
    assert_eq!(final_users[0].dirty_flag, None);
    assert!(
        final_users[0].last_synced_at.unwrap() > before_insert,
        "last_synced_at should be updated after download merge"
    );
}
