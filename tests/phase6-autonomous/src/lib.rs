//! Phase 6 — Autonomous Agent integration test harness.
//!
//! Provides lightweight, deterministic stubs for the trait dependencies of
//! the Brain orchestrator so integration tests can drive the real
//! `BrainOrchestrator`, `GoalManager`, `AgentRegistry`, `RecoveryManager`,
//! `PolicyRegistry`, `Reflector`, and `LearningEngine` without external
//! services.

#![forbid(unsafe_code)]

pub mod stubs;
