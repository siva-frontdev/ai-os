use async_trait::async_trait;
use execution_core::{Runner, ToolBinding};
use std::fmt::Debug;

use crate::container::DefaultContainerRunner;
use crate::error::RunnerResult;
use crate::subprocess::SubprocessRunner;
use crate::wasm::DefaultWasmRunner;

#[async_trait]
pub trait RunnerFactory: Debug + Send + Sync {
    async fn create_runner(&self, binding: &ToolBinding) -> RunnerResult<Box<dyn Runner>>;
}

#[derive(Debug, Clone)]
pub struct DefaultRunnerFactory;

#[async_trait]
impl RunnerFactory for DefaultRunnerFactory {
    async fn create_runner(&self, binding: &ToolBinding) -> RunnerResult<Box<dyn Runner>> {
        match binding {
            ToolBinding::Subprocess { .. } => Ok(Box::new(SubprocessRunner::new())),
            ToolBinding::Wasm { .. } => Ok(Box::new(DefaultWasmRunner)),
            ToolBinding::Container { .. } => Ok(Box::new(DefaultContainerRunner)),
        }
    }
}
