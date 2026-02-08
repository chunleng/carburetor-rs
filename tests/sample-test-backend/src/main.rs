mod test_database;
mod test_service;

use carburetor::config::{CarburetorGlobalConfig, initialize_carburetor_global_config};
use test_database::TestDatabase;

use crate::test_service::TestService;

#[tokio::main]
async fn main() {
    let test_db = TestDatabase::start().await;

    initialize_carburetor_global_config(CarburetorGlobalConfig {
        database_url: test_db.database_url.clone(),
    });

    TestService::start().await;
}
