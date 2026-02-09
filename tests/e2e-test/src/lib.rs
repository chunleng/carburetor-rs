use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::time::Duration;

use sample_test_core::backend_service::TestBackendClient;

pub struct TestBackendHandle {
    process: Child,
    port: u16,
}

impl TestBackendHandle {
    pub fn start() -> Self {
        let port = Self::find_available_port();

        let mut process = Command::new("cargo")
            .args([
                "run",
                "-p",
                "sample-test-backend",
                "--features",
                "backend",
                "--",
            ])
            .arg(port.to_string())
            .spawn()
            .expect("Failed to start sample-test-backend");

        let addr = format!("127.0.0.1:{}", port);
        for _ in 0..100 {
            if std::net::TcpStream::connect(&addr).is_ok() {
                return Self { process, port };
            }
            std::thread::sleep(Duration::from_millis(100));
        }

        Self::graceful_kill(&mut process);

        panic!("Fail to start the TestBackend server");
    }

    pub async fn client(&self) -> TestBackendClient {
        let transport = tarpc::serde_transport::tcp::connect(
            format!("127.0.0.1:{}", self.port),
            tarpc::tokio_serde::formats::Bincode::default,
        )
        .await
        .unwrap();

        TestBackendClient::new(tarpc::client::Config::default(), transport).spawn()
    }

    fn find_available_port() -> u16 {
        TcpListener::bind("127.0.0.1:0")
            .expect("Failed to bind to ephemeral port")
            .local_addr()
            .expect("Failed to get local addr")
            .port()
    }

    fn graceful_kill(process: &mut Child) {
        let pid = Pid::from_raw(process.id() as i32);
        let _ = kill(pid, Signal::SIGTERM);

        for _ in 0..50 {
            match process.try_wait() {
                Ok(Some(_)) => {
                    println!("Backend process exited gracefully");
                    return;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => break,
            }
        }

        println!("Backend process didn't exit gracefully, force killing");
        let _ = process.kill();
        let _ = process.wait();
    }
}

impl Drop for TestBackendHandle {
    fn drop(&mut self) {
        Self::graceful_kill(&mut self.process);
    }
}
