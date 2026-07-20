//! `GoalStatus` and `GoalPriority` shared enums.
use serde::{Deserialize, Serialize};

/// Goal lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GoalStatus {
    Pending,
    Active,
    Planning,
    Executing,
    Evaluating,
    Recovering,
    Completed,
    Failed,
    Cancelled,
    Paused,
}

impl GoalStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Active => "Active",
            Self::Planning => "Planning",
            Self::Executing => "Executing",
            Self::Evaluating => "Evaluating",
            Self::Recovering => "Recovering",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Paused => "Paused",
        }
    }
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Goal priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum GoalPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[allow(dead_code)]
impl GoalPriority {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Normal => "Normal",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }
}

impl std::fmt::Display for GoalPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
