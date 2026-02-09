use carburetor::{
    chrono::{DateTimeUtc, NaiveDate},
    helpers::get_connection,
};
use diesel::{ExpressionMethods, RunQueryDsl, dsl::insert_into};
use futures::StreamExt;
use sample_test_core::{
    backend_service::TestBackend,
    schema::{self, all_clients},
};
use tarpc::{context::Context, server::Channel};
use tokio::signal::unix::{SignalKind, signal};

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
    async fn process_download_request(
        self,
        _: Context,
        request: Option<all_clients::DownloadRequest>,
    ) -> all_clients::DownloadResponse {
        all_clients::process_download_request(request).unwrap()
    }

    async fn process_upload_request(
        self,
        _: Context,
        request: all_clients::UploadRequest,
    ) -> all_clients::UploadResponse {
        all_clients::process_upload_request(request).unwrap()
    }

    async fn test_helper_insert_user(
        self,
        _: Context,
        id: String,
        username: String,
        first_name: Option<String>,
        joined_on: NaiveDate,
        last_synced_at: DateTimeUtc,
        is_deleted: bool,
    ) {
        insert_into(schema::users::table)
            .values((
                schema::InsertUser {
                    id,
                    username,
                    first_name,
                    joined_on,
                    is_deleted,
                },
                schema::users::last_synced_at.eq(last_synced_at),
            ))
            .execute(&mut get_connection().unwrap())
            .unwrap();
    }
}
