#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub mod router;

pub use error::RouterError;
pub use intelligence_core::{RoutingDecision, RoutingPolicy};
pub use router::DefaultRouter;
