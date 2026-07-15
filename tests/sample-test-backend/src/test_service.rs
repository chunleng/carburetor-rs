use carburetor::{
    chrono::{DateTimeUtc, NaiveDate},
    helpers::{get_connection, get_db_utc_now},
};
use diesel::{ExpressionMethods, QueryDsl, QueryableByName, RunQueryDsl, dsl::insert_into};
use futures::StreamExt;
use sample_test_core::{
    ColumnMeta,
    backend_service::TestBackend,
    schema::{self, all_clients, user_only},
};
use tarpc::{context::Context, server::Channel};
use tokio::signal::unix::{SignalKind, signal};

#[derive(Debug, QueryableByName)]
struct ColumnRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    column_name: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    is_primary_key: bool,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    is_nullable: bool,
}

#[derive(Debug, Clone)]
pub struct TestService;

impl TestService {
    pub async fn start(port: u16) {
        let mut listener = tarpc::serde_transport::tcp::listen(
            format!("127.0.0.1:{}", port),
            tarpc::tokio_serde::formats::Bincode::default,
        )
        .await
        .unwrap();

        listener.config_mut().max_frame_length(usize::MAX);
        let mut sigterm = signal(SignalKind::terminate()).expect("Failed to setup SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to setup SIGINT handler");

        loop {
            tokio::select! {
                conn = listener.next() => {
                    let Some(conn) = conn else {
                        break;
                    };

                    let conn = match conn {
                        Ok(conn) => conn,
                        Err(e) => {
                            eprintln!("Connection error: {}", e);
                            continue;
                        }
                    };

                    tokio::spawn(async move {
                        let server = tarpc::server::BaseChannel::with_defaults(conn);
                        server
                            .execute(Self.serve())
                            .for_each(|response| async move {
                                tokio::spawn(response);
                            })
                            .await;
                    });
                }
                _ = sigterm.recv() => { break; }
                _ = sigint.recv() => { break; }
            }
        }
    }
}

impl TestBackend for TestService {
    async fn process_user_only_download_request(
        self,
        _: Context,
        request: Option<user_only::DownloadRequest>,
    ) -> user_only::DownloadResponse {
        user_only::process_download_request(request).unwrap()
    }

    async fn process_user_only_upload_request(
        self,
        _: Context,
        request: user_only::UploadRequest,
    ) -> user_only::UploadResponse {
        user_only::process_upload_request(request).unwrap()
    }

    async fn process_all_clients_download_request(
        self,
        _: Context,
        request: Option<all_clients::DownloadRequest>,
        context_user_id: String,
    ) -> all_clients::DownloadResponse {
        let context = all_clients::SyncContext {
            user_id: context_user_id,
        };
        all_clients::process_download_request(request, &context).unwrap()
    }

    async fn process_all_clients_upload_request(
        self,
        _: Context,
        request: all_clients::UploadRequest,
        context_user_id: String,
    ) -> all_clients::UploadResponse {
        let context = all_clients::SyncContext {
            user_id: context_user_id,
        };
        all_clients::process_upload_request(request, &context).unwrap()
    }

    async fn test_helper_insert_user(
        self,
        _: Context,
        id: String,
        username: String,
        first_name: Option<String>,
        joined_on: NaiveDate,
        created_at: DateTimeUtc,
        is_deleted: bool,
    ) {
        let mut conn = get_connection().unwrap();
        let utc_now = get_db_utc_now(&mut conn).unwrap();
        insert_into(schema::users::table)
            .values((
                schema::InsertableUser {
                    id,
                    username,
                    first_name,
                    joined_on,
                    created_at,
                    is_deleted,
                },
                schema::users::last_synced_at.eq(utc_now),
            ))
            .execute(&mut get_connection().unwrap())
            .unwrap();
    }

    async fn test_helper_insert_message(
        self,
        _: Context,
        id: String,
        recipient_id: String,
        subject: String,
        body: String,
        is_deleted: bool,
    ) {
        let mut conn = get_connection().unwrap();
        let utc_now = get_db_utc_now(&mut conn).unwrap();
        insert_into(schema::messages::table)
            .values((
                schema::InsertableMessage {
                    id,
                    recipient_id,
                    subject,
                    body,
                    is_deleted,
                },
                schema::messages::last_synced_at.eq(utc_now),
            ))
            .execute(&mut get_connection().unwrap())
            .unwrap();
    }

    async fn test_helper_get_user_last_synced_at(self, _: Context, id: String) -> DateTimeUtc {
        schema::users::table
            .find(&id)
            .select(schema::users::last_synced_at)
            .first(&mut get_connection().unwrap())
            .unwrap()
    }

    async fn test_helper_get_table_columns(
        self,
        _: Context,
        table_name: String,
    ) -> Vec<ColumnMeta> {
        diesel::sql_query(
            "SELECT c.column_name, \
               CASE WHEN pk.column_name IS NOT NULL THEN true ELSE false END AS is_primary_key, \
               CASE WHEN c.is_nullable = 'YES' THEN true ELSE false END AS is_nullable \
             FROM information_schema.columns c \
             LEFT JOIN ( \
               SELECT kcu.column_name \
               FROM information_schema.table_constraints tc \
               JOIN information_schema.key_column_usage kcu \
                 ON tc.constraint_name = kcu.constraint_name \
                 AND tc.table_schema = kcu.table_schema \
               WHERE tc.constraint_type = 'PRIMARY KEY' \
                 AND tc.table_name = $1 \
             ) pk ON c.column_name = pk.column_name \
             WHERE c.table_name = $2 \
             ORDER BY c.ordinal_position",
        )
        .bind::<diesel::sql_types::Text, _>(&table_name)
        .bind::<diesel::sql_types::Text, _>(&table_name)
        .load::<ColumnRow>(&mut get_connection().unwrap())
        .unwrap()
        .into_iter()
        .map(|row| ColumnMeta {
            name: row.column_name,
            is_primary_key: row.is_primary_key,
            is_nullable: row.is_nullable,
        })
        .collect()
    }
}
