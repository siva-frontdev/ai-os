#![forbid(unsafe_code)]

pub mod error;
pub mod external;
pub mod local;

pub use error::{DiscoveryError, DiscoveryResult};
pub use external::DefaultExternalDiscoverer;
pub use local::DefaultLocalDiscoverer;
