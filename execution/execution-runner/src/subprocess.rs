use async_trait::async_trait;
use execution_core::{
    ExecutionError, ExecutionId, ExecutionMetrics, ExecutionPlan, ExecutionResult, Runner,
    RunnerOutput, ToolBinding,
};
use std::collections::HashMap;
use std::os::unix::process::ExitStatusExt;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct SubprocessRunner {
    children: Arc<Mutex<HashMap<ExecutionId, u32>>>,
}

impl SubprocessRunner {
    pub fn new() -> Self {
        Self {
            children: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn kill_process(pid: u32) {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .output();
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = std::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .output();
    }
}

impl Default for SubprocessRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for SubprocessRunner {
    fn clone(&self) -> Self {
        Self {
            children: self.children.clone(),
        }
    }
}

#[async_trait]
impl Runner for SubprocessRunner {
    async fn execute(&self, plan: &ExecutionPlan) -> ExecutionResult<RunnerOutput> {
        let execution_id = plan.id;

        let (binary, args, env, working_dir) = match &plan.binding {
            ToolBinding::Subprocess {
                binary,
                args,
                env,
                working_dir,
                ..
            } => (
                binary.clone(),
                args.clone(),
                env.clone(),
                working_dir.clone(),
            ),
            _ => {
                return Err(ExecutionError::ConfigurationError(
                    "expected Subprocess ToolBinding".into(),
                ));
            }
        };

        let mut cmd = tokio::process::Command::new(&binary);
        cmd.args(&args);
        cmd.envs(&env);
        if let Some(dir) = &working_dir {
            cmd.current_dir(dir);
        }
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| ExecutionError::ProcessSpawnFailed(e.to_string()))?;

        let pid = child
            .id()
            .ok_or_else(|| ExecutionError::ProcessSpawnFailed("failed to get child PID".into()))?;

        self.children
            .lock()
            .map_err(|e| ExecutionError::BackendUnavailable(format!("lock poisoned: {e}")))?
            .insert(execution_id, pid);

        let mut child_stdout = child.stdout.take().ok_or_else(|| {
            ExecutionError::ArtifactCollectionFailed("stdout not available".into())
        })?;
        let mut child_stderr = child.stderr.take().ok_or_else(|| {
            ExecutionError::ArtifactCollectionFailed("stderr not available".into())
        })?;

        let mut stdout_buf = Vec::new();
        let mut stderr_buf = Vec::new();

        let (stdout_res, stderr_res) = tokio::join!(
            tokio::io::AsyncReadExt::read_to_end(&mut child_stdout, &mut stdout_buf),
            tokio::io::AsyncReadExt::read_to_end(&mut child_stderr, &mut stderr_buf),
        );

        stdout_res.map_err(|e| ExecutionError::ArtifactCollectionFailed(e.to_string()))?;
        stderr_res.map_err(|e| ExecutionError::ArtifactCollectionFailed(e.to_string()))?;

        let timeout_dur = std::time::Duration::from_millis(plan.budget.timeout_ms);
        let started = std::time::Instant::now();

        let wait_result = tokio::time::timeout(timeout_dur, child.wait()).await;

        self.children
            .lock()
            .map_err(|e| ExecutionError::BackendUnavailable(format!("lock poisoned: {e}")))?
            .remove(&execution_id);

        let wall_clock_ms = started.elapsed().as_millis() as u64;

        match wait_result {
            Ok(Ok(status)) => match status.code() {
                Some(0) => Ok(RunnerOutput {
                    execution_id,
                    exit_code: 0,
                    stdout: stdout_buf,
                    stderr: stderr_buf,
                    metrics: ExecutionMetrics {
                        wall_clock_ms,
                        ..Default::default()
                    },
                    artifacts: Vec::new(),
                }),
                Some(code) => Err(ExecutionError::InvalidExitCode { exit_code: code }),
                None => {
                    let signal = status
                        .signal()
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    Err(ExecutionError::SignalTerminated { signal })
                }
            },
            Ok(Err(e)) => Err(ExecutionError::ExecutionPanic(e.to_string())),
            Err(_) => {
                Self::kill_process(pid).await;
                Err(ExecutionError::TimeoutExceeded {
                    elapsed_ms: wall_clock_ms,
                })
            }
        }
    }

    async fn cancel(&self, execution_id: &ExecutionId) -> ExecutionResult<()> {
        let pid = self
            .children
            .lock()
            .map_err(|e| ExecutionError::BackendUnavailable(format!("lock poisoned: {e}")))?
            .remove(execution_id)
            .ok_or_else(|| {
                ExecutionError::NotFound(format!("execution {execution_id} not running"))
            })?;

        Self::kill_process(pid).await;
        Ok(())
    }

    async fn health(&self) -> ExecutionResult<()> {
        let status = tokio::process::Command::new("/bin/true")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| ExecutionError::ProcessSpawnFailed(e.to_string()))?
            .wait()
            .await
            .map_err(|e| ExecutionError::ExecutionPanic(e.to_string()))?;

        if !status.success() {
            return Err(ExecutionError::ProcessSpawnFailed(
                "health check /bin/true failed".into(),
            ));
        }
        Ok(())
    }

    fn backend_type(&self) -> &'static str {
        "subprocess"
    }
}
