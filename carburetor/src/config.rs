use std::sync::OnceLock;

use crate::error::Error;

static CONFIG: OnceLock<CarburetorGlobalConfig> = OnceLock::new();

pub fn initialize_carburetor_global_config(config: CarburetorGlobalConfig) {
    if CONFIG.set(config).is_err() {
        panic!("{}", Error::ConfigInit)
    }
}

#[cfg(any(for_backend, for_client))]
pub(crate) fn get_carburetor_config() -> &'static CarburetorGlobalConfig {
    CONFIG.get_or_init(|| CarburetorGlobalConfig::default())
}

#[derive(Debug, Clone)]
pub struct CarburetorGlobalConfig {
    #[cfg(for_backend)]
    pub database_url: String,

    #[cfg(for_client)]
    pub database_path: String,
}

impl Default for CarburetorGlobalConfig {
    fn default() -> Self {
        Self {
            #[cfg(for_backend)]
            database_url: "postgres://postgres:password@localhost:5432/".to_string(),

            #[cfg(for_client)]
            database_path: "./default.db".to_string(),
        }
    }
}
