pub mod config;
pub mod error;
pub mod helpers;
pub mod models;

// Re-export chrono so that user can use Chrono type for model without adding to their dependencies
pub mod chrono {
    use chrono::{DateTime, Utc};

    pub use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    pub type DateTimeUtc = DateTime<Utc>;
}

// Re-export serde_json so that user can use Value type for model without adding to their
// dependencies
pub mod serde_json {
    pub use serde_json::{Value, from_str, from_value};
}

pub use prelude::*;
pub mod prelude {
    pub use carburetor_macro::*;
}
