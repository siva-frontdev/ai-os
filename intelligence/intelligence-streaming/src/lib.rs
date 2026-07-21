#![forbid(unsafe_code)]
#![warn(missing_docs)]
mod decoder;
mod error;
pub use decoder::DefaultStreamingManager;
pub use error::StreamingError;
