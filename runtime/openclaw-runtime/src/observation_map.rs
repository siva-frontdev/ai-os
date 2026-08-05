//! Observation translation: MCP observation payloads → AI-OS
//! [`Observation`]s.

use ai_os_runtime_api::{now_ms, CapabilityId, Observation, ObservationKind};
use serde_json::Value;

/// Translate a `notifications/observation` payload into an AI-OS
/// [`Observation`].
///
/// The payload may carry `source`, `kind`, `channel_id`, `sender`, and
/// `trace_id` hints; defaults are applied for anything missing. The full
/// payload is preserved so the Cognitive Loop (World Understanding) has
/// every detail the runtime provided.
pub fn observation_from_mcp(payload: Value) -> Observation {
    let source = payload
        .get("source")
        .and_then(Value::as_str)
        .map(|s| CapabilityId::new(s.to_string()))
        .unwrap_or_else(|| CapabilityId::new("runtime.inbound"));

    let kind = match payload
        .get("kind")
        .and_then(Value::as_str)
        .map(|k| k.to_ascii_lowercase())
        .as_deref()
    {
        Some("inbound_message") => ObservationKind::InboundMessage,
        Some("status_changed") => ObservationKind::StatusChanged,
        Some("schedule_trigger") => ObservationKind::ScheduleTrigger,
        Some("health") => ObservationKind::Health,
        _ => ObservationKind::DeviceEvent,
    };

    Observation {
        source,
        kind,
        channel_id: payload
            .get("channel_id")
            .and_then(Value::as_str)
            .map(String::from),
        sender: payload
            .get("sender")
            .and_then(Value::as_str)
            .map(String::from),
        timestamp_ms: payload
            .get("timestamp_ms")
            .and_then(Value::as_u64)
            .unwrap_or_else(now_ms),
        trace_id: payload
            .get("trace_id")
            .and_then(Value::as_str)
            .map(String::from),
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_inbound_message() {
        let obs = observation_from_mcp(serde_json::json!({
            "source": "telegram.receive",
            "kind": "inbound_message",
            "channel_id": "-100123",
            "sender": "u1",
            "text": "hello"
        }));
        assert_eq!(obs.source, CapabilityId::new("telegram.receive"));
        assert_eq!(obs.kind, ObservationKind::InboundMessage);
        assert_eq!(obs.channel_id.as_deref(), Some("-100123"));
        assert_eq!(obs.sender.as_deref(), Some("u1"));
        assert_eq!(obs.payload["text"], "hello");
    }

    #[test]
    fn applies_defaults_when_fields_missing() {
        let obs = observation_from_mcp(serde_json::json!({"text": "hi"}));
        assert_eq!(obs.source, CapabilityId::new("runtime.inbound"));
        assert_eq!(obs.kind, ObservationKind::DeviceEvent);
    }

    #[test]
    fn preserves_unknown_kinds() {
        let obs = observation_from_mcp(serde_json::json!({"kind": "custom_thing"}));
        assert_eq!(obs.kind, ObservationKind::DeviceEvent);
    }
}
