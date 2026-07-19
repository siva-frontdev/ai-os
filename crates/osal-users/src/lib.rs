//! OSAL Users — user and group identity, authentication, and session management.
//!
//! # Re-exports from `osal_core`
//!
//! | Item | Description |
//! |---|---|
//! | [`UserManager`](osal_core::UserManager) | Async trait for user/group operations |
//! | [`UserError`](osal_core::UserError) | User-operation error variants |
//! | [`UserInfo`](osal_core::UserInfo) | User identity information |
//! | [`GroupInfo`](osal_core::GroupInfo) | Group identity information |
//! | [`Uid`](osal_core::Uid) | Numeric user ID |
//! | [`Gid`](osal_core::Gid) | Numeric group ID |
//! | [`UserId`](osal_core::UserId) | String-based user identifier |
//! | [`OsalEvent`](osal_core::OsalEvent) | System-wide OS events (includes `UserLoggedIn`, `UserLoggedOut`) |
//!
//! # Unique types
//!
//! - [`Credential`] — authentication credential (password, key, token, or none)
//! - [`UserSession`] — active user session with access token
//!
//! # No-op implementation
//!
//! [`DefaultUserManager`] is a placeholder that returns an error for every
//! operation. Useful during early development or as a base for wrappers.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

use async_trait::async_trait;
use tokio::sync::mpsc::{self, Receiver};

/// OSAL-specific user types not present in `osal_core`.
pub mod types;

pub use osal_core::{
    Gid, GroupInfo, OsalEvent, Uid, UserError, UserId, UserInfo, UserManager,
};
pub use types::{Credential, UserSession};

/// No-op implementation of [`UserManager`].
///
/// Returns [`UserError::Other`] for every operation. Use this as a default
/// before a real implementation is wired in.
#[derive(Debug)]
pub struct DefaultUserManager;

impl DefaultUserManager {
    /// Create a new `DefaultUserManager`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultUserManager {
    fn default() -> Self {
        Self
    }
}

#[async_trait]
impl UserManager for DefaultUserManager {
    async fn current_user(&self, _ctx: &osal_capabilities::CapabilityContext) -> Result<UserInfo, UserError> {
        Err(UserError::Other("DefaultUserManager: not implemented".into()))
    }

    async fn enumerate_users(
        &self,
        _ctx: &osal_capabilities::CapabilityContext,
    ) -> Result<Vec<UserInfo>, UserError> {
        Err(UserError::Other("DefaultUserManager: not implemented".into()))
    }

    async fn enumerate_groups(
        &self,
        _ctx: &osal_capabilities::CapabilityContext,
    ) -> Result<Vec<GroupInfo>, UserError> {
        Err(UserError::Other("DefaultUserManager: not implemented".into()))
    }

    async fn switch_user(
        &self,
        _ctx: &osal_capabilities::CapabilityContext,
        _user_id: &UserId,
    ) -> Result<(), UserError> {
        Err(UserError::Other("DefaultUserManager: not implemented".into()))
    }

    async fn get_user_by_uid(
        &self,
        _ctx: &osal_capabilities::CapabilityContext,
        _uid: Uid,
    ) -> Result<UserInfo, UserError> {
        Err(UserError::Other("DefaultUserManager: not implemented".into()))
    }

    fn events(&self) -> Receiver<OsalEvent> {
        let (_, rx) = mpsc::channel(1);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_ctx() -> osal_capabilities::CapabilityContext {
        osal_capabilities::CapabilityContext::new("test")
    }

    #[tokio::test]
    async fn test_default_user_manager_current_user_returns_error() {
        let mgr = DefaultUserManager;
        assert!(mgr.current_user(&dummy_ctx()).await.is_err());
    }

    #[tokio::test]
    async fn test_default_user_manager_enumerate_users_returns_error() {
        let mgr = DefaultUserManager;
        assert!(mgr.enumerate_users(&dummy_ctx()).await.is_err());
    }

    #[tokio::test]
    async fn test_default_user_manager_enumerate_groups_returns_error() {
        let mgr = DefaultUserManager;
        assert!(mgr.enumerate_groups(&dummy_ctx()).await.is_err());
    }

    #[tokio::test]
    async fn test_default_user_manager_switch_user_returns_error() {
        let mgr = DefaultUserManager;
        let user_id = UserId("test".into());
        assert!(mgr.switch_user(&dummy_ctx(), &user_id).await.is_err());
    }

    #[tokio::test]
    async fn test_default_user_manager_get_user_by_uid_returns_error() {
        let mgr = DefaultUserManager;
        assert!(mgr.get_user_by_uid(&dummy_ctx(), Uid(1000)).await.is_err());
    }

    #[test]
    fn test_default_user_manager_events_returns_closed_receiver() {
        let mgr = DefaultUserManager;
        let mut rx = mgr.events();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_default_user_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<DefaultUserManager>();
    }

    #[test]
    fn test_default_user_manager_new_and_default() {
        let a = DefaultUserManager::new();
        let b = DefaultUserManager::default();
        let c = DefaultUserManager;
        std::mem::drop((a, b, c));
    }

    #[test]
    fn test_credential_variants() {
        let pw = Credential::Password { hash: "abc123".into() };
        let key = Credential::Key { public_key: "ssh-rsa ...".into() };
        let token = Credential::Token { token: "tok_xxx".into() };
        let none = Credential::None;
        assert!(matches!(pw, Credential::Password { .. }));
        assert!(matches!(key, Credential::Key { .. }));
        assert!(matches!(token, Credential::Token { .. }));
        assert!(matches!(none, Credential::None));
    }

    #[test]
    fn test_credential_serde_roundtrip() {
        let cred = Credential::Password { hash: "abc".into() };
        let json = serde_json::to_string(&cred).unwrap();
        let deserialized: Credential = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&cred).unwrap(), serde_json::to_string(&deserialized).unwrap());
    }

    #[test]
    fn test_user_session_creation() {
        let session = UserSession {
            uid: Uid(1000),
            username: "alice".into(),
            started_at: chrono::Utc::now(),
            access_token: "tok_abc".into(),
        };
        assert_eq!(session.uid, Uid(1000));
        assert_eq!(session.username, "alice");
        assert!(!session.access_token.is_empty());
    }

    #[test]
    fn test_user_session_serde_roundtrip() {
        let session = UserSession {
            uid: Uid(1001),
            username: "bob".into(),
            started_at: chrono::Utc::now(),
            access_token: "tok_xyz".into(),
        };
        let json = serde_json::to_string(&session).unwrap();
        let deserialized: UserSession = serde_json::from_str(&json).unwrap();
        assert_eq!(session.uid, deserialized.uid);
        assert_eq!(session.username, deserialized.username);
        assert_eq!(session.access_token, deserialized.access_token);
    }
}
