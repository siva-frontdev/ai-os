#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod registry;

pub use error::{ModelsError, ModelsResult};
pub use registry::DefaultModelRegistry;
