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
    pub database_url: String,
}

impl Default for CarburetorGlobalConfig {
    fn default() -> Self {
        Self {
            database_url: "postgres://postgres:password@localhost:5432/".to_string(),
        }
    }
}
