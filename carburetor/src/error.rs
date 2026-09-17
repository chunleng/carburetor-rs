use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(
        "Configuration initialization failed. Config has been applied multiple times, or after the framework is running"
    )]
    ConfigInit,

    #[error("Migration error: {0}")]
    Migration(String),

    #[error(
        "Unrecoverable migration failure: local database was wiped and recreated from scratch. Original error: {source}"
    )]
    DatabaseWiped {
        #[source]
        source: Box<Error>,
    },

    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Unknown error: {message}\n{source}")]
    Unhandled {
        message: String,
        #[source]
        source: anyhow::Error,
    },

    #[error(
        "Staged values do not match the local column types. The tables have been reset: {details}",
        details = errors
            .iter()
            .map(|e| format!("`{}`: {}", e.table_name, e.source))
            .collect::<Vec<_>>()
            .join(", ")
    )]
    ResetTable { errors: Vec<ResetTableEntry> },
}

#[derive(Debug)]
pub struct ResetTableEntry {
    pub table_name: String,
    pub source: Box<dyn std::error::Error + Send + Sync + 'static>,
}
