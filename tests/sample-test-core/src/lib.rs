pub mod backend_service {
    use carburetor::chrono::{DateTimeUtc, NaiveDate};

    use crate::schema::{all_clients, user_only};

    #[tarpc::service]
    pub trait TestBackend {
        // Backend functions
        async fn process_user_only_download_request(
            request: Option<user_only::DownloadRequest>,
        ) -> user_only::DownloadResponse;
        async fn process_user_only_upload_request(
            request: user_only::UploadRequest,
        ) -> user_only::UploadResponse;
        async fn process_all_clients_download_request(
            request: Option<all_clients::DownloadRequest>,
            context_user_id: String,
        ) -> all_clients::DownloadResponse;
        async fn process_all_clients_upload_request(
            request: all_clients::UploadRequest,
            context_user_id: String,
        ) -> all_clients::UploadResponse;

        // Test helper functions
        async fn test_helper_insert_user(
            id: String,
            username: String,
            first_name: Option<String>,
            joined_on: NaiveDate,
            created_at: DateTimeUtc,
            is_deleted: bool,
        );
        async fn test_helper_insert_message(
            id: String,
            recipient_id: String,
            subject: String,
            body: String,
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
            message {
                #[immutable]
                recipient_id -> Text,
                subject -> Text,
                body -> Text,
            }
        }
        sync_groups {
            user_only {
                user
            }
            all_clients {
                user,
                message(
                    restrict_to = $user_id,
                    restrict_to_column = recipient_id,
                )
            }
        }
    }
}
