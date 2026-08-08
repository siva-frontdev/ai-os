//! # AI-OS LIFE CLI
//!
//! `life` is the AI-OS command-line interface. It provides a guided,
//! non-interactive-only setup wizard for external providers so credentials
//! are obtained through real OAuth flows (never the OAuth Playground) and
//! stored in a gitignored `.env` file the runtime plugins read.
//!
//! The setup surface is designed to be provider-agnostic: every provider
//! implements the [`ProviderSetup`] trait and registers itself in
//! [`setup::providers`]. `life setup whatsapp` will reuse the same wizard
//! plumbing (localhost callback server, browser opening, env persistence,
//! structured errors) without a redesign.
//!
//! See `docs/runtime/gmail-setup.md` for the Gmail walkthrough.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod error;
pub mod setup;

pub use error::SetupError;
pub use setup::{providers, ProviderSetup, SetupContext};
