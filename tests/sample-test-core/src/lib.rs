pub mod backend_service {
    use carburetor::chrono::{DateTimeUtc, NaiveDate};

    use crate::schema::user_only;

    #[tarpc::service]
    pub trait TestBackend {
        // Backend functions
        async fn process_user_only_download_request(
            request: Option<user_only::DownloadRequest>,
        ) -> user_only::DownloadResponse;
        async fn process_user_only_upload_request(
            request: user_only::UploadRequest,
        ) -> user_only::UploadResponse;

        // Test helper functions
        async fn test_helper_insert_user(
            id: String,
            username: String,
            first_name: Option<String>,
            joined_on: NaiveDate,
            created_at: DateTimeUtc,
            is_deleted: bool,
        );
        async fn test_helper_get_user_last_synced_at(id: String) -> DateTimeUtc;
    }
}

pub mod schema {
    use carburetor::prelude::*;

    carburetor_sync_config! {
        tables {
            user {
                username -> Text,
                first_name -> Nullable<Text>,
                joined_on -> Date,
                #[immutable]
                created_at -> Timestamptz,
            }
        }
        sync_groups {
            user_only {
                user
            }
        }
    }
}
