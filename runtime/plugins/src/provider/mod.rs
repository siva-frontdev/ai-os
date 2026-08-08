//! Real external providers for the AI-OS plugins.
//!
//! This module contains production provider clients (Gmail, WhatsApp) that
//! execute against real external services. Providers are the only code that
//! talks to the outside world; plugins translate tool calls into provider
//! effects and relay provider-confirmed outcomes (or structured failures)
//! back to the runtime.

pub mod config;
pub mod error;
pub mod gmail;
pub mod webhook;
pub mod whatsapp;

pub use config::{load_env_file, GmailConfig, Providers, WhatsAppConfig};
pub use error::ProviderError;
pub use gmail::{GmailProvider, GmailReceipt};
pub use webhook::{WebhookConfig, WebhookServer};
pub use whatsapp::{
    inbound_observation, InboundWhatsAppMessage, WhatsAppProvider, WhatsAppReceipt,
};
