use futures::StreamExt;
use sample_test_core::{backend_service::TestBackend, schema::all_clients};
use tarpc::server::Channel;
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
        _: tarpc::context::Context,
        request: Option<all_clients::DownloadRequest>,
    ) -> all_clients::DownloadResponse {
        all_clients::process_download_request(request).unwrap()
    }
}

