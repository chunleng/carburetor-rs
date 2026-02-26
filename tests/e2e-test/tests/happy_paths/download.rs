use carburetor::chrono::NaiveDate;
use diesel::{RunQueryDsl, SelectableHelper, query_dsl::methods::SelectDsl};
use e2e_test::{TestBackendHandle, get_clean_test_client_db};
use sample_test_core::{
    backend_service::TestBackendClient,
    schema::{all_clients, user_only},
};
use tarpc::context::current as ctx;

async fn insert_dummy_message(
    backend: &TestBackendClient,
    id: &str,
    recipient_id: &str,
    is_deleted: bool,
) {
    backend
        .test_helper_insert_message(
            ctx(),
            id.to_string(),
            recipient_id.to_string(),
            "subject".to_string(),
            "body".to_string(),
            is_deleted,
        )
        .await
        .unwrap();
}

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

    let req = user_only::retrieve_download_request().unwrap();
    let res = backend
        .process_user_only_download_request(ctx(), req)
        .await
        .unwrap();
    assert_eq! {res.user.data.len(), 1};
    user_only::store_download_response(res).unwrap();

    insert_dummy_user(&backend, "b", false).await;

    let req = user_only::retrieve_download_request().unwrap();
    let res = backend
        .process_user_only_download_request(ctx(), req)
        .await
        .unwrap();
    assert_eq! {res.user.data.len(), 1};
    user_only::store_download_response(res).unwrap();

    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
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

    let req = user_only::retrieve_download_request().unwrap();
    let res = backend
        .process_user_only_download_request(ctx(), req)
        .await
        .unwrap();

    user_only::store_download_response(res).unwrap();
    let stored_users: Vec<user_only::FullUser> = user_only::users::table
        .select(user_only::FullUser::as_select())
        .load(&mut conn)
        .unwrap();

    assert_eq!(stored_users.len(), 1);
    assert_eq!(stored_users[0].id, "a".to_string());
}


#[tokio::test]
async fn test_download_only_returns_messages_matching_context() {
    get_clean_test_client_db();

    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    insert_dummy_message(&backend, "msg-a", "user-1", false).await;
    insert_dummy_message(&backend, "msg-b", "user-2", false).await;
    insert_dummy_message(&backend, "msg-c", "user-1", false).await;

    let req = all_clients::retrieve_download_request().unwrap();
    let res = backend
        .process_all_clients_download_request(ctx(), req, "user-1".to_string())
        .await
        .unwrap();

    assert_eq!(res.message.data.len(), 2);
    assert!(res.message.data.iter().any(
        |m| matches!(m, carburetor::models::DownloadTableResponseData::Update(u) if u.id == "msg-a")
    ));
    assert!(res.message.data.iter().any(
        |m| matches!(m, carburetor::models::DownloadTableResponseData::Update(u) if u.id == "msg-c")
    ));
    assert!(!res.message.data.iter().any(
        |m| matches!(m, carburetor::models::DownloadTableResponseData::Update(u) if u.id == "msg-b")
    ));
}
