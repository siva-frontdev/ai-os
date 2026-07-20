use async_trait::async_trait;
use execution_core::*;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::error::SandboxError;

// ── InMemoryProfileStore ───────────────────────────────────

#[derive(Debug)]
pub struct InMemoryProfileStore {
    profiles: RwLock<HashMap<String, SandboxProfile>>,
}

impl InMemoryProfileStore {
    pub fn new() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert("default".into(), SandboxProfile::new("default"));
        Self {
            profiles: RwLock::new(profiles),
        }
    }

    pub fn get(&self, name: &str) -> Result<SandboxProfile, SandboxError> {
        self.profiles
            .read()
            .map_err(|_| SandboxError::LockPoisoned)?
            .get(name)
            .cloned()
            .ok_or_else(|| SandboxError::ProfileNotFound(name.into()))
    }

    pub fn list(&self) -> Result<Vec<SandboxProfile>, SandboxError> {
        self.profiles
            .read()
            .map_err(|_| SandboxError::LockPoisoned)
            .map(|p| p.values().cloned().collect())
    }

    pub fn upsert(&self, profile: SandboxProfile) -> Result<(), SandboxError> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|_| SandboxError::LockPoisoned)?;
        profiles.insert(profile.name.clone(), profile);
        Ok(())
    }

    pub fn remove(&self, name: &str) -> Result<(), SandboxError> {
        let mut profiles = self
            .profiles
            .write()
            .map_err(|_| SandboxError::LockPoisoned)?;
        profiles.remove(name);
        Ok(())
    }
}

impl Default for InMemoryProfileStore {
    fn default() -> Self {
        Self::new()
    }
}

// ── DefaultSandboxEnforcer ──────────────────────────────────

#[derive(Debug)]
pub struct DefaultSandboxEnforcer {
    store: InMemoryProfileStore,
}

impl DefaultSandboxEnforcer {
    pub fn new() -> Self {
        Self {
            store: InMemoryProfileStore::new(),
        }
    }

    pub fn store(&self) -> &InMemoryProfileStore {
        &self.store
    }
}

impl Default for DefaultSandboxEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SandboxEnforcer for DefaultSandboxEnforcer {
    async fn resolve_profile(&self, profile_name: &str) -> ExecutionResult<SandboxProfile> {
        self.store.get(profile_name).map_err(Into::into)
    }

    async fn validate(&self, plan: &ExecutionPlan) -> ExecutionResult<()> {
        let profile = self.resolve_profile(&plan.sandbox_profile.name).await?;

        self.check_permissions(&plan.binding, &profile).await?;

        match (&profile.isolation, &plan.binding) {
            (IsolationLevel::None, _) => {}
            (IsolationLevel::Process, _) => {}
            (IsolationLevel::Wasm, ToolBinding::Wasm { .. }) => {}
            (IsolationLevel::Wasm, _) => {
                return Err(execution_core::ExecutionError::IsolationUnsupported(
                    "Wasm isolation requires a Wasm binding".into(),
                ));
            }
            (IsolationLevel::Container, ToolBinding::Container { .. }) => {}
            (IsolationLevel::Container, _) => {
                return Err(execution_core::ExecutionError::IsolationUnsupported(
                    "Container isolation requires a Container binding".into(),
                ));
            }
        }

        Ok(())
    }

    async fn check_permissions(
        &self,
        binding: &ToolBinding,
        profile: &SandboxProfile,
    ) -> ExecutionResult<()> {
        match binding {
            ToolBinding::Subprocess { binary, .. } => {
                if !profile.allowed_paths.is_empty()
                    && !profile.allowed_paths.iter().any(|p| binary.starts_with(p))
                {
                    return Err(SandboxError::FilesystemRestriction {
                        path: binary.clone(),
                        reason: format!("binary not in allowed paths: {:?}", profile.allowed_paths),
                    }
                    .into());
                }

                if profile.denied_paths.iter().any(|p| binary.starts_with(p)) {
                    return Err(SandboxError::PermissionDenied {
                        tool: "subprocess".into(),
                        permission: "filesystem.execute".into(),
                        detail: format!("binary {} is in denied paths", binary),
                    }
                    .into());
                }
            }
            ToolBinding::Wasm { fuel_limit, .. } => {
                if profile.isolation == IsolationLevel::None {
                    return Err(SandboxError::IsolationUnsupported(
                        "Wasm execution requires Process or higher isolation".into(),
                    )
                    .into());
                }

                if let Some(fuel) = fuel_limit {
                    if let Some(max_files) = profile.max_files {
                        if *fuel > max_files * 1000 {
                            return Err(SandboxError::ConstraintViolation {
                                constraint: "fuel_limit".into(),
                                value: fuel.to_string(),
                            }
                            .into());
                        }
                    }
                    if let Some(max_procs) = profile.max_processes {
                        if *fuel > max_procs * 5000 {
                            return Err(SandboxError::ConstraintViolation {
                                constraint: "fuel_limit".into(),
                                value: fuel.to_string(),
                            }
                            .into());
                        }
                    }
                }
            }
            ToolBinding::Container {
                mounts, network, ..
            } => {
                if profile.isolation != IsolationLevel::Container {
                    return Err(SandboxError::IsolationUnsupported(
                        "Container execution requires Container isolation".into(),
                    )
                    .into());
                }

                if *network != ContainerNetwork::None && !profile.network_access {
                    return Err(SandboxError::PermissionDenied {
                        tool: "container".into(),
                        permission: "network".into(),
                        detail: format!("network access ({:?}) denied by profile", network),
                    }
                    .into());
                }

                for mount in mounts {
                    if !profile.allowed_paths.is_empty()
                        && !profile
                            .allowed_paths
                            .iter()
                            .any(|p| mount.source.starts_with(p))
                    {
                        return Err(SandboxError::FilesystemRestriction {
                            path: mount.source.clone(),
                            reason: format!(
                                "mount source not in allowed paths: {:?}",
                                profile.allowed_paths
                            ),
                        }
                        .into());
                    }
                    if profile
                        .denied_paths
                        .iter()
                        .any(|p| mount.source.starts_with(p))
                    {
                        return Err(SandboxError::FilesystemRestriction {
                            path: mount.source.clone(),
                            reason: "mount source is in denied paths".into(),
                        }
                        .into());
                    }
                }
            }
        }
        Ok(())
    }

    async fn check_capabilities(
        &self,
        binding: &ToolBinding,
        required: &[String],
    ) -> ExecutionResult<()> {
        let supported_prefixes: &[&str] = match binding {
            ToolBinding::Subprocess { .. } => &["subprocess", "process", "exec", "shell"],
            ToolBinding::Wasm { .. } => &["wasm", "webassembly", "module"],
            ToolBinding::Container { .. } => &["container", "docker", "oci", "image"],
        };

        let tool_type = match binding {
            ToolBinding::Subprocess { .. } => "subprocess",
            ToolBinding::Wasm { .. } => "wasm",
            ToolBinding::Container { .. } => "container",
        };

        for req in required {
            let supported = supported_prefixes
                .iter()
                .any(|p| req == *p || req.starts_with(p));
            if !supported {
                return Err(SandboxError::CapabilityDenied {
                    tool: tool_type.into(),
                    capability: req.clone(),
                }
                .into());
            }
        }

        Ok(())
    }

    fn register_profile(&self, profile: SandboxProfile) {
        let _ = self.store.upsert(profile);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_plan(
        profile_name: &str,
        binding: ToolBinding,
        isolation: IsolationLevel,
    ) -> ExecutionPlan {
        let mut profile = SandboxProfile::new(profile_name);
        profile.isolation = isolation;
        ExecutionPlan {
            id: ExecutionId::new(),
            request: ExecutionRequest {
                requirement_id: "req-1".into(),
                capability_id: "cap-1".into(),
                inputs: HashMap::new(),
                context: ExecutionContext {
                    session: ExecutionSession {
                        session_id: "s-1".into(),
                        user_id: "u-1".into(),
                        roles: vec!["admin".into()],
                        permissions: ExecutionPermissions::default(),
                    },
                    trace_id: "t-1".into(),
                    span_id: "s-1".into(),
                    originating_goal: None,
                    originating_plan: None,
                },
                budget: ExecutionBudget::default(),
                priority: ExecutionPriority::default(),
                retry_policy: RetryPolicy::default(),
            },
            binding,
            sandbox_profile: profile,
            budget: ExecutionBudget::default(),
            priority: ExecutionPriority::default(),
            permissions: ExecutionPermissions::default(),
            routing_rules: Vec::new(),
            rollback_plan: None,
            state: ExecutionState::Planned,
            created_at: memory_core::Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_resolve_profile_found() {
        let enforcer = DefaultSandboxEnforcer::new();
        let profile = enforcer.resolve_profile("default").await;
        assert!(profile.is_ok());
        assert_eq!(profile.unwrap().name, "default");
    }

    #[tokio::test]
    async fn test_resolve_profile_not_found() {
        let enforcer = DefaultSandboxEnforcer::new();
        let profile = enforcer.resolve_profile("nonexistent").await;
        assert!(profile.is_err());
    }

    #[tokio::test]
    async fn test_validate_subprocess_allowed_path_passes() {
        let enforcer = DefaultSandboxEnforcer::new();
        let mut profile = SandboxProfile::new("readonly");
        profile.allowed_paths = vec!["/usr/bin".into(), "/bin".into()];
        enforcer.register_profile(profile);

        let plan = test_plan(
            "readonly",
            ToolBinding::Subprocess {
                binary: "/bin/ls".into(),
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: vec!["/tmp".into()],
                denied_binaries: vec![],
            },
            IsolationLevel::Process,
        );
        let result = enforcer.validate(&plan).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_subprocess_denied_path_fails() {
        let enforcer = DefaultSandboxEnforcer::new();
        let mut profile = SandboxProfile::new("restricted");
        profile.denied_paths = vec!["/usr/bin/rm".into(), "/bin/rm".into()];
        enforcer.register_profile(profile);

        let plan = test_plan(
            "restricted",
            ToolBinding::Subprocess {
                binary: "/bin/rm".into(),
                args: vec!["-rf".into(), "/".into()],
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: vec![],
                denied_binaries: vec![],
            },
            IsolationLevel::Process,
        );
        let result = enforcer.validate(&plan).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_permissions_container_network_denied() {
        let enforcer = DefaultSandboxEnforcer::new();
        let mut profile = SandboxProfile::new("no-network");
        profile.isolation = IsolationLevel::Container;
        profile.network_access = false;
        let profile_check = profile.clone();
        enforcer.register_profile(profile);

        let binding = ToolBinding::Container {
            image: "nginx".into(),
            command: vec!["nginx".into(), "-g".into(), "daemon off;".into()],
            mounts: vec![],
            network: ContainerNetwork::Bridge,
            pull_policy: PullPolicy::IfNotPresent,
            resource_limits: ContainerResources::default(),
        };
        let result = enforcer.check_permissions(&binding, &profile_check).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_permissions_container_network_allowed() {
        let enforcer = DefaultSandboxEnforcer::new();
        let mut profile = SandboxProfile::new("with-network");
        profile.isolation = IsolationLevel::Container;
        profile.network_access = true;
        let profile_check = profile.clone();
        enforcer.register_profile(profile);

        let binding = ToolBinding::Container {
            image: "nginx".into(),
            command: vec!["nginx".into()],
            mounts: vec![],
            network: ContainerNetwork::Bridge,
            pull_policy: PullPolicy::IfNotPresent,
            resource_limits: ContainerResources::default(),
        };
        let result = enforcer.check_permissions(&binding, &profile_check).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_capabilities_match() {
        let enforcer = DefaultSandboxEnforcer::new();
        let binding = ToolBinding::Subprocess {
            binary: "/bin/echo".into(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec![],
            denied_binaries: vec![],
        };
        let required = vec!["subprocess".into(), "exec".into()];
        let result = enforcer.check_capabilities(&binding, &required).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_check_capabilities_mismatch() {
        let enforcer = DefaultSandboxEnforcer::new();
        let binding = ToolBinding::Subprocess {
            binary: "/bin/echo".into(),
            args: vec![],
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: vec![],
            denied_binaries: vec![],
        };
        let required = vec!["wasm".into()];
        let result = enforcer.check_capabilities(&binding, &required).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_default_profile_initialized() {
        let enforcer = DefaultSandboxEnforcer::new();
        let profile = enforcer.resolve_profile("default").await;
        assert!(profile.is_ok());
        let p = profile.unwrap();
        assert_eq!(p.name, "default");
        assert_eq!(p.isolation, IsolationLevel::Process);
        assert!(!p.network_access);
    }

    #[tokio::test]
    async fn test_register_and_list_profiles() {
        let store = InMemoryProfileStore::new();
        let mut profile = SandboxProfile::new("custom");
        profile.isolation = IsolationLevel::Container;
        profile.network_access = true;
        store.upsert(profile.clone()).unwrap();

        let profiles = store.list().unwrap();
        assert_eq!(profiles.len(), 2);

        let retrieved = store.get("custom").unwrap();
        assert_eq!(retrieved.isolation, IsolationLevel::Container);
        assert!(retrieved.network_access);
    }

    #[tokio::test]
    async fn test_container_mount_restriction_fails() {
        let enforcer = DefaultSandboxEnforcer::new();
        let mut profile = SandboxProfile::new("restricted-mounts");
        profile.isolation = IsolationLevel::Container;
        profile.allowed_paths = vec!["/data".into()];
        let profile_check = profile.clone();
        enforcer.register_profile(profile);

        let binding = ToolBinding::Container {
            image: "app".into(),
            command: vec!["run".into()],
            mounts: vec![ContainerMount {
                source: "/etc".into(),
                target: "/mnt/etc".into(),
                read_only: true,
            }],
            network: ContainerNetwork::None,
            pull_policy: PullPolicy::IfNotPresent,
            resource_limits: ContainerResources::default(),
        };
        let result = enforcer.check_permissions(&binding, &profile_check).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_validate_plan_profile_not_found_fails() {
        let enforcer = DefaultSandboxEnforcer::new();
        let plan = test_plan(
            "missing-profile",
            ToolBinding::Subprocess {
                binary: "/bin/true".into(),
                args: vec![],
                env: HashMap::new(),
                working_dir: None,
                allowed_paths: vec![],
                denied_binaries: vec![],
            },
            IsolationLevel::Process,
        );
        let result = enforcer.validate(&plan).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_in_memory_profile_store_remove() {
        let store = InMemoryProfileStore::new();
        let mut profile = SandboxProfile::new("temp");
        profile.isolation = IsolationLevel::Wasm;
        store.upsert(profile).unwrap();
        assert!(store.get("temp").is_ok());
        store.remove("temp").unwrap();
        assert!(store.get("temp").is_err());
    }
}
