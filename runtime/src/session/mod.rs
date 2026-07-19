//! # Session Manager
//!
//! Manages user/session lifecycle.  Each session carries a
//! [`PermissionContext`] and metadata.
//!
//! ## Design
//!
//! * **Immutable after creation** — sessions are created once
//!   and destroyed once.  Their permission context does not
//!   change.
//! * **Thread-safe store** — sessions held in an
//!   `Arc<RwLock<HashMap>>`.
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::RuntimeError;
use crate::permission::PermissionContext;

// ── Session ID ────────────────────────────────────────────

#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ── Session state ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Active,
    Idle,
    Draining,
    Destroyed,
}

// ── Session struct ───────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub state: SessionState,
    pub created_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
    pub permission_ctx: PermissionContext,
}

// ── Session manager trait ────────────────────────────────

/// Manages session lifecycle.
pub trait SessionManager: Debug + Send + Sync {
    fn create_session(&self, metadata: HashMap<String, String>) -> Session;
    fn get_session(&self, id: &SessionId) -> Option<Session>;
    fn destroy_session(&self, id: &SessionId) -> Result<(), RuntimeError>;
    fn list_sessions(&self) -> Vec<Session>;
    fn session_count(&self) -> usize;
}

// ── Implementation ───────────────────────────────────────

#[derive(Debug)]
pub struct DefaultSessionManager {
    sessions: RwLock<HashMap<SessionId, Session>>,
}

impl DefaultSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for DefaultSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager for DefaultSessionManager {
    fn create_session(&self, metadata: HashMap<String, String>) -> Session {
        let id = SessionId::new();
        let session = Session {
            id: id.clone(),
            state: SessionState::Active,
            created_at: Utc::now(),
            permission_ctx: crate::permission::roles::user(id.as_str()),
            metadata,
        };
        if let Ok(mut guard) = self.sessions.write() {
            guard.insert(id, session.clone());
        }
        session
    }

    fn get_session(&self, id: &SessionId) -> Option<Session> {
        self.sessions.read().ok()?.get(id).cloned()
    }

    fn destroy_session(&self, id: &SessionId) -> Result<(), RuntimeError> {
        let mut guard = self
            .sessions
            .write()
            .map_err(|_| RuntimeError::Session("lock poisoned".into()))?;
        if guard.remove(id).is_none() {
            return Err(RuntimeError::Session(format!("session {} not found", id)));
        }
        Ok(())
    }

    fn list_sessions(&self) -> Vec<Session> {
        self.sessions
            .read()
            .map(|g| g.values().cloned().collect())
            .unwrap_or_default()
    }

    fn session_count(&self) -> usize {
        self.sessions.read().map(|g| g.len()).unwrap_or(0)
    }
}

// ── Events ────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SessionCreated {
    pub session_id: SessionId,
}

impl ai_os_core::events::Event for SessionCreated {
    fn event_type(&self) -> &'static str {
        "runtime.session_created"
    }
}

#[derive(Debug, Clone)]
pub struct SessionDestroyed {
    pub session_id: SessionId,
}

impl ai_os_core::events::Event for SessionDestroyed {
    fn event_type(&self) -> &'static str {
        "runtime.session_destroyed"
    }
}

// ── Tests ─────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let mgr = DefaultSessionManager::new();
        let session = mgr.create_session(HashMap::new());
        let fetched = mgr.get_session(&session.id);
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, session.id);
    }

    #[test]
    fn destroy_removes() {
        let mgr = DefaultSessionManager::new();
        let session = mgr.create_session(HashMap::new());
        mgr.destroy_session(&session.id).unwrap();
        assert!(mgr.get_session(&session.id).is_none());
    }

    #[test]
    fn destroy_missing_fails() {
        let mgr = DefaultSessionManager::new();
        let id = SessionId::new();
        assert!(mgr.destroy_session(&id).is_err());
    }

    #[test]
    fn list_and_count() {
        let mgr = DefaultSessionManager::new();
        assert_eq!(mgr.session_count(), 0);
        mgr.create_session(HashMap::new());
        mgr.create_session(HashMap::new());
        assert_eq!(mgr.session_count(), 2);
        assert_eq!(mgr.list_sessions().len(), 2);
    }

    #[test]
    fn session_has_permissions() {
        let mgr = DefaultSessionManager::new();
        let session = mgr.create_session(HashMap::new());
        assert!(session
            .permission_ctx
            .has_permission(&crate::permission::Permission::Read));
        assert!(session.permission_ctx.has_role("user"));
    }
}
