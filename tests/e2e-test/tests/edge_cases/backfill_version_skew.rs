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
async fn test_apply_backfill_retains_still_unknown_column() {
    let db = get_clean_test_client_db(TestSyncGroup::UserOnly);
    let mut conn = db.get_connection();

    // Multi-version skew: the backend knows `nickname` (applied) and
    // `future_column` (still unknown to this client).
    user_only::store_download_response(build_download_response_with_unknown_column(
        "u1",
        &[
            ("nickname", "staged nickname"),
            ("future_column", "still unknown"),
        ],
        0,
    ))
    .unwrap();

    user_only::apply_backfill().unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(user.nickname, Some("staged nickname".to_string()));
    // The still-unknown column remains staged for a future upgrade.
    assert_eq!(
        get_user_staged_value(&user, "future_column"),
        Value::String("still unknown".to_string())
    );
    assert_eq!(get_user_staged_value(&user, "nickname"), Value::Null);
}

#[tokio::test]
async fn test_apply_backfill_applies_soft_deleted_row_without_resurrection() {
    let db = get_clean_test_client_db(TestSyncGroup::UserOnly);
    let mut conn = db.get_connection();

    // A soft-deleted row still carries staged data from before its deletion.
    let mut response =
        build_download_response_with_unknown_column("u1", &[("nickname", "staged nickname")], 0);
    let carburetor::models::DownloadTableResponseData::Update(ref mut update) =
        response.user.data[0];
    update.is_deleted = true;
    user_only::store_download_response(response).unwrap();

    user_only::apply_backfill().unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    // Applied without resurrection.
    assert!(user.is_deleted);
    assert_eq!(user.nickname, Some("staged nickname".to_string()));
    assert_eq!(get_user_staged_value(&user, "nickname"), Value::Null);
}
