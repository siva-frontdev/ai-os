#![forbid(unsafe_code)]

pub mod errors;
pub mod types;
pub mod workflow_builder;
pub mod workflow_executor;

pub use errors::{WorkflowError, WorkflowResult};
pub use types::{
    StepStatus, Workflow, WorkflowProgress, WorkflowStage, WorkflowStatus, WorkflowStep,
};
pub use workflow_builder::{StepConfig, WorkflowBuilder};
pub use workflow_executor::WorkflowExecutor;

#[cfg(test)]
mod tests;
