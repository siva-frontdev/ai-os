//! GitHub plugin: an in-memory repository with `github.create_issue` and
//! `github.read_repository`. No network is required; the repository state
//! lives in the plugin process.

use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::util::{now_rfc3339, str_arg};

/// Internal repository state.
#[derive(Debug)]
struct GitHubInner {
    name: String,
    description: String,
    default_branch: String,
    issues: Mutex<Vec<Value>>,
}

/// A real GitHub plugin backed by an in-memory repository.
#[derive(Debug)]
pub struct GitHubPlugin {
    inner: Arc<GitHubInner>,
}

impl GitHubPlugin {
    /// Create the plugin for a named repository.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        default_branch: impl Into<String>,
    ) -> Self {
        Self {
            inner: Arc::new(GitHubInner {
                name: name.into(),
                description: description.into(),
                default_branch: default_branch.into(),
                issues: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Number of issues created.
    pub async fn issue_count(&self) -> usize {
        self.inner.issues.lock().await.len()
    }
}

#[async_trait]
impl McpPlugin for GitHubPlugin {
    fn name(&self) -> &'static str {
        "github"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "github.create_issue".into(),
                description: "Open a new issue in the repository".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "body": {"type": "string"}
                    },
                    "required": ["title"]
                }),
            },
            McpTool {
                name: "github.read_repository".into(),
                description: "Read repository metadata and its open issues".into(),
                input_schema: json!({"type": "object", "properties": {}}),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "github.create_issue" => self.create_issue(arguments).await,
            "github.read_repository" => Ok(self.read_repository().await),
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl GitHubPlugin {
    async fn create_issue(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(title) = str_arg(&args, "title") else {
            return Ok(tool_error("github.create_issue requires a 'title'"));
        };
        let body = str_arg(&args, "body").unwrap_or_default();

        let mut issues = self.inner.issues.lock().await;
        let issue_number = issues.len() + 1;
        let issue = json!({
            "number": issue_number,
            "title": title,
            "body": body,
            "state": "open",
            "createdAt": now_rfc3339(),
        });
        issues.push(issue.clone());

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "issueNumber": issue_number,
                "title": title,
                "state": "open",
            }),
            observations: vec![json!({
                "source": "github.create_issue",
                "kind": "status_changed",
                "text": format!("issue #{issue_number} opened: {title}"),
                "issue": issue,
            })],
        })
    }

    async fn read_repository(&self) -> ToolOutcome {
        let issues = self.inner.issues.lock().await.len();
        ToolOutcome {
            is_error: false,
            result: json!({
                "name": self.inner.name,
                "description": self.inner.description,
                "defaultBranch": self.inner.default_branch,
                "openIssues": issues,
            }),
            observations: vec![],
        }
    }
}

fn tool_error(message: &str) -> ToolOutcome {
    ToolOutcome {
        is_error: true,
        result: json!({"error": message}),
        observations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plugin() -> GitHubPlugin {
        GitHubPlugin::new("ai-os", "AI-native OS", "main")
    }

    #[tokio::test]
    async fn create_issue_increments_number_and_emits_observation() {
        let plugin = plugin();
        let first = plugin
            .call_tool("github.create_issue", json!({"title": "bug one"}))
            .await
            .unwrap();
        assert!(!first.is_error);
        assert_eq!(first.result["issueNumber"], 1);
        assert_eq!(first.observations[0]["source"], "github.create_issue");
        assert_eq!(first.observations[0]["kind"], "status_changed");

        let second = plugin
            .call_tool(
                "github.create_issue",
                json!({"title": "bug two", "body": "details"}),
            )
            .await
            .unwrap();
        assert_eq!(second.result["issueNumber"], 2);
        assert_eq!(plugin.issue_count().await, 2);
    }

    #[tokio::test]
    async fn read_repository_returns_metadata() {
        let plugin = plugin();
        let outcome = plugin
            .call_tool("github.read_repository", json!({}))
            .await
            .unwrap();
        assert!(!outcome.is_error);
        assert_eq!(outcome.result["name"], "ai-os");
        assert_eq!(outcome.result["defaultBranch"], "main");
    }

    #[tokio::test]
    async fn create_issue_requires_title() {
        let plugin = plugin();
        let outcome = plugin
            .call_tool("github.create_issue", json!({}))
            .await
            .unwrap();
        assert!(outcome.is_error);
    }
}
