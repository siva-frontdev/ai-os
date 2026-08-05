//! Calendar plugin: an in-memory event store with `calendar.read` and
//! `calendar.create_event`. Dates are `YYYY-MM-DD` strings; when no date is
//! given to `calendar.read`, today (UTC) is used.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use ai_os_mcp_server::{McpPlugin, McpServerError, McpTool, ToolOutcome};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::util::{now_rfc3339, str_arg};

/// Internal event store.
#[derive(Debug)]
struct CalendarInner {
    events: Mutex<Vec<Value>>,
    next_id: AtomicU64,
}

/// A real calendar plugin backed by an in-memory event store.
#[derive(Debug)]
pub struct CalendarPlugin {
    inner: Arc<CalendarInner>,
}

impl Default for CalendarPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl CalendarPlugin {
    /// Create a plugin with an empty event store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(CalendarInner {
                events: Mutex::new(Vec::new()),
                next_id: AtomicU64::new(1),
            }),
        }
    }

    /// The number of scheduled events.
    pub async fn event_count(&self) -> usize {
        self.inner.events.lock().await.len()
    }
}

#[async_trait]
impl McpPlugin for CalendarPlugin {
    fn name(&self) -> &'static str {
        "calendar"
    }

    fn tools(&self) -> Vec<McpTool> {
        vec![
            McpTool {
                name: "calendar.read".into(),
                description: "List events for a date (YYYY-MM-DD); defaults to today".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {"date": {"type": "string"}}
                }),
            },
            McpTool {
                name: "calendar.create_event".into(),
                description: "Schedule a new event for a date (YYYY-MM-DD)".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "title": {"type": "string"},
                        "date": {"type": "string"},
                        "startTime": {"type": "string"},
                        "endTime": {"type": "string"},
                        "location": {"type": "string"}
                    },
                    "required": ["title", "date"]
                }),
            },
        ]
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolOutcome, McpServerError> {
        match name {
            "calendar.read" => Ok(self.read(arguments).await),
            "calendar.create_event" => self.create_event(arguments).await,
            _ => Err(McpServerError::UnknownTool(name.to_string())),
        }
    }
}

impl CalendarPlugin {
    async fn read(&self, args: Value) -> ToolOutcome {
        let date = str_arg(&args, "date").unwrap_or_else(today_utc);
        let events = self
            .inner
            .events
            .lock()
            .await
            .iter()
            .filter(|event| event.get("date").and_then(Value::as_str) == Some(date.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        ToolOutcome {
            is_error: false,
            result: json!({
                "date": date,
                "count": events.len(),
                "events": events,
            }),
            observations: vec![],
        }
    }

    async fn create_event(&self, args: Value) -> Result<ToolOutcome, McpServerError> {
        let Some(title) = str_arg(&args, "title") else {
            return Ok(tool_error("calendar.create_event requires a 'title'"));
        };
        let Some(date) = str_arg(&args, "date") else {
            return Ok(tool_error("calendar.create_event requires a 'date'"));
        };
        let id = self.inner.next_id.fetch_add(1, Ordering::SeqCst);
        let event_id = format!("evt-{id}");

        let event = json!({
            "eventId": event_id,
            "title": title,
            "date": date,
            "startTime": str_arg(&args, "startTime"),
            "endTime": str_arg(&args, "endTime"),
            "location": str_arg(&args, "location"),
            "createdAt": now_rfc3339(),
        });
        self.inner.events.lock().await.push(event.clone());

        Ok(ToolOutcome {
            is_error: false,
            result: json!({
                "eventId": event_id,
                "title": title,
                "date": date,
                "status": "scheduled",
            }),
            observations: vec![json!({
                "source": "calendar.create_event",
                "kind": "status_changed",
                "text": format!("event scheduled: {title} on {date}"),
                "event": event,
            })],
        })
    }
}

fn today_utc() -> String {
    chrono::Utc::now().format("%Y-%m-%d").to_string()
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

    #[tokio::test]
    async fn create_and_read_event_for_date() {
        let plugin = CalendarPlugin::new();
        let created = plugin
            .call_tool(
                "calendar.create_event",
                json!({"title": "standup", "date": "2026-08-06", "startTime": "09:00"}),
            )
            .await
            .unwrap();
        assert!(!created.is_error);
        assert_eq!(created.result["status"], "scheduled");
        assert!(created.result["eventId"].as_str().is_some());
        assert_eq!(plugin.event_count().await, 1);
        assert_eq!(created.observations[0]["source"], "calendar.create_event");

        let read = plugin
            .call_tool("calendar.read", json!({"date": "2026-08-06"}))
            .await
            .unwrap();
        assert!(!read.is_error);
        assert_eq!(read.result["count"], 1);
        assert_eq!(read.result["events"][0]["title"], "standup");

        let other = plugin
            .call_tool("calendar.read", json!({"date": "2026-08-07"}))
            .await
            .unwrap();
        assert_eq!(other.result["count"], 0);
    }

    #[tokio::test]
    async fn create_event_requires_title_and_date() {
        let plugin = CalendarPlugin::new();
        let no_title = plugin
            .call_tool("calendar.create_event", json!({"date": "2026-08-06"}))
            .await
            .unwrap();
        assert!(no_title.is_error);
        let no_date = plugin
            .call_tool("calendar.create_event", json!({"title": "x"}))
            .await
            .unwrap();
        assert!(no_date.is_error);
        assert_eq!(plugin.event_count().await, 0);
    }

    #[test]
    fn today_utc_is_formatted_iso_date() {
        let today = today_utc();
        assert_eq!(today.len(), 10);
        assert_eq!(today.as_bytes()[4], b'-');
        assert_eq!(today.as_bytes()[7], b'-');
    }
}
