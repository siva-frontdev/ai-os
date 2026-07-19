#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod capability;
mod context;
mod set;

pub use capability::{Capability, Pid};
pub use context::CapabilityContext;
pub use set::CapabilitySet;

#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn test_empty_set_denies() {
        let set = CapabilitySet::new();
        assert!(!set.check(&Capability::ProcessSpawn));
        assert!(!set.check(&Capability::TerminalExecute));
    }

    #[test]
    fn test_grant_check() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::ProcessSpawn);
        assert!(set.check(&Capability::ProcessSpawn));
        assert!(!set.check(&Capability::TerminalExecute));
    }

    #[test]
    fn test_revoke() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::ProcessSpawn);
        assert!(set.check(&Capability::ProcessSpawn));
        set.revoke(&Capability::ProcessSpawn);
        assert!(!set.check(&Capability::ProcessSpawn));
    }

    #[test]
    fn test_admin_bypasses_all_checks() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::Admin);
        assert!(set.check(&Capability::ProcessSpawn));
        assert!(set.check(&Capability::TerminalExecute));
        assert!(set.check(&Capability::SystemShutdown));
    }

    #[test]
    fn test_check_path_with_star_wildcard() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::FileRead("*".into()));
        assert!(set.check_path(Capability::FileRead, "/etc/passwd"));
        assert!(set.check_path(Capability::FileRead, "/any/path"));
    }

    #[test]
    fn test_check_path_exact() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::FileRead("/tmp/foo.txt".into()));
        assert!(set.check_path(Capability::FileRead, "/tmp/foo.txt"));
        assert!(!set.check_path(Capability::FileRead, "/tmp/bar.txt"));
    }

    #[test]
    fn test_check_path_admin() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::Admin);
        assert!(set.check_path(Capability::FileRead, "/etc/shadow"));
    }

    #[test]
    fn test_is_empty() {
        let set = CapabilitySet::new();
        assert!(set.is_empty());
    }

    #[test]
    fn test_not_empty_after_grant() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::ProcessEnumerate);
        assert!(!set.is_empty());
    }

    #[test]
    fn test_extend() {
        let mut set = CapabilitySet::new();
        set.extend(vec![Capability::ProcessSpawn, Capability::TerminalExecute]);
        assert!(set.check(&Capability::ProcessSpawn));
        assert!(set.check(&Capability::TerminalExecute));
    }

    #[test]
    fn test_default_is_empty() {
        let set = CapabilitySet::default();
        assert!(set.is_empty());
    }

    #[test]
    fn test_from_hashset() {
        use std::collections::HashSet;
        let mut hs = HashSet::new();
        hs.insert(Capability::ProcessSpawn);
        let set = CapabilitySet::from(hs);
        assert!(set.check(&Capability::ProcessSpawn));
    }

    #[test]
    fn test_into_iter() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::ProcessSpawn);
        set.grant(Capability::TerminalExecute);
        let caps: Vec<_> = set.into_iter().collect();
        assert_eq!(caps.len(), 2);
    }

    #[test]
    fn test_context_new() {
        let ctx = CapabilityContext::new("test-subject");
        assert_eq!(ctx.subject_id, "test-subject");
        assert!(ctx.capabilities.is_empty());
    }

    #[test]
    fn test_capability_display() {
        assert_eq!(format!("{}", Capability::ProcessSpawn), "ProcessSpawn");
        assert_eq!(format!("{}", Capability::Admin), "Admin");
        assert_eq!(format!("{}", Capability::FileRead("/tmp".into())), "FileRead(/tmp)");
        assert_eq!(format!("{}", Capability::SystemShutdown), "SystemShutdown");
    }

    #[test]
    fn test_pid_display() {
        let pid = Pid(42);
        assert_eq!(format!("{}", pid), "42");
    }

    #[test]
    fn test_capability_serde_roundtrip() {
        let cap = Capability::ProcessSpawn;
        let json = serde_json::to_string(&cap).unwrap();
        let deserialized: Capability = serde_json::from_str(&json).unwrap();
        assert_eq!(cap, deserialized);
    }

    #[test]
    fn test_capability_set_serde_roundtrip() {
        let mut set = CapabilitySet::new();
        set.grant(Capability::ProcessSpawn);
        set.grant(Capability::Admin);
        let json = serde_json::to_string(&set).unwrap();
        let deserialized: CapabilitySet = serde_json::from_str(&json).unwrap();
        assert!(deserialized.check(&Capability::ProcessSpawn));
        assert!(deserialized.check(&Capability::SystemShutdown));
    }
}
