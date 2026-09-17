use carburetor::error::Error;
use carburetor::helpers::carburetor_offset::retrieve_offsets;
use carburetor::models::DownloadTableResponseData;
use carburetor::serde_json::Value;
use e2e_test::{
    TestSyncGroup, get_clean_test_client_db,
    utils::{
        database_util::retrieve_stored_user,
        fixture_util::{
            build_download_response_with_unknown_column, build_dummy_download_response,
            get_user_staged_value,
        },
    },
};
use sample_test_core::schema::user_only;

/// A staged value whose type doesn't match the local column (a string staged
/// for the Integer `priority` column) makes `apply_backfill` return
/// `Error::ResetTable` naming the table, with the carburetor offset dropped
/// and the staged data cleared. The resync afterwards must converge: the
/// dropped offset forces a full re-fetch, and the unchanged row comes back
/// with the same `last_synced_at` it already has locally, so the row must
/// still be re-applied and the server's correct `priority` restored.
#[tokio::test]
async fn test_apply_backfill_type_mismatch_resets_table_and_resync_converges() {
    let db = get_clean_test_client_db(TestSyncGroup::UserOnly);
    let mut conn = db.get_connection();

    // Stage a string for the Integer `priority` column, simulating a backend that sent an
    // incompatible value.
    let res = build_download_response_with_unknown_column("u1", &[("priority", "not a number")], 0);
    user_only::store_download_response(res).unwrap();
    assert_eq!(
        get_user_staged_value(&retrieve_stored_user(&mut conn, "u1"), "priority"),
        Value::String("not a number".to_string())
    );
    // Precondition: staging stored an offset for the table, so the deletion
    // assertion after the reset is meaningful.
    assert!(retrieve_offsets(&mut conn).unwrap().contains_key("users"));

    let err = user_only::apply_backfill().unwrap_err();
    assert!(
        matches!(
            &err,
            Error::ResetTable { errors }
                if errors.len() == 1 && errors[0].table_name == "users"
        ),
        "expected Error::ResetTable naming users, got: {:?}",
        err
    );

    // The reset dropped the table's offset and cleared the staged data.
    assert!(!retrieve_offsets(&mut conn).unwrap().contains_key("users"));
    assert_eq!(
        get_user_staged_value(&retrieve_stored_user(&mut conn, "u1"), "priority"),
        Value::Null
    );

    // Resync: the dropped offset forces a full re-fetch. The server holds the
    // correct data (priority = 5), and the unchanged row comes back with the
    // same `last_synced_at` it already has locally, so it must still be
    // re-applied: the server's correct `priority` is restored instead of the
    // column staying at its local default until the row happens to change on
    // the backend.
    let stored = retrieve_stored_user(&mut conn, "u1");
    let mut res = build_dummy_download_response("u1", 0);
    let DownloadTableResponseData::Update(ref mut update) = res.user.data[0];
    update.priority = 5;
    update.last_synced_at = stored
        .last_synced_at
        .expect("downloaded user must have last_synced_at");
    user_only::store_download_response(res).unwrap();

    let user = retrieve_stored_user(&mut conn, "u1");
    assert_eq!(
        user.priority, 5,
        "unchanged row must be re-applied so the server's priority is restored"
    );
}
