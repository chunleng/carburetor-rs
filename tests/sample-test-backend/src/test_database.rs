use diesel::{Connection, PgConnection, RunQueryDsl};
use testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner};
use testcontainers_modules::postgres::Postgres;

pub struct TestDatabase {
    pub database_url: String,

    _container: ContainerAsync<Postgres>,
}

impl TestDatabase {
    pub async fn start() -> Self {
        let container = Postgres::default()
            .with_tag("16")
            .start()
            .await
            .expect("Failed to start postgres container");

        let host_port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("Failed to get postgres port");
        let database_url = format!(
            "postgres://postgres:postgres@localhost:{}/postgres",
            host_port
        );

        Self::init_database(&mut Self::get_connection(&database_url));

        Self {
            _container: container,
            database_url,
        }
    }

    fn get_connection(database_url: &str) -> PgConnection {
        let mut retries = 10;
        loop {
            match PgConnection::establish(database_url) {
                Ok(conn) => break conn,
                Err(_) if retries > 0 => {
                    println!("Connection failed, retrying... ({} attempts left)", retries);
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    retries -= 1;
                }
                Err(e) => panic!("Failed to connect to database: {:?}", e),
            }
        }
    }

    fn init_database(conn: &mut PgConnection) {
        diesel::sql_query(
            "CREATE TABLE users(
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                first_name TEXT,
                joined_on DATE,
                last_synced_at TIMESTAMPTZ,
                is_deleted BOOLEAN
            )",
        )
        .execute(conn)
        .expect("Failed to create users table");
    }
}
