use chrono::{DateTime, Utc};
use diesel::RunQueryDsl;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value, from_value, to_value};

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub enum DirtyFlag {
    Insert,
    Update,
}

impl ToString for DirtyFlag {
    fn to_string(&self) -> String {
        match self {
            Self::Insert => "insert".to_string(),
            Self::Update => "update".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dirty_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_last_synced_at: Option<DateTime<Utc>>,
}

fn is_empty<T: Serialize>(value: &T) -> bool {
    match to_value(value) {
        Ok(Value::Object(map)) => map.is_empty(),
        _ => false,
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ClientSyncMetadata<T> {
    #[serde(rename = ".insert_time", skip_serializing_if = "Option::is_none")]
    pub insert_time: Option<DateTime<Utc>>,

    #[serde(flatten, skip_serializing_if = "is_empty")]
    pub data: Option<T>,

    // Note: unknown data is mainly for DB migration use to recover data in the future
    #[serde(
        default,
        rename = ".unknown_data",
        skip_serializing_if = "Map::is_empty"
    )]
    pub unknown_data: Map<String, Value>,
}

impl<T> ClientSyncMetadata<T> {
    /// Stages values of columns unknown to the client schema, keyed by column name. Re-downloading
    /// the same row overwrites the previously staged value (latest server value wins).
    pub fn stage_unknown_data(&mut self, unknown_data: &Map<String, Value>) {
        for (column, value) in unknown_data {
            self.unknown_data.insert(column.clone(), value.clone());
        }
    }
}

/// Clears the staged `.unknown_data` entries of every row in `table_name`, leaving the rest of
/// the metadata JSON intact. Rows without staged data are untouched (json_remove on a missing
/// key is a no-op).
pub fn clear_table_unknown_data(
    conn: &mut diesel::SqliteConnection,
    table_name: &str,
    metadata_column: &str,
) -> Result<()> {
    let query = format!(
        "UPDATE {table_name} SET {metadata_column} = json_remove({metadata_column}, '$.\".unknown_data\"')"
    );
    diesel::sql_query(&query)
        .execute(conn)
        .map_err(|e| Error::Unhandled {
            message: format!("Failed to clear staged data for table '{table_name}'"),
            source: e.into(),
        })?;

    Ok(())
}

impl<T: DeserializeOwned> From<Value> for ClientSyncMetadata<T> {
    fn from(value: Value) -> Self {
        from_value(value).unwrap()
    }
}

impl<T: Serialize> From<ClientSyncMetadata<T>> for Value {
    fn from(item: ClientSyncMetadata<T>) -> Self {
        to_value(item).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use diesel::{Connection, RunQueryDsl};
    use serde::Deserialize;
    use serde_json::json;

    #[derive(Debug, Clone, Default, Deserialize, Serialize)]
    struct User {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<Metadata>,
        #[serde(skip_serializing_if = "Option::is_none")]
        created_at: Option<Metadata>,
    }

    #[test]
    fn test_empty() {
        // This test is important to ensure that we are not storing extra information when we
        // retrieve and put back into the database.
        let value: Value = json!({});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        assert_eq!(value, Value::from(metadata));
    }

    #[test]
    fn test_partial_filled_known_data() {
        let value: Value = json!({"name": {}});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        assert_eq!(value, Value::from(metadata));
    }

    #[test]
    fn test_partial_filled_metadata() {
        let value: Value = json!({"name": {"dirty_at": "2025-01-01T00:00:00Z"}});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        assert_eq!(value, Value::from(metadata));
    }

    #[test]
    fn test_insert_time_metadata() {
        let value: Value = json!({".insert_time": "2025-05-01T00:00:00Z"});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        assert_eq!(value, Value::from(metadata));
    }

    #[test]
    fn test_empty_unknown_data_dropped_on_roundtrip() {
        let metadata: ClientSyncMetadata<User> =
            serde_json::from_value(json!({".unknown_data": {}})).unwrap();

        assert_eq!(Value::from(metadata), json!({}));
    }

    #[test]
    fn test_staged_data_survives_metadata_roundtrip() {
        // Staged entries are persisted in the metadata JSON column; they must
        // survive a serialize -> deserialize roundtrip intact.
        let value: Value = json!({".unknown_data": {"name": "Alice", "future_column": "staged"}});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        let roundtripped: ClientSyncMetadata<User> = Value::from(metadata).into();

        assert_eq!(value, Value::from(roundtripped));
    }

    #[test]
    fn test_stage_unknown_data_overwrites_previous_value() {
        let mut metadata: ClientSyncMetadata<User> = ClientSyncMetadata::default();
        let mut old_data = Map::new();
        old_data.insert("new_column".to_string(), json!("old value"));
        metadata.stage_unknown_data(&old_data);

        let mut new_data = Map::new();
        new_data.insert("new_column".to_string(), json!("new value"));
        metadata.stage_unknown_data(&new_data);

        assert_eq!(metadata.unknown_data["new_column"], json!("new value"));
        assert_eq!(metadata.unknown_data.len(), 1);
    }

    fn setup_conn() -> diesel::SqliteConnection {
        let mut conn = diesel::SqliteConnection::establish(":memory:").unwrap();
        diesel::sql_query("CREATE TABLE users (id TEXT PRIMARY KEY, column_sync_metadata TEXT)")
            .execute(&mut conn)
            .unwrap();
        conn
    }

    fn insert_user(conn: &mut diesel::SqliteConnection, id: &str, metadata: &str) {
        diesel::sql_query(format!(
            "INSERT INTO users (id, column_sync_metadata) VALUES ('{id}', '{metadata}')"
        ))
        .execute(conn)
        .unwrap();
    }

    fn stored_metadata(conn: &mut diesel::SqliteConnection, id: &str) -> String {
        #[derive(diesel::QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            column_sync_metadata: String,
        }
        let row: Row = diesel::sql_query("SELECT column_sync_metadata FROM users WHERE id = ?")
            .bind::<diesel::sql_types::Text, _>(id)
            .get_result(conn)
            .unwrap();
        row.column_sync_metadata
    }

    #[test]
    fn test_clears_staged_data_but_keeps_rest_of_metadata() {
        let mut conn = setup_conn();
        insert_user(
            &mut conn,
            "u1",
            r#"{"name": {"dirty_at": "2025-01-01T00:00:00Z"}, ".unknown_data": {"future_column": "staged"}}"#,
        );

        clear_table_unknown_data(&mut conn, "users", "column_sync_metadata").unwrap();

        let metadata: serde_json::Value =
            serde_json::from_str(&stored_metadata(&mut conn, "u1")).unwrap();
        assert_eq!(
            metadata,
            json!({"name": {"dirty_at": "2025-01-01T00:00:00Z"}})
        );
    }

    #[test]
    fn test_rows_without_staged_data_untouched() {
        let mut conn = setup_conn();
        insert_user(
            &mut conn,
            "u1",
            r#"{"name": {"dirty_at": "2025-01-01T00:00:00Z"}}"#,
        );
        insert_user(&mut conn, "u2", r#"{}"#);

        clear_table_unknown_data(&mut conn, "users", "column_sync_metadata").unwrap();

        // json_remove rewrites the JSON even when the key is absent, so compare parsed values
        let u1: serde_json::Value =
            serde_json::from_str(&stored_metadata(&mut conn, "u1")).unwrap();
        assert_eq!(u1, json!({"name": {"dirty_at": "2025-01-01T00:00:00Z"}}));
        let u2: serde_json::Value =
            serde_json::from_str(&stored_metadata(&mut conn, "u2")).unwrap();
        assert_eq!(u2, json!({}));
    }
}
