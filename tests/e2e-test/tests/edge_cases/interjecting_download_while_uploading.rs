use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::schema::all_clients;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_download_between_upload_process_and_store() {
    let mut conn = get_clean_test_client_db().get_connection();
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    // Seed backend and client with an already-synced user
    backend
        .test_helper_insert_user(
            ctx(),
            "user-interject-1".to_string(),
            "original".to_string(),
            Some("Original".to_string()),
            carburetor::chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            false,
        )
        .await
        .unwrap();

    let before_seed = backend
        .test_helper_get_user(ctx(), "user-interject-1".to_string())
        .await
        .unwrap()
        .last_synced_at;

    let synced_user = all_clients::FullUser {
        id: "user-interject-1".to_string(),
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
    carburetor::helpers::carburetor_offset::upsert_offset(&mut conn, "users", before_seed).unwrap();

    // Client updates the user — dirty_flag="update"
    all_clients::update_user(all_clients::UpdateUser {
        id: "user-interject-1".to_string(),
        username: Some("updated".to_string()),
        first_name: None,
        joined_on: None,
    })
    .unwrap();

    // Retrieve upload request
    let (upload_cutoff, upload_request) = all_clients::retrieve_upload_request().unwrap();
    assert_eq!(upload_request.user.len(), 1);

    // Backend processes the upload (record is now updated on backend, last_synced_at=T_backend)
    let upload_response = backend
        .process_upload_request(ctx(), upload_request)
        .await
        .unwrap();
    assert_eq!(upload_response.user.len(), 1);
    assert!(upload_response.user[0].is_ok());

    // --- Interject: download before store_upload_response is called ---
    let download_request = all_clients::retrieve_download_request().unwrap();
    let download_response = backend
        .process_download_request(ctx(), download_request)
        .await
        .unwrap();
    assert_eq! {download_response.user.data.len(), 1};

    // Store the download — LWW must not clobber the still-dirty local state
    all_clients::store_download_response(download_response).unwrap();

    // Verify: dirty_flag is still set because store_upload_response hasn't run yet
    let users_mid: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(users_mid.len(), 1);
    assert_eq!(
        users_mid[0].dirty_flag.as_deref(),
        Some("update"),
        "dirty_flag must survive the interjecting download"
    );
    assert_eq!(
        users_mid[0].username, "updated",
        "local dirty value must not be overwritten by download"
    );

    // Now store the upload response — clears the dirty flag
    all_clients::store_upload_response(upload_cutoff, upload_response).unwrap();

    let users_final: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();
    assert_eq!(users_final.len(), 1);
    assert_eq!(
        users_final[0].dirty_flag, None,
        "dirty_flag must be cleared after store_upload_response"
    );
    assert_eq!(users_final[0].username, "updated");
}
