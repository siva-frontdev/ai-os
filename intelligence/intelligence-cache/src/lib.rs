#![forbid(unsafe_code)]
#![warn(missing_docs)]
pub mod cache;
mod error;
pub use cache::{CacheEntry, DefaultResponseCache};
pub use error::CacheError;
