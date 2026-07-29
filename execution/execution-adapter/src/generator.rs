use async_trait::async_trait;
use execution_core::*;
use std::collections::HashMap;
use tracing::debug;

#[derive(Debug)]
pub struct DefaultAdapterGenerator;

impl DefaultAdapterGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DefaultAdapterGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultAdapterGenerator {
    fn render_cli_wrapper(request: &CapabilitySynthesisRequest) -> String {
        let name = &request.provider.name;
        format!(
            r#"#!/usr/bin/env bash
# Auto-generated CLI wrapper for {}
# Capability: {}
# Description: {}

exec {} "$@"
"#,
            name, request.capability_id, request.description, name
        )
    }

    fn render_python_script(request: &CapabilitySynthesisRequest) -> String {
        let name = &request.provider.name;
        let cap_id = &request.capability_id;
        format!(
            r#"#!/usr/bin/env python3
"""Auto-generated adapter for {name}
Capability: {cap_id}
Description: {desc}
"""
import subprocess
import sys
import json

def main():
    result = subprocess.run(
        ["{name}"] + sys.argv[1:],
        capture_output=True,
        text=True,
    )
    print(json.dumps({{
        "exit_code": result.returncode,
        "stdout": result.stdout,
        "stderr": result.stderr,
    }}))

if __name__ == "__main__":
    main()
"#,
            name = name,
            cap_id = cap_id,
            desc = request.description
        )
    }

    fn render_bash_script(request: &CapabilitySynthesisRequest) -> String {
        let name = &request.provider.name;
        format!(
            r#"#!/usr/bin/env bash
# Auto-generated bash adapter for {name}
# Capability: {cap_id}
set -euo pipefail
{name} "$@"
"#,
            name = name,
            cap_id = request.capability_id
        )
    }

    fn render_javascript(request: &CapabilitySynthesisRequest) -> String {
        let name = &request.provider.name;
        format!(
            r#"#!/usr/bin/env node
// Auto-generated JS adapter for {name}
const {{ execSync }} = require('child_process');
const result = execSync('{name} ' + process.argv.slice(2).join(' '), {{
    encoding: 'utf-8',
    stdio: ['inherit', 'pipe', 'pipe'],
}});
process.stdout.write(result);
"#,
            name = name
        )
    }

    fn render_rest_client(request: &CapabilitySynthesisRequest) -> String {
        let name = &request.provider.name;
        format!(
            r#"#!/usr/bin/env bash
# Auto-generated REST client for {name}
# Capability: {cap_id}
BASE_URL="${{{name}_API_URL:-https://api.{name}.com}}"
AUTH_TOKEN="${{{name}_API_TOKEN:-}}"

curl -s -H "Authorization: Bearer $AUTH_TOKEN" "$BASE_URL${{1:-/}}" 2>/dev/null || {{
    echo '{{"error":"REST request failed"}}'
    exit 1
}}
"#,
            name = name,
            cap_id = request.capability_id
        )
    }
}

#[async_trait]
impl AdapterGenerator for DefaultAdapterGenerator {
    async fn can_synthesize(&self, capability: &DiscoveredCapability) -> bool {
        capability.compatible && capability.trust.score >= 0.3
    }

    async fn generate(
        &self,
        request: &CapabilitySynthesisRequest,
    ) -> execution_core::ExecutionResult<AdapterTemplate> {
        let adapter_type = request
            .preferred_adapter_type
            .unwrap_or(AdapterType::CliWrapper);

        match adapter_type {
            AdapterType::CliWrapper => self.generate_cli_wrapper(request).await,
            AdapterType::RestClient => self.generate_rest_client(request).await,
            AdapterType::PythonScript => {
                self.generate_script(request, AdapterType::PythonScript)
                    .await
            }
            AdapterType::BashScript => self.generate_script(request, AdapterType::BashScript).await,
            AdapterType::JavaScript => self.generate_script(request, AdapterType::JavaScript).await,
            AdapterType::PowerShell => self.generate_script(request, AdapterType::PowerShell).await,
            _ => Err(execution_core::ExecutionError::UnsupportedAdapterType(
                adapter_type.to_string(),
            )),
        }
    }

    async fn generate_cli_wrapper(
        &self,
        request: &CapabilitySynthesisRequest,
    ) -> execution_core::ExecutionResult<AdapterTemplate> {
        let template = Self::render_cli_wrapper(request);
        debug!(name = %request.provider.name, "generated CLI wrapper adapter");
        Ok(AdapterTemplate {
            adapter_type: AdapterType::CliWrapper,
            template,
            parameters: HashMap::new(),
            required_runtime: None,
            sandbox_profile: "default".into(),
        })
    }

    async fn generate_rest_client(
        &self,
        request: &CapabilitySynthesisRequest,
    ) -> execution_core::ExecutionResult<AdapterTemplate> {
        let template = Self::render_rest_client(request);
        debug!(name = %request.provider.name, "generated REST client adapter");
        Ok(AdapterTemplate {
            adapter_type: AdapterType::RestClient,
            template,
            parameters: HashMap::new(),
            required_runtime: Some("curl".into()),
            sandbox_profile: "default".into(),
        })
    }

    async fn generate_script(
        &self,
        request: &CapabilitySynthesisRequest,
        language: AdapterType,
    ) -> execution_core::ExecutionResult<AdapterTemplate> {
        let (template, runtime) = match language {
            AdapterType::PythonScript => (Self::render_python_script(request), Some("python3")),
            AdapterType::BashScript => (Self::render_bash_script(request), Some("bash")),
            AdapterType::JavaScript => (Self::render_javascript(request), Some("node")),
            AdapterType::PowerShell => {
                let name = &request.provider.name;
                (
                    format!(
                        r#"# Auto-generated PowerShell adapter for {name}
& {name} @args
"#,
                        name = name
                    ),
                    Some("pwsh"),
                )
            }
            _ => {
                return Err(execution_core::ExecutionError::UnsupportedAdapterType(
                    language.to_string(),
                ))
            }
        };

        debug!(?language, name = %request.provider.name, "generated script adapter");
        Ok(AdapterTemplate {
            adapter_type: language,
            template,
            parameters: HashMap::new(),
            required_runtime: runtime.map(|s| s.to_string()),
            sandbox_profile: "default".into(),
        })
    }

    fn supported_adapter_types(&self) -> Vec<AdapterType> {
        vec![
            AdapterType::CliWrapper,
            AdapterType::RestClient,
            AdapterType::PythonScript,
            AdapterType::BashScript,
            AdapterType::JavaScript,
            AdapterType::PowerShell,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_request() -> CapabilitySynthesisRequest {
        CapabilitySynthesisRequest {
            capability_id: "test.capability".into(),
            capability_name: "Test Capability".into(),
            description: "A test capability".into(),
            provider: ProviderMetadata::new("test-cli", "cli", "/usr/bin/test-cli"),
            input_schema: HashMap::new(),
            output_schema: HashMap::new(),
            preferred_adapter_type: None,
        }
    }

    #[tokio::test]
    async fn test_generate_cli_wrapper() {
        let generator = DefaultAdapterGenerator::new();
        let request = make_request();
        let adapter = generator.generate_cli_wrapper(&request).await.unwrap();
        assert_eq!(adapter.adapter_type, AdapterType::CliWrapper);
        assert!(adapter.template.contains("test-cli"));
    }

    #[tokio::test]
    async fn test_generate_python_script() {
        let generator = DefaultAdapterGenerator::new();
        let request = make_request();
        let adapter = generator
            .generate_script(&request, AdapterType::PythonScript)
            .await
            .unwrap();
        assert_eq!(adapter.adapter_type, AdapterType::PythonScript);
        assert!(adapter.template.contains("python3"));
        assert!(adapter.template.contains("test-cli"));
    }

    #[tokio::test]
    async fn test_generate_bash_script() {
        let generator = DefaultAdapterGenerator::new();
        let request = make_request();
        let adapter = generator
            .generate_script(&request, AdapterType::BashScript)
            .await
            .unwrap();
        assert_eq!(adapter.adapter_type, AdapterType::BashScript);
        assert!(adapter.template.contains("bash"));
    }

    #[tokio::test]
    async fn test_can_synthesize() {
        let generator = DefaultAdapterGenerator::new();
        let trust = TrustScore::new(CapabilityOrigin::DiscoveredLocal);
        let cap = DiscoveredCapability {
            id: "test".into(),
            name: "test".into(),
            description: "test".into(),
            origin: CapabilityOrigin::DiscoveredLocal,
            provider: ProviderMetadata::new("t", "cli", "/t"),
            trust,
            input_schema: HashMap::new(),
            output_schema: HashMap::new(),
            required_permissions: Vec::new(),
            supported_parameters: HashMap::new(),
            adapter_template: None,
            compatible: true,
        };
        assert!(generator.can_synthesize(&cap).await);
    }

    #[tokio::test]
    async fn test_supported_adapter_types() {
        let generator = DefaultAdapterGenerator::new();
        let types = generator.supported_adapter_types();
        assert!(types.contains(&AdapterType::CliWrapper));
        assert!(types.contains(&AdapterType::PythonScript));
        assert!(types.contains(&AdapterType::BashScript));
    }
}
