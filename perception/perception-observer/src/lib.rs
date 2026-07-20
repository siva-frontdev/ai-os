#![forbid(unsafe_code)]

//! # perception-observer
//!
//! Observation providers and collectors for the Perception Platform.
//!
//! This crate implements the [`Observer`] trait and provides default
//! observer implementations for filesystem, process, terminal, and
//! network sources. Each observer subscribes to OSAL events or reads
//! directly from I/O sources and produces [`Observation`] instances.

mod observer;

pub use observer::*;
