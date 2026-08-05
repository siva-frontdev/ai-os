//! Action translation: AI-OS [`Action`] input → MCP tool arguments.

use serde_json::Value;

/// Build the MCP `tools/call` arguments from an action's structured input.
///
/// Action inputs are already typed JSON validated against the capability's
/// `input_schema`, so the default mapping is a straight pass-through. A
/// capability-specific mapping can be layered on top here later — the key
/// property is that no stringly-typed shell fragments are ever built.
pub fn build_tool_arguments(input: &Value) -> Value {
    input.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_structured_input() {
        let input = serde_json::json!({
            "to": "a@b.c",
            "subject": "hello",
            "body": "world"
        });
        assert_eq!(build_tool_arguments(&input), input);
    }

    #[test]
    fn passes_through_arbitrary_objects() {
        let input = serde_json::json!({"any": {"nested": [1, 2, 3]}});
        assert_eq!(build_tool_arguments(&input), input);
    }
}
