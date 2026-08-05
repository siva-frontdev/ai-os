//! Minimal JSON Schema validator.
//!
//! Validates action inputs at the AI-OS boundary against a capability's
//! declared `input_schema`. Supports the subset of JSON Schema used by
//! runtime capabilities:
//!
//! - root `type`: `object`, `string`, `number`, `integer`, `boolean`,
//!   `array`, `null`
//! - `properties`, `required`, `additionalProperties` (objects)
//! - `items` (arrays)
//! - `enum` (strings and scalars)
//!
//! Unknown keywords are ignored, so full schemas from external runtimes
//! never cause a false rejection. A missing `type` accepts any value.

use serde_json::Value;

/// Validate `value` against `schema`.
///
/// Returns `Ok(())` when valid, or a human-readable reason.
pub fn validate(value: &Value, schema: &Value) -> Result<(), String> {
    let Some(type_name) = schema.get("type").and_then(Value::as_str) else {
        return validate_required_only(value, schema);
    };

    match type_name {
        "object" => validate_object(value, schema),
        "string" => validate_string(value, schema),
        "number" => {
            if value.is_number() {
                Ok(())
            } else {
                Err(format!("expected number, got {}", type_of(value)))
            }
        }
        "integer" => {
            if value.is_i64() || value.is_u64() {
                Ok(())
            } else if let Some(n) = value.as_f64() {
                if n.fract() == 0.0 {
                    Ok(())
                } else {
                    Err("expected integer".into())
                }
            } else {
                Err(format!("expected integer, got {}", type_of(value)))
            }
        }
        "boolean" => {
            if value.is_boolean() {
                Ok(())
            } else {
                Err(format!("expected boolean, got {}", type_of(value)))
            }
        }
        "array" => validate_array(value, schema),
        "null" => {
            if value.is_null() {
                Ok(())
            } else {
                Err("expected null".into())
            }
        }
        // Unknown or "any" type: accept.
        _ => Ok(()),
    }
}

fn validate_required_only(value: &Value, schema: &Value) -> Result<(), String> {
    if value.is_object() {
        if let Some(required) = schema.get("required").and_then(Value::as_array) {
            for name in required {
                let Some(name) = name.as_str() else { continue };
                if value.get(name).is_none() {
                    return Err(format!("missing required property '{name}'"));
                }
            }
        }
    }
    Ok(())
}

fn validate_object(value: &Value, schema: &Value) -> Result<(), String> {
    let Some(obj) = value.as_object() else {
        return Err(format!("expected object, got {}", type_of(value)));
    };

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for name in required {
            let Some(name) = name.as_str() else { continue };
            if !obj.contains_key(name) {
                return Err(format!("missing required property '{name}'"));
            }
        }
    }

    if let Some(props) = schema.get("properties").and_then(Value::as_object) {
        for (name, prop_schema) in props {
            if let Some(v) = obj.get(name) {
                validate(v, prop_schema)?;
            }
        }
    }

    if let Some(Value::Bool(false)) = schema.get("additionalProperties") {
        if let Some(props) = schema.get("properties").and_then(Value::as_object) {
            for key in obj.keys() {
                if !props.contains_key(key) {
                    return Err(format!("unexpected property '{key}'"));
                }
            }
        }
    }

    Ok(())
}

fn validate_string(value: &Value, schema: &Value) -> Result<(), String> {
    let Some(s) = value.as_str() else {
        return Err(format!("expected string, got {}", type_of(value)));
    };
    if let Some(Value::Array(enum_values)) = schema.get("enum") {
        if !enum_values.iter().any(|v| v.as_str() == Some(s)) {
            return Err(format!(
                "'{s}' is not one of the allowed values {}",
                enum_values
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    Ok(())
}

fn validate_array(value: &Value, schema: &Value) -> Result<(), String> {
    let Some(arr) = value.as_array() else {
        return Err(format!("expected array, got {}", type_of(value)));
    };
    if let Some(items) = schema.get("items") {
        if !items.is_null() {
            for item in arr {
                validate(item, items)?;
            }
        }
    }
    Ok(())
}

fn type_of(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(value: serde_json::Value) -> serde_json::Value {
        value
    }

    #[test]
    fn accepts_valid_email_send_input() {
        let s = schema(serde_json::json!({
            "type": "object",
            "properties": {
                "to": {"type": "string"},
                "subject": {"type": "string"},
                "body": {"type": "string"}
            },
            "required": ["to"]
        }));
        let input = serde_json::json!({"to": "a@b.c", "subject": "hi", "body": "hello"});
        assert!(validate(&input, &s).is_ok());
    }

    #[test]
    fn rejects_missing_required() {
        let s = schema(serde_json::json!({
            "type": "object",
            "required": ["to"]
        }));
        let input = serde_json::json!({"subject": "hi"});
        assert!(validate(&input, &s).is_err());
    }

    #[test]
    fn rejects_type_mismatch() {
        let s = schema(serde_json::json!({"type": "string"}));
        assert!(validate(&serde_json::json!(42), &s).is_err());
        assert!(validate(&serde_json::json!("ok"), &s).is_ok());
    }

    #[test]
    fn rejects_additional_properties_when_disallowed() {
        let s = schema(serde_json::json!({
            "type": "object",
            "properties": {"to": {"type": "string"}},
            "additionalProperties": false
        }));
        assert!(validate(&serde_json::json!({"to": "a", "bogus": 1}), &s).is_err());
        assert!(validate(&serde_json::json!({"to": "a"}), &s).is_ok());
    }

    #[test]
    fn accepts_any_value_when_no_type_declared() {
        let s = schema(serde_json::json!({}));
        assert!(validate(&serde_json::json!("anything"), &s).is_ok());
        assert!(validate(&serde_json::json!({"a": [1]}), &s).is_ok());
    }

    #[test]
    fn supports_enum() {
        let s = schema(serde_json::json!({"type": "string", "enum": ["a", "b"]}));
        assert!(validate(&serde_json::json!("a"), &s).is_ok());
        assert!(validate(&serde_json::json!("c"), &s).is_err());
    }

    #[test]
    fn supports_arrays_with_items() {
        let s = schema(serde_json::json!({"type": "array", "items": {"type": "string"}}));
        assert!(validate(&serde_json::json!(["x", "y"]), &s).is_ok());
        assert!(validate(&serde_json::json!(["x", 1]), &s).is_err());
    }
}
