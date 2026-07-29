use async_trait::async_trait;
use execution_core::*;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::debug;

const COMMON_CLIS: &[(&str, &[&str])] = &[
    ("cargo", &["cargo", "--version"]),
    ("docker", &["docker", "--version"]),
    ("podman", &["podman", "--version"]),
    ("git", &["git", "--version"]),
    ("npm", &["npm", "--version"]),
    ("node", &["node", "--version"]),
    ("python3", &["python3", "--version"]),
    ("python", &["python", "--version"]),
    ("rustc", &["rustc", "--version"]),
    ("dotnet", &["dotnet", "--version"]),
    ("mvn", &["mvn", "--version"]),
    ("gradle", &["gradle", "--version"]),
    ("go", &["go", "version"]),
    ("deno", &["deno", "--version"]),
    ("bun", &["bun", "--version"]),
    ("curl", &["curl", "--version"]),
    ("wget", &["wget", "--version"]),
    ("ssh", &["ssh", "-V"]),
    ("chrome", &["google-chrome", "--version"]),
    ("chromium", &["chromium", "--version"]),
    ("brave", &["brave-browser", "--version"]),
    ("firefox", &["firefox", "--version"]),
    ("code", &["code", "--version"]),
    ("vim", &["vim", "--version"]),
    ("nvim", &["nvim", "--version"]),
    ("aws", &["aws", "--version"]),
    ("gcloud", &["gcloud", "--version"]),
    ("az", &["az", "--version"]),
    ("kubectl", &["kubectl", "--version"]),
    ("helm", &["helm", "--version"]),
    ("terraform", &["terraform", "--version"]),
    ("ansible", &["ansible", "--version"]),
    ("pwsh", &["pwsh", "--version"]),
    ("wasmtime", &["wasmtime", "--version"]),
    ("wasmer", &["wasmer", "--version"]),
    ("blender", &["blender", "--version"]),
    ("ffmpeg", &["ffmpeg", "-version"]),
    ("imagemagick", &["convert", "--version"]),
    ("sqlite3", &["sqlite3", "--version"]),
    ("psql", &["psql", "--version"]),
    ("mysql", &["mysql", "--version"]),
    ("redis-cli", &["redis-cli", "--version"]),
    ("mongosh", &["mongosh", "--version"]),
];

const COMMON_RUNTIMES: &[(&str, &[&str])] = &[
    ("node", &["node", "--version"]),
    ("python3", &["python3", "--version"]),
    ("python", &["python", "--version"]),
    ("deno", &["deno", "--version"]),
    ("bun", &["bun", "--version"]),
    ("rustc", &["rustc", "--version"]),
    ("go", &["go", "version"]),
    ("dotnet", &["dotnet", "--version"]),
    ("java", &["java", "-version"]),
    ("ruby", &["ruby", "--version"]),
    ("perl", &["perl", "--version"]),
    ("php", &["php", "--version"]),
    ("wasmtime", &["wasmtime", "--version"]),
];

#[derive(Debug)]
pub struct DefaultLocalDiscoverer {
    installed_cache: RwLock<HashMap<String, ProviderMetadata>>,
    runtime_cache: RwLock<HashMap<String, ProviderMetadata>>,
}

impl DefaultLocalDiscoverer {
    pub fn new() -> Self {
        Self {
            installed_cache: RwLock::new(HashMap::new()),
            runtime_cache: RwLock::new(HashMap::new()),
        }
    }

    async fn check_command(cmd: &str, args: &[&str]) -> Option<String> {
        let result = tokio::process::Command::new(cmd).args(args).output().await;
        match result {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_string();
                let version = if version.is_empty() {
                    String::from_utf8_lossy(&output.stderr)
                        .lines()
                        .next()
                        .unwrap_or("")
                        .to_string()
                } else {
                    version
                };
                Some(if version.is_empty() {
                    "installed".into()
                } else {
                    version
                })
            }
            _ => None,
        }
    }

    async fn build_provider_map(&self) -> HashMap<String, ProviderMetadata> {
        let mut cache = self.installed_cache.write().await;
        if !cache.is_empty() {
            return cache.clone();
        }
        for (name, args) in COMMON_CLIS {
            if let Some(version) = Self::check_command(args[0], &args[1..]).await {
                cache.insert(
                    name.to_string(),
                    ProviderMetadata::new(*name, "cli", args[0]),
                );
                let name_ref: &str = name;
                if let Some(entry) = cache.get_mut(name_ref) {
                    entry.version = Some(version);
                    entry.installed = true;
                }
            }
        }
        cache.clone()
    }

    async fn build_runtime_map(&self) -> HashMap<String, ProviderMetadata> {
        let mut cache = self.runtime_cache.write().await;
        if !cache.is_empty() {
            return cache.clone();
        }
        for (name, args) in COMMON_RUNTIMES {
            if let Some(version) = Self::check_command(args[0], &args[1..]).await {
                cache.insert(
                    name.to_string(),
                    ProviderMetadata::new(*name, "runtime", args[0]),
                );
                let name_ref: &str = name;
                if let Some(entry) = cache.get_mut(name_ref) {
                    entry.version = Some(version);
                    entry.installed = true;
                }
            }
        }
        cache.clone()
    }
}

impl Default for DefaultLocalDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapabilityDiscoverer for DefaultLocalDiscoverer {
    async fn discover_local(
        &self,
        query: &DiscoveryQuery,
    ) -> execution_core::ExecutionResult<Vec<DiscoveredCapability>> {
        let providers = self.build_provider_map().await;
        let runtimes = self.build_runtime_map().await;
        let mut results = Vec::new();

        let lower_name = query.capability_name.to_lowercase();
        let lower_id = query.capability_id.to_lowercase();

        for (name, provider) in &providers {
            if lower_name.contains(name)
                || lower_id.contains(name)
                || query.keywords.iter().any(|k| k.contains(name))
            {
                let trust = TrustScore::new(CapabilityOrigin::DiscoveredLocal);
                let cap = DiscoveredCapability {
                    id: format!("{}::{}", query.capability_id, name),
                    name: provider.name.clone(),
                    description: format!("Local CLI provider: {}", provider.name),
                    origin: CapabilityOrigin::DiscoveredLocal,
                    provider: provider.clone(),
                    trust,
                    input_schema: HashMap::new(),
                    output_schema: HashMap::new(),
                    required_permissions: vec!["subprocess".into(), "exec".into()],
                    supported_parameters: HashMap::new(),
                    adapter_template: Some(AdapterTemplate {
                        adapter_type: AdapterType::CliWrapper,
                        template: format!("{{{{ runner }}}} {} {{{{ args }}}}", provider.name),
                        parameters: HashMap::new(),
                        required_runtime: None,
                        sandbox_profile: "default".into(),
                    }),
                    compatible: true,
                };
                results.push(cap);
            }
        }

        for (name, provider) in &runtimes {
            if lower_name.contains(name)
                || lower_id.contains(name)
                || query.keywords.iter().any(|k| k.contains(name))
            {
                let trust = TrustScore::new(CapabilityOrigin::DiscoveredLocal);
                let cap = DiscoveredCapability {
                    id: format!("{}::runtime::{}", query.capability_id, name),
                    name: format!("runtime-{}", provider.name),
                    description: format!("Local runtime provider: {}", provider.name),
                    origin: CapabilityOrigin::DiscoveredLocal,
                    provider: provider.clone(),
                    trust,
                    input_schema: HashMap::new(),
                    output_schema: HashMap::new(),
                    required_permissions: vec!["subprocess".into(), "exec".into()],
                    supported_parameters: HashMap::new(),
                    adapter_template: None,
                    compatible: true,
                };
                results.push(cap);
            }
        }

        debug!(count = results.len(), capability = %query.capability_id, "local discovery results");
        Ok(results)
    }

    async fn discover_external(
        &self,
        _query: &DiscoveryQuery,
    ) -> execution_core::ExecutionResult<Vec<DiscoveredCapability>> {
        Ok(Vec::new())
    }

    async fn check_installed(&self, provider_name: &str) -> execution_core::ExecutionResult<bool> {
        let providers = self.build_provider_map().await;
        Ok(providers.contains_key(provider_name))
    }

    async fn discover_installed_clis(
        &self,
    ) -> execution_core::ExecutionResult<Vec<ProviderMetadata>> {
        let providers = self.build_provider_map().await;
        Ok(providers.into_values().collect())
    }

    async fn discover_installed_runtimes(
        &self,
    ) -> execution_core::ExecutionResult<Vec<ProviderMetadata>> {
        let runtimes = self.build_runtime_map().await;
        Ok(runtimes.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_local_returns_results() {
        let discoverer = DefaultLocalDiscoverer::new();
        let query = DiscoveryQuery {
            capability_id: "build.project".into(),
            capability_name: "Build Project".into(),
            keywords: vec!["cargo".into(), "build".into()],
            categories: Vec::new(),
        };
        let results = discoverer.discover_local(&query).await.unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_check_command_exists() {
        let result = DefaultLocalDiscoverer::check_command("echo", &["hello"]).await;
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_check_command_not_exists() {
        let result = DefaultLocalDiscoverer::check_command("nonexistent_command_xyz", &[]).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_discover_installed_clis() {
        let discoverer = DefaultLocalDiscoverer::new();
        let clis = discoverer.discover_installed_clis().await.unwrap();
        assert!(!clis.is_empty());
    }

    #[tokio::test]
    async fn test_check_installed() {
        let discoverer = DefaultLocalDiscoverer::new();
        let echo_installed = discoverer.check_installed("echo").await.unwrap_or(false);
        assert!(!echo_installed);
    }
}
