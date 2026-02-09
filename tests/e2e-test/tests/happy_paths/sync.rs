use std::time::Duration;

use diesel::{QueryDsl, RunQueryDsl, SelectableHelper};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::all_clients;
use tarpc::context::current as ctx;
use tokio::time::sleep;

#[tokio::test]
async fn test_upload_insert_then_download() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Insert a user on client using the generated function
    let inserted_user = all_clients::insert_user(all_clients::InsertUser {
        username: "sync_user".to_string(),
        first_name: Some("SyncUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 8, 1).unwrap(),
    })
    .unwrap();

    // Upload the dirty record to backend
    let (upload_cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);

    all_clients::store_upload_response(upload_cutoff, upload_response).unwrap();

    // Verify dirty flag is cleared
    let stored_users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].dirty_flag, None);

    let mut download_response = None;
    // Looping because there might be a small delay from postgres to update the record
    for i in 0..3 {
        // Download from backend (backend will return the updated record)
        let download_request = all_clients::retrieve_download_request().unwrap();
        let res = backend
            .process_download_request(ctx(), download_request)
            .await
            .unwrap();
        if res.user.data.len() == 1 {
            download_response = Some(res);
            break;
        }
        if i == 2 {
            panic!("Backend should return the updated record");
        }
        sleep(Duration::from_millis(100)).await;
    }
    let download_response = download_response.unwrap();

    all_clients::store_download_response(download_response).unwrap();

    // Verify the record was properly merged and last_synced_at is now set
    let final_users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
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

    let before_insert = carburetor::helpers::get_utc_now();

    // Insert a user on the backend
    backend
        .test_helper_insert_user(
            ctx(),
            "user-sync-2".to_string(),
            "original_user".to_string(),
            Some("OriginalUser".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
            before_insert,
            false,
        )
        .await
        .unwrap();

    // Seed the client with the user as if it was already downloaded (record + offset)
    let synced_user = all_clients::FullUser {
        id: "user-sync-2".to_string(),
        username: "original_user".to_string(),
        first_name: Some("OriginalUser".to_string()),
        joined_on: carburetor::chrono::NaiveDate::from_ymd_opt(2025, 9, 1).unwrap(),
        last_synced_at: Some(before_insert),
        is_deleted: false,
        dirty_flag: None,
        column_sync_metadata: carburetor::serde_json::from_str("{}").unwrap(),
    };
    diesel::insert_into(all_clients::users::table)
        .values(&synced_user)
        .execute(&mut conn)
        .unwrap();

    carburetor::helpers::carburetor_offset::upsert_offset(&mut conn, "users", before_insert)
        .unwrap();

    // Update the user on the client using the generated function
    let updated_user = all_clients::update_user(all_clients::UpdateUser {
        id: "user-sync-2".to_string(),
        username: Some("updated_user".to_string()),
        first_name: Some(Some("UpdatedUser".to_string())),
        joined_on: None,
    })
    .unwrap();
    assert_eq!(updated_user.dirty_flag.as_deref(), Some("update"));

    // Upload the dirty update to backend
    let (upload_cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);

    all_clients::store_upload_response(upload_cutoff, upload_response).unwrap();

    let mut download_response = None;
    // Looping because there might be a small delay from postgres to update the record
    for i in 0..3 {
        // Download from backend (backend will return the updated record)
        let download_request = all_clients::retrieve_download_request().unwrap();
        let res = backend
            .process_download_request(ctx(), download_request)
            .await
            .unwrap();
        if res.user.data.len() == 1 {
            download_response = Some(res);
            break;
        }
        if i == 2 {
            match &res.user.data[0] {
                carburetor::models::DownloadTableResponseData::Update(update_data) => {
                    assert_eq!(update_data.id, updated_user.id);
                }
            }
            panic!("Backend should return the updated record");
        }
        sleep(Duration::from_millis(100)).await;
    }
    let download_response = download_response.unwrap();

    all_clients::store_download_response(download_response).unwrap();

    // Verify the record was properly merged and last_synced_at is updated
    let final_users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
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
