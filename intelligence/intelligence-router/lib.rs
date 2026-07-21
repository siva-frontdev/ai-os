#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Model router with policy-based scoring and fallback selection.

pub mod error;
pub mod router;

pub use error::RouterError;
pub use router::DefaultRouter;
