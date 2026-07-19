//! # AI-OS Core
//!
//! The foundational layer of the AI-native operating platform.
//!
//! ## Modules
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`config`]     | Configuration sources (JSON, env, layered) |
//! | [`container`]  | Type-based dependency injection container |
//! | [`logging`]    | Structured logging with levels, fields, sinks |
//! | [`events`]     | In-process pub/sub event bus |
//! | [`lifecycle`]  | Service lifecycle (start, stop, state machine) |
//! | [`registry`]   | Name-based service directory |
//! | [`health`]     | Health checks and monitoring |
//! | [`bootstrap`]  | Builder that wires all subsystems together |
//! | [`application`]| Top-level platform handle |
//! | [`utils`]      | Shared helpers (ID generation, timestamps) |
//!
//! ## Quick start
//!
//! ```ignore
//! use ai_os_core::bootstrap::PlatformBuilder;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let app = PlatformBuilder::new().build().await?;
//!     app.run_until_signal().await?;
//!     Ok(())
//! }
//! ```
pub mod application;
pub mod bootstrap;
pub mod config;
pub mod container;
pub mod error;
pub mod events;
pub mod health;
pub mod lifecycle;
pub mod logging;
pub mod registry;
pub mod utils;

// Re-export the most common types at the crate root for
// convenience.
pub use error::CoreError;
