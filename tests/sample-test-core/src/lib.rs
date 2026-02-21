pub mod backend_service {
    use carburetor::chrono::NaiveDate;

    use crate::schema::all_clients::{
        self, DownloadRequest, DownloadResponse, UploadRequest, UploadResponse,
    };

    #[tarpc::service]
    pub trait TestBackend {
        // Backend functions
        async fn process_download_request(request: Option<DownloadRequest>) -> DownloadResponse;
        async fn process_upload_request(request: UploadRequest) -> UploadResponse;

        // Test helper functions
        async fn test_helper_insert_user(
            id: String,
            username: String,
            first_name: Option<String>,
            joined_on: NaiveDate,
            is_deleted: bool,
        );
        async fn test_helper_get_user(id: String) -> all_clients::DownloadUpdateUser;
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
            }
        }
        sync_groups {
            all_clients {
                user
            }
        }
    }
}
