//! Shared helpers for the plugins.

use ai_os_mcp_server::ToolOutcome;
use serde_json::json;

use crate::provider::ProviderError;

/// The current time as an RFC 3339 string in UTC.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Read a string argument from a `tools/call` argument object.
pub fn str_arg(args: &serde_json::Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(serde_json::Value::as_str)
        .map(String::from)
}

/// Build an error outcome from a provider failure, preserving the structured
/// error payload so the runtime can relay it verbatim to LIFE.
pub fn provider_tool_error(err: &ProviderError) -> ToolOutcome {
    ToolOutcome {
        is_error: true,
        result: err.as_json(),
        observations: vec![],
    }
}

/// Build an error outcome for an invalid tool argument.
pub fn tool_error(message: &str) -> ToolOutcome {
    ToolOutcome {
        is_error: true,
        result: json!({"error": message}),
        observations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn reads_string_argument() {
        let args = json!({"chat_id": "c1", "text": "hello"});
        assert_eq!(str_arg(&args, "chat_id").as_deref(), Some("c1"));
        assert_eq!(str_arg(&args, "missing"), None);
    }

    #[test]
    fn rfc3339_timestamp_is_non_empty() {
        assert!(!now_rfc3339().is_empty());
    }
}
