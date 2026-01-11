use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, from_value, to_value};

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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct UnknownMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
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
    #[serde(flatten)]
    pub unknown_data: HashMap<String, UnknownMetadata>,
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
    fn test_empty_unknown_data() {
        let value: Value = json!({"unknown": {}});
        let metadata: ClientSyncMetadata<User> = value.clone().into();

        assert_eq!(value, Value::from(metadata));
    }
}
