use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadTableResponse<T> {
    pub cutoff_at: DateTime<Utc>,
    pub data: Vec<DownloadTableResponseData<T>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DownloadTableResponseData<T> {
    Update(T),
    // For future support of feature such as Postgres WAL update
    // UpdatePartial(U)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadTableResponseData {
    pub id: String,
    pub last_synced_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadTableResponseError {
    pub id: String,
    pub code: UploadTableResponseErrorType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum UploadTableResponseErrorType {
    Unknown,
    RecordNotFound,
    RecordAlreadyExists,
}
