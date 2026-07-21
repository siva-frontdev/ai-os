#![forbid(unsafe_code)]
#![warn(missing_docs)]
mod error;
pub mod renderer;
pub use error::PromptsError;
pub use renderer::DefaultPromptRenderer;
