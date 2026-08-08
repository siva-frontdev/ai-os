//! Autonomous communication policy.
//!
//! Orchestration-level rules that decide whether a proactive
//! `Decision::Communicate` should be delivered now, deferred, or stored.
//! This is the concrete realization of the `communication.policy`
//! ruleset described in `docs/architecture/autonomous-companion.md`.
//!
//! User-initiated replies are always delivered; the policy only gates
//! **proactive** (self-initiated) communication, so the companion never
//! becomes noisy and respects quiet hours.

use chrono::Timelike;

use super::settings::{CommunicationSettings, NotificationPriority};

/// What the policy decided to do with a proactive message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyVerdict {
    /// Deliver now.
    Allow,
    /// Deliver now (critical priority) — overrides quiet hours.
    AllowCritical,
    /// Defer until the next non-quiet window (e.g. next morning bundle).
    Defer,
    /// Do not communicate; store only.
    Store,
}

/// Whether a message originated from the user or from the companion's
/// own initiative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageOrigin {
    /// The user sent an observation/message — always reply.
    UserInitiated,
    /// The companion decided to speak on its own (tick/rhythm/completion).
    Proactive,
}

/// Evaluates proactive communication against the configured policy.
#[derive(Debug, Clone)]
pub struct CommunicationPolicy {
    pub quiet_hours_enabled: bool,
    pub quiet_start_hour: u8,
    pub quiet_end_hour: u8,
    pub quiet_min_priority: NotificationPriority,
}

impl CommunicationPolicy {
    pub fn from_settings(settings: &CommunicationSettings) -> Self {
        Self {
            quiet_hours_enabled: settings.quiet_hours_enabled,
            quiet_start_hour: settings.quiet_start_hour,
            quiet_end_hour: settings.quiet_end_hour,
            quiet_min_priority: settings.quiet_min_priority,
        }
    }

    /// Default policy (matches `CommunicationSettings::default()`).
    pub fn default_policy() -> Self {
        Self::from_settings(&CommunicationSettings::default())
    }

    /// Is `local_hour` (0-23) inside the configured quiet window?
    ///
    /// Handles windows that wrap midnight (22:00–08:00) and windows
    /// entirely within a day (23:00–23:30 is not representable by hours,
    /// so any start == end means "disabled").
    pub fn is_quiet_hour(&self, local_hour: u8) -> bool {
        if !self.quiet_hours_enabled || self.quiet_start_hour == self.quiet_end_hour {
            return false;
        }
        if self.quiet_start_hour < self.quiet_end_hour {
            local_hour >= self.quiet_start_hour && local_hour < self.quiet_end_hour
        } else {
            // Wraps midnight: e.g. start 22, end 8 → [22..24) ∪ [0..8)
            local_hour >= self.quiet_start_hour || local_hour < self.quiet_end_hour
        }
    }

    /// Decide whether a proactive message may be delivered.
    ///
    /// `local_hour` is the hour of the user's local time (0-23). The
    /// companion determines the user's timezone from settings; the default
    /// is the system local time.
    pub fn evaluate(
        &self,
        origin: MessageOrigin,
        priority: NotificationPriority,
        local_hour: u8,
    ) -> PolicyVerdict {
        // User-initiated replies are never gated.
        if origin == MessageOrigin::UserInitiated {
            return PolicyVerdict::Allow;
        }

        if !self.is_quiet_hour(local_hour) {
            return PolicyVerdict::Allow;
        }

        // Quiet hours active.
        match priority {
            NotificationPriority::Critical => PolicyVerdict::AllowCritical,
            NotificationPriority::Low => PolicyVerdict::Store,
            _ if priority >= self.quiet_min_priority => PolicyVerdict::AllowCritical,
            _ => PolicyVerdict::Defer,
        }
    }

    /// Compute the local hour for the current instant.
    pub fn now_hour() -> u8 {
        chrono::Local::now().hour() as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> CommunicationPolicy {
        CommunicationPolicy {
            quiet_hours_enabled: true,
            quiet_start_hour: 22,
            quiet_end_hour: 8,
            quiet_min_priority: NotificationPriority::High,
        }
    }

    #[test]
    fn user_initiated_always_delivered_even_in_quiet_hours() {
        let p = policy();
        assert_eq!(
            p.evaluate(MessageOrigin::UserInitiated, NotificationPriority::Low, 23),
            PolicyVerdict::Allow
        );
        assert_eq!(
            p.evaluate(
                MessageOrigin::UserInitiated,
                NotificationPriority::Normal,
                3
            ),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn proactive_normal_deferred_during_quiet_hours() {
        let p = policy();
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Normal, 23),
            PolicyVerdict::Defer
        );
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Normal, 2),
            PolicyVerdict::Defer
        );
    }

    #[test]
    fn proactive_low_stored_during_quiet_hours() {
        let p = policy();
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Low, 23),
            PolicyVerdict::Store
        );
    }

    #[test]
    fn critical_interrupts_during_quiet_hours() {
        let p = policy();
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Critical, 3),
            PolicyVerdict::AllowCritical
        );
    }

    #[test]
    fn allowed_outside_quiet_hours() {
        let p = policy();
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Normal, 12),
            PolicyVerdict::Allow
        );
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Low, 12),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn quiet_hour_boundaries() {
        let p = policy();
        assert!(!p.is_quiet_hour(21), "21:00 is before quiet hours");
        assert!(p.is_quiet_hour(22), "22:00 is quiet");
        assert!(p.is_quiet_hour(23), "23:00 is quiet");
        assert!(p.is_quiet_hour(7), "07:00 is still quiet");
        assert!(!p.is_quiet_hour(8), "08:00 ends quiet hours");
        assert!(!p.is_quiet_hour(9), "09:00 is not quiet");
    }

    #[test]
    fn quiet_hours_disabled_always_allows() {
        let p = CommunicationPolicy {
            quiet_hours_enabled: false,
            ..policy()
        };
        assert!(!p.is_quiet_hour(23));
        assert_eq!(
            p.evaluate(MessageOrigin::Proactive, NotificationPriority::Normal, 3),
            PolicyVerdict::Allow
        );
    }

    #[test]
    fn equal_start_end_disables_window() {
        let p = CommunicationPolicy {
            quiet_start_hour: 0,
            quiet_end_hour: 0,
            ..policy()
        };
        assert!(!p.is_quiet_hour(0));
    }

    #[test]
    fn now_hour_in_range() {
        let h = CommunicationPolicy::now_hour();
        assert!((0..=23).contains(&h), "hour should be 0-23, got {h}");
    }
}
