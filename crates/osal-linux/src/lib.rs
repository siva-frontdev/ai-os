//! Linux implementation of all OSAL subsystem traits.
//!
//! # Safety
//! This crate contains `unsafe` blocks for FFI with libc.
//! All unsafe code is documented with `// SAFETY:` comments.
//! No other crate in the platform is allowed to contain unsafe code.

pub mod filesystem;
pub mod process;
pub mod terminal;
pub mod network;
pub mod monitoring;
pub mod devices;
pub mod users;
pub mod platform;

mod facade;

pub use facade::LinuxKernelFacade;
