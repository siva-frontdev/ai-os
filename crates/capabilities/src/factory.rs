use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use execution_core::{
    ExecutionError, ExecutionId, ExecutionPlan, ExecutionResult, Runner, RunnerOutput, ToolBinding,
};
use execution_runner::RunnerFactory;
use execution_runner::RunnerResult;

use crate::browser;
use crate::desktop;
use crate::development;
use crate::fs;
use crate::input;

const CAPABILITY_PREFIX: &str = "@capability/";

#[derive(Debug)]
pub struct CapabilitiesRunnerFactory {
    inner: Arc<dyn RunnerFactory>,
}

impl CapabilitiesRunnerFactory {
    pub fn new(inner: Arc<dyn RunnerFactory>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl RunnerFactory for CapabilitiesRunnerFactory {
    async fn create_runner(&self, binding: &ToolBinding) -> RunnerResult<Box<dyn Runner>> {
        if let ToolBinding::Subprocess { binary, .. } = binding {
            if binary.starts_with(CAPABILITY_PREFIX) {
                return Ok(Box::new(CapabilitiesRunner));
            }
        }
        self.inner.create_runner(binding).await
    }
}

#[derive(Debug)]
pub struct CapabilitiesRunner;

#[async_trait]
impl Runner for CapabilitiesRunner {
    async fn execute(&self, plan: &ExecutionPlan) -> ExecutionResult<RunnerOutput> {
        let capability = &plan.request.capability_id;
        let inputs = &plan.request.inputs;

        let result = if capability.starts_with("development.") {
            development::dispatch(capability, inputs)
        } else if capability.starts_with("browser.") {
            browser::dispatch(capability, inputs)
        } else if capability.starts_with("fs.") {
            fs::dispatch(capability, inputs)
        } else if capability.starts_with("input.") {
            input::dispatch(capability, inputs)
        } else {
            desktop::dispatch(capability, inputs)
        };
        let (stdout, stderr, exit_code) = match result {
            Ok(r) => r,
            Err(e) => {
                return Err(ExecutionError::ConfigurationError(e.to_string()));
            }
        };

        Ok(RunnerOutput {
            execution_id: plan.id,
            exit_code,
            stdout,
            stderr,
            metrics: execution_core::types::ExecutionMetrics::default(),
            artifacts: Vec::new(),
        })
    }

    async fn cancel(&self, _execution_id: &ExecutionId) -> ExecutionResult<()> {
        Ok(())
    }

    async fn health(&self) -> ExecutionResult<()> {
        Ok(())
    }

    fn backend_type(&self) -> &'static str {
        "capabilities"
    }
}

pub fn capability_binding(capability_id: &str) -> ToolBinding {
    ToolBinding::Subprocess {
        binary: format!("{}{}", CAPABILITY_PREFIX, capability_id),
        args: Vec::new(),
        env: HashMap::new(),
        working_dir: None,
        allowed_paths: Vec::new(),
        denied_binaries: Vec::new(),
    }
}
