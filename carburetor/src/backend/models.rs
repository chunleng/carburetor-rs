use chrono::{DateTime, Utc};

#[derive(Debug, Clone)]
pub struct DownloadSyncResponse<T> {
    pub last_sync_at: DateTime<Utc>,
    pub data: Vec<T>,
}
