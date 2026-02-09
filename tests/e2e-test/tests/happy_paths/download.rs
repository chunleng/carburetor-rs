use carburetor::{chrono::NaiveDate, helpers::get_utc_now};
use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::{backend_service::TestBackendClient, schema::all_clients};
use tarpc::context::current as ctx;

async fn insert_dummy_user(backend: &TestBackendClient, id: &str, is_deleted: bool) {
    backend
        .test_helper_insert_user(
            ctx(),
            id.to_string(),
            "username".to_string(),
            None,
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            get_utc_now(),
            is_deleted,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn test_download_from_offset() {
    let mut conn = get_clean_test_client_db().get_connection();

    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    insert_dummy_user(&backend, "a", false).await;

    let req = all_clients::retrieve_download_request().unwrap();
    let res = backend.process_download_request(ctx(), req).await.unwrap();
    assert_eq! {res.user.data.len(), 1};
    all_clients::store_download_response(res).unwrap();

    insert_dummy_user(&backend, "b", false).await;

    let req = all_clients::retrieve_download_request().unwrap();
    let res = backend.process_download_request(ctx(), req).await.unwrap();
    assert_eq! {res.user.data.len(), 1};
    all_clients::store_download_response(res).unwrap();

    let stored_users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 2);
    assert!(stored_users.iter().any(|u| u.id == "a"));
    assert!(stored_users.iter().any(|u| u.id == "b"));
}

#[tokio::test]
async fn test_clean_download() {
    let mut conn = get_clean_test_client_db().get_connection();

    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    insert_dummy_user(&backend, "a", false).await;
    insert_dummy_user(&backend, "b", true).await;

    let req = all_clients::retrieve_download_request().unwrap();
    let res = backend.process_download_request(ctx(), req).await.unwrap();

    all_clients::store_download_response(res).unwrap();
    let stored_users: Vec<all_clients::FullUser> = all_clients::users::table
        .select(all_clients::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, "a".to_string());
}
