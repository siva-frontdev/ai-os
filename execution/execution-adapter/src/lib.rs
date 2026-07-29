#![forbid(unsafe_code)]

pub mod error;
pub mod generator;

pub use error::{AdapterError, AdapterResult};
pub use generator::DefaultAdapterGenerator;
