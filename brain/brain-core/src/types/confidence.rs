//! `Confidence` — calibrated confidence score in \[0.0, 1.0\].
///
/// Used for hypothesis confidence, decision confidence, improvement scores, etc.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Confidence(pub f32);

impl Confidence {
    /// Minimum representable confidence (effectively zero).
    pub const MIN: Self = Self(0.0);
    /// Maximum representable confidence (certainty).
    pub const MAX: Self = Self(1.0);
    /// Default confidence (neutral).
    pub const DEFAULT: Self = Self(0.5);

    /// Create a new Confidence, clamped to \[0.0, 1.0\].
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    /// Raw inner value (may be outside \[0.0, 1.0\] if constructed with `from_raw`).
    pub const fn raw(&self) -> f32 {
        self.0
    }

    /// Create a Confidence without clamping (internal use only).
    pub const fn from_raw(value: f32) -> Self {
        Self(value)
    }
}

impl Default for Confidence {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}
