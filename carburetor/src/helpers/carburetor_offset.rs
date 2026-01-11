use std::collections::HashMap;

use chrono::{DateTime, Utc};
use diesel::prelude::*;

use crate::error::{Error, Result};

diesel::table! {
    carburetor_offsets (table_name) {
        table_name -> Text,
        cutoff_at -> TimestamptzSqlite,
    }
}

#[derive(Debug, Clone, Queryable, Selectable, Insertable)]
#[diesel(table_name = carburetor_offsets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct FullCarburetorOffset {
    pub table_name: String,
    pub cutoff_at: DateTime<Utc>,
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = carburetor_offsets)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ChangesetCarburetorOffset {
    pub table_name: String,
    pub cutoff_at: Option<DateTime<Utc>>,
}

pub fn upsert_offset(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
    cutoff_at: DateTime<Utc>,
) -> crate::error::Result<()> {
    use crate::error::Error;

    let offset = FullCarburetorOffset {
        table_name: table_name.to_string(),
        cutoff_at,
    };

    diesel::replace_into(carburetor_offsets::table)
        .values(&offset)
        .execute(conn)
        .map_err(|e| Error::Unhandled {
            message: format!("Failed to update offset for table '{}'", table_name),
            source: e.into(),
        })?;

    Ok(())
}

pub fn retrieve_offsets(
    conn: &mut diesel::SqliteConnection,
) -> Result<HashMap<String, DateTime<Utc>>> {
    let offsets = carburetor_offsets::table
        .load::<FullCarburetorOffset>(conn)
        .map_err(|e| Error::Unhandled {
            message: "Failed to retrieve offsets".to_string(),
            source: e.into(),
        })?
        .into_iter()
        .map(|o| (o.table_name, o.cutoff_at))
        .collect();

    Ok(offsets)
}
