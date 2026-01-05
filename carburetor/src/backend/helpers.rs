use chrono::{DateTime, Utc};
use diesel::{Connection, PgConnection};

use crate::{
    config::get_carburetor_config,
    error::{Error, Result},
};

pub fn get_connection() -> Result<PgConnection> {
    Ok(
        PgConnection::establish(&get_carburetor_config().database_url.clone()).map_err(|e| {
            Error::Unhandled {
                message: "Connection to PostgresDB failed".to_string(),
                source: e.into(),
            }
        })?,
    )
}

pub fn get_utc_now() -> DateTime<Utc> {
    Utc::now()
}
