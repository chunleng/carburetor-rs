use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(
        "Configuration initialization failed. Config has been applied multiple times, or after the framework is running"
    )]
    ConfigInit,

    #[error("Unknown error: {message}\n{source}")]
    Unhandled {
        message: String,
        #[source]
        source: anyhow::Error,
    },
}
