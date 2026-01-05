#[cfg(feature = "backend")]
pub mod backend;

pub mod config;
pub mod error;

// Re-export chrono so that user can use Chrono type for model without adding to their dependencies
pub mod chrono {
    use chrono::{DateTime, Utc};

    pub use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    pub type DateTimeUtc = DateTime<Utc>;
}

pub use prelude::*;
pub mod prelude {
    pub use carburetor_macro::carburetor;
}
