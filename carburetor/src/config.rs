use std::sync::OnceLock;

use crate::error::Error;

static CONFIG: OnceLock<CarburetorGlobalConfig> = OnceLock::new();

pub fn initialize_carburetor_global_config(config: CarburetorGlobalConfig) {
    if CONFIG.set(config).is_err() {
        panic!("{}", Error::ConfigInit)
    }
}

pub(crate) fn get_carburetor_config() -> &'static CarburetorGlobalConfig {
    CONFIG.get_or_init(|| CarburetorGlobalConfig::default())
}

#[derive(Debug, Clone)]
pub struct CarburetorGlobalConfig {
    #[cfg(feature = "backend")]
    pub database_url: String,

    #[cfg(feature = "client")]
    pub database_path: String,
}

impl Default for CarburetorGlobalConfig {
    fn default() -> Self {
        Self {
            #[cfg(feature = "backend")]
            database_url: "postgres://postgres:password@localhost:5432/".to_string(),

            #[cfg(feature = "client")]
            database_path: "./default.db".to_string(),
        }
    }
}
