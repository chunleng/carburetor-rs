use std::time::Duration;

use carburetor::chrono::NaiveDate;
use carburetor::helpers::client_sync_metadata::ClientSyncMetadata;
use carburetor::models::{DownloadTableResponse, DownloadTableResponseData};
use carburetor::serde_json::{Map, Value};
use sample_test_core::schema::user_only;

/// Builds a download response containing a single user row.
pub fn build_dummy_download_response(
    id: &str,
    last_synced_at_offset_secs: u64,
) -> user_only::DownloadResponse {
    user_only::DownloadResponse {
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
    }
}

/// Builds a download response whose user row carries `unknown_data` entries,
/// simulating a backend that knows columns the client schema doesn't model.
/// The entries are populated directly on the row's `unknown_data` field, so
/// `store_download_response` stages them into the sync metadata.
pub fn build_download_response_with_unknown_column(
    id: &str,
    unknown_data: &[(&str, &str)],
    last_synced_at_offset_secs: u64,
) -> user_only::DownloadResponse {
    let mut response = build_dummy_download_response(id, last_synced_at_offset_secs);
    let carburetor::models::DownloadTableResponseData::Update(ref mut update) =
        response.user.data[0];
    update.unknown_data = unknown_data
        .iter()
        .map(|(column, value)| (column.to_string(), Value::String(value.to_string())))
        .collect();
    response
}

/// Reads the staged value for `column` from the user's sync metadata
/// (`.unknown_data` dictionary). Returns `Value::Null` when nothing is staged for
/// the column.
pub fn get_user_staged_value(user: &user_only::FullUser, column: &str) -> Value {
    let metadata: ClientSyncMetadata<user_only::UserSyncMetadata> =
        user.column_sync_metadata.clone().into();
    Value::from(metadata)[".unknown_data"][column].clone()
}
