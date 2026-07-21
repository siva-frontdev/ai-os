#![forbid(unsafe_code)]
#![warn(missing_docs)]
mod error;
pub mod generator;
pub use error::EmbeddingsError;
pub use generator::DefaultEmbeddingManager;
