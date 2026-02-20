#[cfg(feature = "client")]
pub mod carburetor_offset;
#[cfg(feature = "client")]
pub mod client_sync_metadata;

use chrono::{DateTime, Utc};

#[cfg(feature = "backend")]
pub fn get_db_utc_now(
    conn: &mut diesel::PgConnection,
) -> crate::error::Result<DateTime<Utc>> {
    use diesel::RunQueryDsl;
    use diesel::dsl::sql;
    use diesel::sql_types::Timestamptz;
    sql::<Timestamptz>("SELECT CURRENT_TIMESTAMP")
        .get_result(conn)
        .map_err(|e: diesel::result::Error| crate::error::Error::Unhandled {
            message: "Failed to get database time".to_string(),
            source: e.into(),
        })
}

#[cfg(feature = "backend")]
pub fn get_connection() -> crate::error::Result<diesel::PgConnection> {
    use crate::{config::get_carburetor_config, error::Error};
    use diesel::{Connection, PgConnection};
    Ok(
        PgConnection::establish(&get_carburetor_config().database_url.clone()).map_err(|e| {
            Error::Unhandled {
                message: "Connection to PostgresDB failed".to_string(),
                source: e.into(),
            }
        })?,
    )
}

#[cfg(feature = "client")]
pub fn get_connection() -> crate::error::Result<diesel::SqliteConnection> {
    use crate::{config::get_carburetor_config, error::Error};
    use diesel::{Connection, SqliteConnection, connection::SimpleConnection};
    let mut conn =
        SqliteConnection::establish(&get_carburetor_config().database_path).map_err(|e| {
            Error::Unhandled {
                message: "Connection to Sqlite failed".to_string(),
                source: e.into(),
            }
        })?;

    // Default Sqlite lock fails immediately, this config ensure that connection tries to acquire
    // lock for 5 seconds before failing
    conn.batch_execute("PRAGMA busy_timeout = 5000;")
        .map_err(|e| Error::Unhandled {
            message: "Updating config for Sqlite failed".to_string(),
            source: e.into(),
        })?;

    Ok(conn)
}

pub fn get_utc_now() -> DateTime<Utc> {
    Utc::now()
}

pub fn generate_id(prefix: String) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let now = get_utc_now();
    let mut hasher = DefaultHasher::new();
    now.timestamp_nanos_opt().hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}-{:06x}-{}", prefix, hash & 0xFFFFFF, now.timestamp())
}
