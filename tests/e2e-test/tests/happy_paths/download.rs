use e2e_test::TestBackendHandle;
use tarpc::context::current as ctx;

#[tokio::test]
async fn test_clean_download() {
    let backend_server = TestBackendHandle::start();
    let backend = backend_server.client().await;

    let _ = dbg!(backend.process_download_request(ctx(), None).await);
}
