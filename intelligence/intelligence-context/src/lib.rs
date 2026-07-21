#![forbid(unsafe_code)]
#![warn(missing_docs)]
pub mod assembler;
mod error;
pub use assembler::DefaultContextBuilder;
pub use error::ContextError;
