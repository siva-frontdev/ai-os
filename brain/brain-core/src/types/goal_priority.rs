//! `GoalPriority` enum.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GoalPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

impl GoalPriority {
    pub const fn as_str(&self) -> &'static str {
        match self { Self::Low => "Low", Self::Normal => "Normal", Self::High => "High", Self::Critical => "Critical" }
    }
}

impl std::fmt::Display for GoalPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { f.write_str(self.as_str()) }
}
