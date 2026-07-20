use async_trait::async_trait;
use execution_core::{
    types::ExecutionResult as ExecutionResultStruct, ExecutionArtifact, ExecutionError,
    ExecutionId, ExecutionState, ResultCollector, RunnerOutput,
};
use memory_core::Timestamp;
use std::collections::HashMap;
use std::fmt;
use std::sync::RwLock;

pub struct DefaultResultCollector {
    parsers: Vec<Box<dyn execution_core::OutputParser>>,
    max_output_bytes: u64,
    artifacts: RwLock<HashMap<ExecutionId, Vec<ExecutionArtifact>>>,
}

impl fmt::Debug for DefaultResultCollector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultResultCollector")
            .field("parsers_count", &self.parsers.len())
            .field("max_output_bytes", &self.max_output_bytes)
            .finish()
    }
}

impl DefaultResultCollector {
    pub fn new(parsers: Vec<Box<dyn execution_core::OutputParser>>, max_output_bytes: u64) -> Self {
        Self {
            parsers,
            max_output_bytes,
            artifacts: RwLock::new(HashMap::new()),
        }
    }

    fn truncate_output(&self, data: &[u8]) -> Vec<u8> {
        if data.len() as u64 > self.max_output_bytes {
            data[..self.max_output_bytes as usize].to_vec()
        } else {
            data.to_vec()
        }
    }

    async fn try_parse(&self, stdout: &[u8]) -> Option<serde_json::Value> {
        for parser in &self.parsers {
            if let Ok(Some(value)) = parser.parse(stdout).await {
                return Some(value);
            }
        }
        None
    }

    async fn try_parse_with(
        &self,
        stdout: &[u8],
        parser: &dyn execution_core::OutputParser,
    ) -> Option<serde_json::Value> {
        parser.parse(stdout).await.ok().flatten()
    }
}

#[async_trait]
impl ResultCollector for DefaultResultCollector {
    async fn collect(&self, output: RunnerOutput) -> Result<ExecutionResultStruct, ExecutionError> {
        let stdout = self.truncate_output(&output.stdout);
        let stderr = self.truncate_output(&output.stderr);

        let parsed_output = self.try_parse(&stdout).await;

        let state = if output.exit_code == 0 {
            ExecutionState::Collecting
        } else {
            ExecutionState::Failed
        };

        let now = Timestamp::now();

        Ok(ExecutionResultStruct {
            execution_id: output.execution_id,
            state,
            exit_code: Some(output.exit_code),
            stdout,
            stderr,
            parsed_output,
            artifacts: output.artifacts,
            metrics: output.metrics,
            error: None,
            routing_results: Vec::new(),
            started_at: now,
            completed_at: now,
        })
    }

    async fn collect_with_parse(
        &self,
        output: RunnerOutput,
        parser: &dyn execution_core::OutputParser,
    ) -> Result<ExecutionResultStruct, ExecutionError> {
        let stdout = self.truncate_output(&output.stdout);
        let stderr = self.truncate_output(&output.stderr);

        let parsed_output = self.try_parse_with(&stdout, parser).await;

        let state = if output.exit_code == 0 {
            ExecutionState::Collecting
        } else {
            ExecutionState::Failed
        };

        let now = Timestamp::now();

        Ok(ExecutionResultStruct {
            execution_id: output.execution_id,
            state,
            exit_code: Some(output.exit_code),
            stdout,
            stderr,
            parsed_output,
            artifacts: output.artifacts,
            metrics: output.metrics,
            error: None,
            routing_results: Vec::new(),
            started_at: now,
            completed_at: now,
        })
    }

    async fn store_artifact(
        &self,
        execution_id: &ExecutionId,
        artifact: ExecutionArtifact,
    ) -> Result<(), ExecutionError> {
        let mut store = self
            .artifacts
            .write()
            .map_err(|_| ExecutionError::ConfigurationError("lock poisoned".into()))?;
        store.entry(*execution_id).or_default().push(artifact);
        Ok(())
    }
}

impl DefaultResultCollector {
    pub fn get_artifacts(
        &self,
        execution_id: &ExecutionId,
    ) -> crate::error::ResultsResult<Vec<ExecutionArtifact>> {
        let store = self
            .artifacts
            .read()
            .map_err(|_| crate::error::ResultsError::LockPoisoned)?;
        Ok(store.get(execution_id).cloned().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::RawParser;
    use execution_core::{ExecutionMetrics, OutputParser};

    fn make_output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> RunnerOutput {
        RunnerOutput {
            execution_id: ExecutionId::new(),
            exit_code,
            stdout: stdout.to_vec(),
            stderr: stderr.to_vec(),
            metrics: ExecutionMetrics::default(),
            artifacts: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_collect_success() {
        let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(RawParser)];
        let collector = DefaultResultCollector::new(parsers, 1024 * 1024);
        let output = make_output(0, b"hello world", b"");

        let result = collector.collect(output).await.unwrap();
        assert_eq!(result.state, ExecutionState::Collecting);
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, b"hello world");
    }

    #[tokio::test]
    async fn test_collect_failure() {
        let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(RawParser)];
        let collector = DefaultResultCollector::new(parsers, 1024 * 1024);
        let output = make_output(1, b"", b"error occurred");

        let result = collector.collect(output).await.unwrap();
        assert_eq!(result.state, ExecutionState::Failed);
        assert_eq!(result.exit_code, Some(1));
        assert_eq!(result.stderr, b"error occurred");
    }

    #[tokio::test]
    async fn test_truncate_output() {
        let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
        let collector = DefaultResultCollector::new(parsers, 10);
        let large = b"this is more than ten bytes";
        let output = make_output(0, large, b"");

        let result = collector.collect(output).await.unwrap();
        assert_eq!(result.stdout.len(), 10);
        assert_eq!(&result.stdout[..], b"this is mo");
    }

    #[tokio::test]
    async fn test_store_and_get_artifact() {
        let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
        let collector = DefaultResultCollector::new(parsers, 1024);
        let id = ExecutionId::new();

        let artifact = ExecutionArtifact {
            name: "test.json".into(),
            mime_type: "application/json".into(),
            size_bytes: 4,
            data: b"data".to_vec(),
        };

        collector.store_artifact(&id, artifact).await.unwrap();
        let artifacts = collector.get_artifacts(&id).unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name, "test.json");
    }

    #[tokio::test]
    async fn test_collect_with_parse() {
        let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
        let collector = DefaultResultCollector::new(parsers, 1024);
        let output = make_output(0, b"{\"key\": 42}", b"");

        let result = collector
            .collect_with_parse(output, &crate::parser::JsonParser)
            .await
            .unwrap();

        assert_eq!(result.state, ExecutionState::Collecting);
        let parsed = result.parsed_output.unwrap();
        assert_eq!(parsed["key"], 42);
    }

    #[tokio::test]
    async fn test_empty_output_raw_parsed() {
        let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(RawParser)];
        let collector = DefaultResultCollector::new(parsers, 1024);
        let output = make_output(0, b"", b"");

        let result = collector.collect(output).await.unwrap();
        assert!(result.parsed_output.is_some());
        assert_eq!(
            result.parsed_output.unwrap(),
            serde_json::Value::String(String::new())
        );
    }
}
