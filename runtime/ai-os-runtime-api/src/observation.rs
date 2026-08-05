//! Observations: the inbound half of the Runtime API.

use serde::{Deserialize, Serialize};

use crate::capability::CapabilityId;
use crate::time::now_ms;

/// An observation the runtime detected: an inbound message, a device
/// event, a status change, or a schedule trigger.
///
/// Observations enter AI-OS through `Runtime::observe()` /
/// `Runtime::subscribe()` and are fed to the Cognitive Loop (World
/// Understanding) exactly like channel-facing inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// Which connector/capability observed it (e.g. `telegram.receive`).
    pub source: CapabilityId,
    /// Classification of the observation.
    pub kind: ObservationKind,
    /// Channel or session id (e.g. a Telegram chat id).
    pub channel_id: Option<String>,
    /// Sender identity, if known.
    pub sender: Option<String>,
    /// When the observation happened (ms since epoch).
    pub timestamp_ms: u64,
    /// Structured payload: message text, event data, ...
    pub payload: serde_json::Value,
    /// Optional trace id.
    pub trace_id: Option<String>,
}

impl Observation {
    /// Convenience constructor with sensible defaults.
    pub fn new(
        source: impl Into<CapabilityId>,
        kind: ObservationKind,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            source: source.into(),
            kind,
            channel_id: None,
            sender: None,
            timestamp_ms: now_ms(),
            payload,
            trace_id: None,
        }
    }
}

/// Classification of an observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationKind {
    /// An inbound message from a person or channel.
    InboundMessage,
    /// A status change in an external system.
    StatusChanged,
    /// A device event.
    DeviceEvent,
    /// A scheduled trigger fired.
    ScheduleTrigger,
    /// A health probe result.
    Health,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observation_round_trips_through_serde() {
        let obs = Observation::new(
            "telegram.receive",
            ObservationKind::InboundMessage,
            serde_json::json!({"text": "hello"}),
        );
        let json = serde_json::to_value(&obs).unwrap();
        assert_eq!(json["source"], "telegram.receive");
        assert_eq!(json["kind"], "InboundMessage");
        let back: Observation = serde_json::from_value(json).unwrap();
        assert_eq!(back.source, CapabilityId::new("telegram.receive"));
    }
}
