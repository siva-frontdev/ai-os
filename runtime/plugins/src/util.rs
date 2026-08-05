//! Shared helpers for the plugins.

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
