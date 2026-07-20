#![forbid(unsafe_code)]

pub mod collector;
pub mod error;
pub mod parser;
pub mod router;

pub use collector::DefaultResultCollector;
pub use error::{ResultsError, ResultsResult};
pub use parser::{DefaultOutputParser, JsonParser, RawParser};
pub use router::DefaultOutputRouter;
