//! Locate the real `ai-os-mcp-server` binary for subprocess tests.

use std::path::PathBuf;

/// Locate the `ai-os-mcp-server` binary built from the `ai-os-plugins`
/// package.
///
/// Resolution order:
/// 1. `AI_OS_MCP_SERVER_BIN` environment variable (explicit override);
/// 2. the workspace `target/debug/ai-os-mcp-server`;
/// 3. the workspace `target/release/ai-os-mcp-server`.
pub fn mcp_server_bin() -> PathBuf {
    if let Ok(path) = std::env::var("AI_OS_MCP_SERVER_BIN") {
        if !path.is_empty() {
            let explicit = PathBuf::from(path);
            if explicit.exists() {
                return explicit;
            }
        }
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    for profile in ["debug", "release"] {
        let candidate = workspace_target.join(profile).join("ai-os-mcp-server");
        if candidate.exists() {
            return candidate;
        }
    }

    panic!(
        "ai-os-mcp-server binary not found; build it with \
         `cargo build -p ai-os-plugins --bin ai-os-mcp-server` \
         (or set AI_OS_MCP_SERVER_BIN)"
    );
}
