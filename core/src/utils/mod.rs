use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Returns the current UTC timestamp.
pub fn now() -> DateTime<Utc> {
    Utc::now()
}

/// Generates a unique identifier string.
pub fn ulid() -> String {
    Uuid::new_v4().to_string()
}

/// A thread-safe identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(Uuid);

impl Id {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl Default for Id {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Extension trait for `Result` to map errors into `CoreError`.
pub trait MapCoreError<T> {
    fn core_err(self, msg: &str) -> Result<T, crate::error::CoreError>;
}

impl<T, E: std::fmt::Display> MapCoreError<T> for Result<T, E> {
    fn core_err(self, msg: &str) -> Result<T, crate::error::CoreError> {
        self.map_err(|e| crate::error::CoreError::General(format!("{}: {}", msg, e)))
    }
}
