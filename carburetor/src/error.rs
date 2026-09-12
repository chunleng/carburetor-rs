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
}
