//! Filesystem plugin: `filesystem.read`, `filesystem.write`,
//! `filesystem.search`. All paths are resolved and canonicalized against a
//! plugin root directory; any path that escapes the root is rejected.

use std::path::{Path, PathBuf};

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::util::str_arg;

/// A filesystem plugin rooted at a single directory.
#[derive(Debug)]
pub struct FileSystemPlugin {
    root: PathBuf,
}

impl FileSystemPlugin {
    /// Create the plugin, canonicalizing `root`. The root must exist.
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, McpServerError> {
        let root = std::fs::canonicalize(root.into())
            .map_err(|e| McpServerError::Plugin(format!("filesystem root invalid: {e}")))?;
        if !root.is_dir() {
            return Err(McpServerError::Plugin(format!(
                "filesystem root is not a directory: {}",
                root.display()
            )));
        }
        Ok(Self { root })
    }

    /// Resolve a user path to an absolute path inside the root.
    ///
    /// Relative paths join the root; absolute paths are used as-is. `..`
    /// components are rejected outright. The deepest existing ancestor is
    /// canonicalized and must stay inside the root, so symlinks cannot
    /// smuggle a path outside it. Missing trailing components (a not-yet-
    /// created file's parent directories) are appended after the check.
    fn resolve(&self, raw: &str) -> Result<PathBuf, String> {
        let base = Path::new(raw);
        for component in base.components() {
            if matches!(component, std::path::Component::ParentDir) {
                return Err(format!("path escapes plugin root: {raw}"));
            }
        }
        let candidate = if base.is_absolute() {
            base.to_path_buf()
        } else {
            self.root.join(base)
        };

        let mut missing = Vec::new();
        let mut probe = candidate.as_path();
        loop {
            if probe.exists() {
                let canonical = probe
                    .canonicalize()
                    .map_err(|e| format!("failed to resolve '{}': {e}", probe.display()))?;
                if !canonical.starts_with(&self.root) {
                    return Err(format!("path escapes plugin root: {raw}"));
                }
                let joined = missing
                    .into_iter()
                    .rev()
                    .fold(canonical, |acc, name| acc.join(name));
                return Ok(joined);
            }
            match probe.file_name() {
                Some(name) => {
                    missing.push(name.to_os_string());
                    probe = probe
                        .parent()
                        .ok_or_else(|| format!("path escapes plugin root: {raw}"))?;
                }
                None => return Err(format!("path escapes plugin root: {raw}")),
            }
        }
    }

    /// Walk the root collecting file paths whose name contains `query`.
    async fn collect_matches(&self, query: &str) -> Vec<String> {
        let mut results = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries = match tokio::fs::read_dir(&dir).await {
                Ok(entries) => entries,
                Err(_) => continue,
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    stack.push(path);
                } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.contains(query) {
                        if let Ok(relative) = path.strip_prefix(&self.root) {
                            results.push(relative.display().to_string());
                        }
                    }
                }
            }
        }
        results.sort();
        results
    }
}

#[async_trait]
impl McpPlugin for FileSystemPlugin {
    fn name(&self) -> &'static str {
        "filesystem"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "filesystem.read".into(),
                description: "Read a file's text content (relative to the plugin root)".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"path": {"type": "string"}},
                    "required": ["path"]
                }),
            },
            McpTool {
                name: "filesystem.write".into(),
                description: "Write text content to a file (relative to the plugin root)".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"},
                        "content": {"type": "string"}
                    },
                    "required": ["path", "content"]
                }),
            },
            McpTool {
                name: "filesystem.search".into(),
                description: "Find files under the root whose name contains a query".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"query": {"type": "string"}},
                    "required": ["query"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "filesystem.read" => self.read(arguments).await,
            "filesystem.write" => self.write(arguments).await,
            "filesystem.search" => self.search(arguments).await,
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl FileSystemPlugin {
    async fn read(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(path) = str_arg(&args, "path") else {
            return Ok(tool_error("filesystem.read requires a 'path'"));
        };
        let resolved = match self.resolve(&path) {
            Ok(p) => p,
            Err(reason) => return Ok(tool_error(&reason)),
        };
        match tokio::fs::read_to_string(&resolved).await {
            Ok(content) => {
                let bytes = content.len();
                Ok(ToolOutcome {
                    is_error: false,
                    result: json!({
                        "path": path,
                        "content": content,
                        "bytes": bytes,
                    }),
                    observations: vec![],
                })
            }
            Err(e) => Ok(tool_error(&format!(
                "failed to read '{}': {e}",
                resolved.display()
            ))),
        }
    }

    async fn write(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(path) = str_arg(&args, "path") else {
            return Ok(tool_error("filesystem.write requires a 'path'"));
        };
        let Some(content) = str_arg(&args, "content") else {
            return Ok(tool_error("filesystem.write requires a 'content'"));
        };
        let resolved = match self.resolve(&path) {
            Ok(p) => p,
            Err(reason) => return Ok(tool_error(&reason)),
        };
        if let Some(parent) = resolved.parent() {
            if let Err(e) = tokio::fs::create_dir_all(parent).await {
                return Ok(tool_error(&format!(
                    "failed to create parent for '{}': {e}",
                    resolved.display()
                )));
            }
        }
        match tokio::fs::write(&resolved, content.as_bytes()).await {
            Ok(()) => Ok(ToolOutcome {
                is_error: false,
                result: json!({
                    "path": path,
                    "bytesWritten": content.len(),
                }),
                observations: vec![],
            }),
            Err(e) => Ok(tool_error(&format!(
                "failed to write '{}': {e}",
                resolved.display()
            ))),
        }
    }

    async fn search(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(query) = str_arg(&args, "query") else {
            return Ok(tool_error("filesystem.search requires a 'query'"));
        };
        let matches = self.collect_matches(&query).await;
        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "query": query,
                "matches": matches,
            }),
            observations: vec![],
        })
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

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ai-os-plugins-fs-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn write_then_read_round_trips() {
        let root = temp_root("rw");
        let plugin = FileSystemPlugin::new(&root).unwrap();

        let write = plugin
            .call_tool(
                "filesystem.write",
                json!({"path": "notes/a.txt", "content": "hello"}),
            )
            .await
            .unwrap();
        assert!(!write.is_error);
        assert_eq!(write.result["bytesWritten"], 5);

        let read = plugin
            .call_tool("filesystem.read", json!({"path": "notes/a.txt"}))
            .await
            .unwrap();
        assert!(!read.is_error);
        assert_eq!(read.result["content"], "hello");
        assert_eq!(read.result["bytes"], 5);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn search_finds_matching_files_recursively() {
        let root = temp_root("search");
        let plugin = FileSystemPlugin::new(&root).unwrap();
        plugin
            .call_tool(
                "filesystem.write",
                json!({"path": "x/report.md", "content": "# r"}),
            )
            .await
            .unwrap();
        plugin
            .call_tool(
                "filesystem.write",
                json!({"path": "x/other.txt", "content": "o"}),
            )
            .await
            .unwrap();

        let search = plugin
            .call_tool("filesystem.search", json!({"query": "report"}))
            .await
            .unwrap();
        assert!(!search.is_error);
        assert_eq!(search.result["matches"], json!(["x/report.md"]));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn rejects_path_escaping_the_root() {
        let root = temp_root("escape");
        let plugin = FileSystemPlugin::new(&root).unwrap();
        let read = plugin
            .call_tool("filesystem.read", json!({"path": "../outside.txt"}))
            .await
            .unwrap();
        assert!(read.is_error);
        assert!(read.result["error"].as_str().unwrap().contains("escapes"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn read_missing_file_reports_tool_error() {
        let root = temp_root("missing");
        let plugin = FileSystemPlugin::new(&root).unwrap();
        let read = plugin
            .call_tool("filesystem.read", json!({"path": "nope.txt"}))
            .await
            .unwrap();
        assert!(read.is_error);
        let _ = std::fs::remove_dir_all(&root);
    }
}
