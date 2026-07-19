//! OSAL-specific user types not present in `osal_core`.

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use osal_core::Uid;

/// Authentication credential.
///
/// Carries the proof material a user presents to establish identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Credential {
    /// Password-based authentication.
    Password {
        /// Hash of the user's password.
        hash: String,
    },
    /// Public-key-based authentication.
    Key {
        /// The public key material.
        public_key: String,
    },
    /// Token-based authentication.
    Token {
        /// The bearer token string.
        token: String,
    },
    /// No credential (anonymous / default user).
    None,
}

/// Active user session.
///
/// Created after successful authentication or user switch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// Numeric user ID of the authenticated user.
    pub uid: Uid,
    /// Username of the authenticated user.
    pub username: String,
    /// When the session was created.
    pub started_at: DateTime<Utc>,
    /// Session access token for subsequent authorization.
    pub access_token: String,
}
