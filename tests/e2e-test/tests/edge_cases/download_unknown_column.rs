use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;
use carburetor::serde_json::Value;
use e2e_test::{
    TestSyncGroup, get_clean_test_client_db,
    utils::{
        database_util::retrieve_stored_user,
        fixture_util::{build_download_response_with_unknown_column, get_user_staged_value},
    },
};
use sample_test_core::schema::user_only;

#[tokio::test]
async fn test_download_with_unknown_column_stages_value_on_insert() {
    let db = get_clean_test_client_db(TestSyncGroup::UserOnly);
    let mut conn = db.get_connection();

    let res =
        build_download_response_with_unknown_column("u1", &[("future_column", "backend value")], 0);
    user_only::store_download_response(res).unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(user.username, "user_1");
    assert_eq!(
        get_user_staged_value(&user, "future_column"),
        Value::String("backend value".to_string())
    );
}

#[tokio::test]
async fn test_download_with_unknown_column_overwrites_staged_value() {
    let db = get_clean_test_client_db(TestSyncGroup::UserOnly);
    let mut conn = db.get_connection();

    let res =
        build_download_response_with_unknown_column("u1", &[("future_column", "old value")], 0);
    user_only::store_download_response(res).unwrap();
    assert_eq!(
        get_user_staged_value(&retrieve_stored_user(&mut conn, "u1"), "future_column"),
        Value::String("old value".to_string())
    );

    // Newer last_synced_at so the update is accepted; the staged value must be
    // overwritten in place rather than duplicated.
    let res =
        build_download_response_with_unknown_column("u1", &[("future_column", "new value")], 3600);
    user_only::store_download_response(res).unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(
        get_user_staged_value(&user, "future_column"),
        Value::String("new value".to_string())
    );
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        user.column_sync_metadata.clone().into();
    assert_eq!(metadata.unknown_data.len(), 1);
}
