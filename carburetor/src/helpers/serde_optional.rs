//! Serde deserializers for columns with SQL defaults.
//!
//! On the backend, `#[default(sql)]` columns are padded with `Option` in
//! `UploadInsert` models so old clients that omit them deserialize to `None`
//! (letting the DB apply its default).

use serde::{Deserialize, Deserializer};

/// Deserializer for non-nullable sql-default columns (`Option<T>`).
///
/// - Missing key → `None` (via `#[serde(default)]`) → DB default.
/// - `null` → **error** (non-nullable column must not receive null).
/// - value → `Some(value)`.
pub mod strict_optional {
    use super::*;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Deserialize::deserialize(deserializer).map(Some)
    }
}

/// Deserializer for nullable sql-default columns (`Option<Option<T>>`).
///
/// - Missing key → `None` (via `#[serde(default)]`) → DB default.
/// - `null` → `Some(None)` → insert explicit NULL.
/// - value → `Some(Some(value))` → insert value.
pub mod double_optional {
    use super::*;

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Deserialize::deserialize(deserializer).map(Some)
    }
}
