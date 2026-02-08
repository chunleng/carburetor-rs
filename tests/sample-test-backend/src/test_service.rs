use futures::StreamExt;
use sample_test_core::{backend_service::TestBackend, schema::all_clients};
use tarpc::server::Channel;

#[derive(Debug, Clone)]
pub struct TestService;

impl TestService {
    pub async fn start() {
        let mut listener = tarpc::serde_transport::tcp::listen(
            "127.0.0.1:8080",
            tarpc::tokio_serde::formats::Bincode::default,
        )
        .await
        .unwrap();

        listener.config_mut().max_frame_length(usize::MAX);

        while let Some(conn) = listener.next().await {
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
