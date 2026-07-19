#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! OSAL filesystem subsystem — data types, events, and utilities.
//!
//! The `FileSystem` trait and `FilesystemError` are defined in
//! [`osal_core`]; this crate provides richer subsystem-specific types
//! ([`FileMetadata`], [`DirEntry`]), events ([`FileEvent`]), and
//! utility types ([`TempFile`]).

pub mod events;
pub mod types;

pub use events::FileEvent;
pub use types::{FileMetadata, DirEntry, TempFile};

/// Re-export of the core filesystem error type.
pub use osal_core::FilesystemError;

/// Re-export of the core file kind enum.
pub use osal_core::FileKind;

/// Re-export of the core `FileSystem` trait.
pub use osal_core::FileSystem;
