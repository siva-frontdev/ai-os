//! # Context Manager
//!
//! Propagates distributed-trace-style context across async task
//! boundaries.  Each context carries a `trace_id`, `span_id`,
//! parent reference, and arbitrary metadata.
//!
//! ## Design
//!
//! * **Thread-local** — the current context is stored in a
//!   `tokio::task_local!` so it follows async control flow
//!   without explicit parameter passing.
//! * **Parent-child** — `create_child` generates a new `span_id`
//!   while keeping the same `trace_id`.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::RuntimeError;

// ── Context struct ───────────────────────────────────────

/// Immutable context value propagated through async execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
    pub metadata: HashMap<String, String>,
}

impl Context {
    pub fn new() -> Self {
        let trace_id = Uuid::new_v4().to_string();
        let span_id = Uuid::new_v4().to_string();
        Self {
            trace_id,
            span_id,
            parent_span_id: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    pub fn child(&self) -> Self {
        Self {
            trace_id: self.trace_id.clone(),
            span_id: Uuid::new_v4().to_string(),
            parent_span_id: Some(self.span_id.clone()),
            metadata: self.metadata.clone(),
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

// ── Thread-local current context ─────────────────────────

tokio::task_local! {
    static CURRENT_CONTEXT: Context;
}

// ── Context manager trait ─────────────────────────────────

/// Manages context creation and propagation (dyn-compatible).
pub trait ContextManager: Debug + Send + Sync {
    /// Return the current context, or create a root one.
    fn current(&self) -> Context;

    /// Create a child span for `parent`.
    fn create_child(&self, parent: &Context) -> Context;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug, Default)]
pub struct DefaultContextManager;

impl DefaultContextManager {
    pub fn new() -> Self {
        Self
    }
}

impl ContextManager for DefaultContextManager {
    fn current(&self) -> Context {
        CURRENT_CONTEXT.try_with(|c| c.clone()).unwrap_or_default()
    }

    fn create_child(&self, parent: &Context) -> Context {
        parent.child()
    }
}

/// Execute a future within the given context (standalone helper).
pub async fn run_with_context<F, T>(ctx: Context, f: F) -> T
where
    F: std::future::Future<Output = T> + Send,
{
    CURRENT_CONTEXT.scope(ctx, f).await
}

// ── Convenience functions ─────────────────────────────────

/// Execute a future within a new root context.
pub async fn with_new_context<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T> + Send,
{
    let ctx = Context::new();
    CURRENT_CONTEXT.scope(ctx, f).await
}

/// Execute a future within a child span of the current context.
pub async fn with_child_context<F, T>(metadata: HashMap<String, String>, f: F) -> T
where
    F: std::future::Future<Output = T> + Send,
{
    let parent = CURRENT_CONTEXT.try_with(|c| c.clone()).unwrap_or_default();
    let child = Context {
        trace_id: parent.trace_id,
        span_id: Uuid::new_v4().to_string(),
        parent_span_id: Some(parent.span_id),
        metadata,
    };
    CURRENT_CONTEXT.scope(child, f).await
}

// ── Event ─────────────────────────────────────────────────

/// Fired when a new context is created as a child of another.
#[derive(Debug, Clone)]
pub struct ContextCreated {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

impl ai_os_core::events::Event for ContextCreated {
    fn event_type(&self) -> &'static str {
        "runtime.context_created"
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_creation() {
        let ctx = Context::new();
        assert!(!ctx.trace_id.is_empty());
        assert!(!ctx.span_id.is_empty());
        assert!(ctx.parent_span_id.is_none());
    }

    #[test]
    fn child_context_preserves_trace() {
        let parent = Context::new().with_metadata("user", "alice");
        let child = parent.child();
        assert_eq!(child.trace_id, parent.trace_id);
        assert_ne!(child.span_id, parent.span_id);
        assert_eq!(child.parent_span_id, Some(parent.span_id.clone()));
        assert_eq!(child.metadata.get("user").unwrap(), "alice");
    }

    #[tokio::test]
    async fn run_with_context_propagates() {
        let ctx = Context::new().with_metadata("key", "val");
        let result = run_with_context(ctx, async {
            let current = CURRENT_CONTEXT.try_with(|c| c.clone()).unwrap();
            current.metadata.get("key").cloned()
        })
        .await;
        assert_eq!(result, Some("val".into()));
    }

    #[tokio::test]
    async fn current_falls_back_to_default() {
        let mgr = DefaultContextManager::new();
        let ctx = mgr.current();
        assert!(!ctx.trace_id.is_empty()); // default root
    }
}
