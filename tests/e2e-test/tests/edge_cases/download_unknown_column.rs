use std::time::Duration;

use carburetor::chrono::NaiveDate;
use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;
use carburetor::models::{DownloadTableResponse, DownloadTableResponseData};
use carburetor::serde_json::{Map, Value};
use diesel::{
    RunQueryDsl, SelectableHelper,
    query_dsl::methods::{FindDsl, SelectDsl},
};
use e2e_test::get_clean_test_client_db;
use sample_test_core::schema::user_only;

/// Builds a download response whose user row carries `future_column`, a column
/// that exists on the backend but not in the client schema. The extra field is
/// injected at the JSON level, mirroring a backend that was upgraded first.
fn download_response_with_unknown_column(
    id: &str,
    future_column: &str,
    last_synced_at_offset_secs: u64,
) -> user_only::DownloadResponse {
    let response = user_only::DownloadResponse {
        user: DownloadTableResponse {
            cutoff_at: carburetor::helpers::get_utc_now(),
            data: vec![DownloadTableResponseData::Update(
                user_only::DownloadUpdateUser {
                    id: id.to_string(),
                    username: "user_1".to_string(),
                    first_name: None,
                    joined_on: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
                    created_at: carburetor::helpers::get_utc_now(),
                    nickname: None,
                    priority: 0,
                    preferences: None,
                    last_synced_at: carburetor::helpers::get_utc_now()
                        + Duration::from_secs(last_synced_at_offset_secs),
                    is_deleted: false,
                    unknown_data: Map::new(),
                },
            )],
        },
    };
    let mut value = carburetor::serde_json::to_value(&response).unwrap();
    value["user"]["data"][0]["Update"]["future_column"] = Value::String(future_column.to_string());
    carburetor::serde_json::from_value(value).unwrap()
}

fn retrieve_stored_user(conn: &mut diesel::SqliteConnection, id: &str) -> user_only::FullUser {
    user_only::users::table
        .select(user_only::FullUser::as_select())
        .find(id)
        .first(conn)
        .unwrap()
}

fn staged_unknown_data(user: &user_only::FullUser) -> Value {
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        user.column_sync_metadata.clone().into();
    Value::from(metadata)["future_column"]["data"].clone()
}

#[tokio::test]
async fn test_download_with_unknown_column_stages_value_on_insert() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    let res = download_response_with_unknown_column("u1", "backend value", 0);
    user_only::store_download_response(res).unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(user.username, "user_1");
    assert_eq!(
        staged_unknown_data(&user),
        Value::String("backend value".to_string())
    );
}

#[tokio::test]
async fn test_download_with_unknown_column_overwrites_staged_value() {
    let db = get_clean_test_client_db();
    let mut conn = db.get_connection();

    let res = download_response_with_unknown_column("u1", "old value", 0);
    user_only::store_download_response(res).unwrap();
    assert_eq!(
        staged_unknown_data(&retrieve_stored_user(&mut conn, "u1")),
        Value::String("old value".to_string())
    );

    // Newer last_synced_at so the update is accepted; the staged value must be
    // overwritten in place rather than duplicated.
    let res = download_response_with_unknown_column("u1", "new value", 3600);
    user_only::store_download_response(res).unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(
        staged_unknown_data(&user),
        Value::String("new value".to_string())
    );
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        user.column_sync_metadata.clone().into();
    assert_eq!(metadata.unknown_data.len(), 1);
}
