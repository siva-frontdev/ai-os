use async_trait::async_trait;
use execution_core::*;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug)]
pub struct DefaultExternalDiscoverer;

impl DefaultExternalDiscoverer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultExternalDiscoverer {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapabilityDiscoverer for DefaultExternalDiscoverer {
    async fn discover_local(
        &self,
        _query: &DiscoveryQuery,
    ) -> execution_core::ExecutionResult<Vec<DiscoveredCapability>> {
        Ok(Vec::new())
    }

    async fn discover_external(
        &self,
        query: &DiscoveryQuery,
    ) -> execution_core::ExecutionResult<Vec<DiscoveredCapability>> {
        let mut results = Vec::new();
        let lower_name = query.capability_name.to_lowercase();
        let lower_id = query.capability_id.to_lowercase();

        let known_official_providers = [
            ("cargo", "rust-lang.org", "https://doc.rust-lang.org/cargo/"),
            (
                "docker",
                "docker.com",
                "https://docs.docker.com/engine/reference/commandline/cli/",
            ),
            ("npm", "npmjs.com", "https://docs.npmjs.com/cli/"),
            (
                "python",
                "python.org",
                "https://docs.python.org/3/using/cmdline.html",
            ),
            ("git", "git-scm.com", "https://git-scm.com/docs/git"),
            ("node", "nodejs.org", "https://nodejs.org/api/cli.html"),
            (
                "aws",
                "aws.amazon.com",
                "https://awscli.amazonaws.com/v2/documentation/api/latest/index.html",
            ),
            (
                "gcloud",
                "cloud.google.com",
                "https://cloud.google.com/sdk/docs",
            ),
            (
                "az",
                "azure.microsoft.com",
                "https://learn.microsoft.com/en-us/cli/azure/",
            ),
            (
                "kubectl",
                "kubernetes.io",
                "https://kubernetes.io/docs/reference/kubectl/",
            ),
            (
                "terraform",
                "hashicorp.com",
                "https://developer.hashicorp.com/terraform/cli",
            ),
            (
                "ansible",
                "ansible.com",
                "https://docs.ansible.com/ansible/latest/cli/",
            ),
            (
                "dotnet",
                "microsoft.com",
                "https://learn.microsoft.com/en-us/dotnet/core/tools/",
            ),
            (
                "deno",
                "deno.com",
                "https://deno.land/manual@v1.35.0/getting_started/command_line_interface",
            ),
        ];

        for (name, publisher, docs_url) in &known_official_providers {
            if lower_name.contains(name)
                || lower_id.contains(name)
                || query.keywords.iter().any(|k| k.contains(name))
            {
                let mut provider =
                    ProviderMetadata::new(*name, "official_cli", docs_url.to_string());
                provider.installed = false;
                provider.available = true;

                let trust = TrustScore {
                    score: 0.7,
                    origin: CapabilityOrigin::DiscoveredExternal,
                    verification_status: VerificationStatus::Verified,
                    execution_count: 0,
                    success_rate: 0.9,
                    failure_modes: Vec::new(),
                    last_verified: None,
                };

                let cap = DiscoveredCapability {
                    id: format!("{}::official::{}", query.capability_id, name),
                    name: format!("official-{}", name),
                    description: format!("Official {} CLI from {}", name, publisher),
                    origin: CapabilityOrigin::DiscoveredExternal,
                    provider,
                    trust,
                    input_schema: HashMap::new(),
                    output_schema: HashMap::new(),
                    required_permissions: vec!["subprocess".into(), "network".into()],
                    supported_parameters: HashMap::new(),
                    adapter_template: Some(AdapterTemplate {
                        adapter_type: AdapterType::CliWrapper,
                        template: format!("{{{{ runner }}}} {} {{{{ args }}}}", name),
                        parameters: HashMap::new(),
                        required_runtime: None,
                        sandbox_profile: "default".into(),
                    }),
                    compatible: true,
                };
                results.push(cap);
            }
        }

        debug!(count = results.len(), capability = %query.capability_id, "external discovery results");
        Ok(results)
    }

    async fn check_installed(&self, _provider_name: &str) -> execution_core::ExecutionResult<bool> {
        Ok(false)
    }

    async fn discover_installed_clis(
        &self,
    ) -> execution_core::ExecutionResult<Vec<ProviderMetadata>> {
        Ok(Vec::new())
    }

    async fn discover_installed_runtimes(
        &self,
    ) -> execution_core::ExecutionResult<Vec<ProviderMetadata>> {
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_external_returns_results() {
        let discoverer = DefaultExternalDiscoverer::new();
        let query = DiscoveryQuery {
            capability_id: "build.project".into(),
            capability_name: "Build Project".into(),
            keywords: vec!["cargo".into(), "rust".into()],
            categories: Vec::new(),
        };
        let results = discoverer.discover_external(&query).await.unwrap();
        assert!(!results.is_empty());
        assert!(results.iter().any(|c| c.provider.name == "cargo"));
    }

    #[tokio::test]
    async fn test_discover_external_unrelated_returns_empty() {
        let discoverer = DefaultExternalDiscoverer::new();
        let query = DiscoveryQuery {
            capability_id: "unknown.xyz".into(),
            capability_name: "XyZzz".into(),
            keywords: vec!["nonexistent".into()],
            categories: Vec::new(),
        };
        let results = discoverer.discover_external(&query).await.unwrap();
        assert!(results.is_empty());
    }
}
