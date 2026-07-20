use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ColumnMeta {
    pub name: String,
    pub is_primary_key: bool,
    pub is_nullable: bool,
    pub column_default: Option<String>,
}

pub mod backend_service {
    use carburetor::chrono::{DateTimeUtc, NaiveDate};

    use crate::ColumnMeta;

    #[tarpc::service]
    pub trait TestBackend {
        // Backend functions
        async fn process_user_only_download_request(request_json: String) -> String;
        async fn process_user_only_upload_request(request_json: String) -> String;
        async fn process_all_clients_download_request(
            request_json: String,
            context_user_id: String,
        ) -> String;
        async fn process_all_clients_upload_request(
            request_json: String,
            context_user_id: String,
        ) -> String;

        // Test helper functions
        async fn test_helper_insert_user(
            id: String,
            username: String,
            first_name: Option<String>,
            joined_on: NaiveDate,
            created_at: DateTimeUtc,
            is_deleted: bool,
            nickname: Option<String>,
            priority: Option<i32>,
            preferences: Option<Option<String>>,
        );
        async fn test_helper_insert_message(
            id: String,
            recipient_id: String,
            subject: String,
            body: String,
            notes: Option<String>,
            is_deleted: bool,
        );
        async fn test_helper_get_user_last_synced_at(id: String) -> DateTimeUtc;
        async fn test_helper_get_table_columns(table_name: String) -> Vec<ColumnMeta>;
        async fn test_helper_get_database_url() -> String;
        async fn test_helper_rerun_migrations() -> Result<(), String>;
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
                #[default(rust = "carburetor::helpers::get_utc_now()")]
                created_at -> Timestamptz,
                #[default(rust = "Some(\"default_nickname\".to_string())")]
                nickname -> Nullable<Text>,
                #[default(sql = Number(0))]
                priority -> Integer,
                #[default(sql = Text("no preference"))]
                preferences -> Nullable<Text>,
            }
            message {
                #[immutable]
                recipient_id -> Text,
                subject -> Text,
                body -> Text,
                notes -> Nullable<Text>,
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
