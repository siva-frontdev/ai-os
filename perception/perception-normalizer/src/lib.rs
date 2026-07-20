#![forbid(unsafe_code)]

//! # perception-normalizer
//!
//! Schema validation, format detection, sanitization, and
//! normalization pipeline for the Perception Platform.
//!
//! This crate implements the [`Normalizer`] trait, format
//! detection ([`FormatDetector`]), schema management
//! ([`SchemaRegistry`]), and provides default format parsers
//! for JSON, text, and binary payloads.

mod normalizer;

pub use normalizer::*;
