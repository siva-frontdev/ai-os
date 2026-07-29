//! Linux implementation of all OSAL subsystem traits.
//!
//! # Safety
//! This crate contains `unsafe` blocks for FFI with libc.
//! All unsafe code is documented with `// SAFETY:` comments.
//! No other crate in the platform is allowed to contain unsafe code.

pub mod desktop;
pub mod devices;
pub mod filesystem;
pub mod monitoring;
pub mod network;
pub mod platform;
pub mod process;
pub mod terminal;
pub mod users;

mod facade;

pub use facade::LinuxKernelFacade;
