pub mod backend_service {
    use crate::schema::all_clients::{DownloadRequest, DownloadResponse};

    #[tarpc::service]
    pub trait TestBackend {
        async fn process_download_request(request: Option<DownloadRequest>) -> DownloadResponse;
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
