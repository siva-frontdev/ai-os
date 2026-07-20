#![forbid(unsafe_code)]

pub mod errors;
pub mod lesson_store;
pub mod reflector;
pub mod types;

pub use errors::{ReflectionError, ReflectionResult};
pub use lesson_store::LessonStore;
pub use reflector::Reflector;
pub use types::{
    ComparisonResult, Improvement, ImprovementCategory, Lesson, Mistake,
    MistakeCategory, MistakeSeverity, Reflection,
};

#[cfg(test)]
mod tests;
