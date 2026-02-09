use carburetor::config::{CarburetorGlobalConfig, initialize_carburetor_global_config};
use carburetor::helpers::get_connection;
use diesel::{RunQueryDsl, SqliteConnection};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::fs;
use std::net::TcpListener;
use std::process::{Child, Command};
use std::sync::OnceLock;
use std::time::Duration;
use tempfile::TempDir;

use sample_test_core::backend_service::TestBackendClient;

static TEST_CLIENT_DB: OnceLock<TestClientDatabase> = OnceLock::new();

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

pub struct TestClientDatabase {
    _temp_dir: TempDir,
}

impl TestClientDatabase {
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let database_path = temp_dir
            .path()
            .join("test.db")
            .to_string_lossy()
            .to_string();

        // Only initialize if not already initialized
        let _ = std::panic::catch_unwind(|| {
            initialize_carburetor_global_config(CarburetorGlobalConfig {
                database_path: database_path.clone(),
            });
        });

        Self {
            _temp_dir: temp_dir,
        }
    }

    pub fn get_connection(&self) -> SqliteConnection {
        get_connection().unwrap()
    }

    pub fn reset(&self) {
        let db_path = self._temp_dir.path().join("test.db");
        // Remove the database file if it exists
        let _ = fs::remove_file(&db_path);

        // Re-initialize the config to point to the (now missing) db file
        // (This is safe because the config is already set, and get_connection uses the same path)
        let mut conn = get_connection().unwrap();

        // Recreate tables
        diesel::sql_query(
            "CREATE TABLE users(
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                first_name TEXT,
                joined_on DATE NOT NULL,
                last_synced_at TIMESTAMPTZ,
                is_deleted BOOLEAN NOT NULL,
                dirty_flag TEXT,
                column_sync_metadata JSON NOT NULL
            )",
        )
        .execute(&mut conn)
        .expect("Failed to create users table");

        diesel::sql_query(
            "CREATE TABLE carburetor_offsets(
                table_name TEXT PRIMARY KEY,
                cutoff_at TIMESTAMPTZ NOT NULL
            )",
        )
        .execute(&mut conn)
        .expect("Failed to create carburetor_offsets table");
    }
}

pub fn get_clean_test_client_db() -> &'static TestClientDatabase {
    let db = TEST_CLIENT_DB.get_or_init(|| TestClientDatabase::new());
    db.reset();
    db
}
