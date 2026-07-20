use async_trait::async_trait;
use execution_core::{ExecutionResult, ToolBinding, ToolRegistry, ToolResolver};
use std::fmt::Debug;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct DefaultToolResolver {
    registry: Arc<dyn ToolRegistry>,
}

impl DefaultToolResolver {
    pub fn new(registry: Arc<dyn ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolResolver for DefaultToolResolver {
    async fn resolve_capability(&self, capability_id: &str) -> ExecutionResult<ToolBinding> {
        let bindings = self.registry.resolve(capability_id).await?;
        bindings.into_iter().next().ok_or_else(|| {
            execution_core::ExecutionError::NoSuitableCandidate {
                capability: capability_id.into(),
                detail: "no registered tool satisfies this capability".into(),
            }
        })
    }

    async fn resolve_with_fallback(
        &self,
        capability_id: &str,
        preferred_backends: &[String],
    ) -> ExecutionResult<ToolBinding> {
        let bindings = self.registry.resolve(capability_id).await?;

        for backend in preferred_backends {
            for binding in &bindings {
                if binding_backend_matches(binding, backend) {
                    return Ok(binding.clone());
                }
            }
        }

        bindings.into_iter().next().ok_or_else(|| {
            execution_core::ExecutionError::NoSuitableCandidate {
                capability: capability_id.into(),
                detail: "no tool satisfies capability after fallback".into(),
            }
        })
    }

    async fn check_capability(
        &self,
        capability_id: &str,
        tool: &execution_core::ToolBinding,
    ) -> bool {
        let result = self.registry.resolve(capability_id).await;
        match result {
            Ok(bindings) => bindings.contains(tool),
            Err(_) => false,
        }
    }
}

fn binding_backend_matches(binding: &execution_core::ToolBinding, backend: &str) -> bool {
    match binding {
        execution_core::ToolBinding::Subprocess { .. } => backend == "subprocess",
        execution_core::ToolBinding::Wasm { .. } => backend == "wasm",
        execution_core::ToolBinding::Container { .. } => backend == "container",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_core::{ExecutionCapability, ToolBinding};
    use std::collections::HashMap;
    use std::sync::Arc;

    fn make_subprocess_binding(binary: &str) -> ToolBinding {
        ToolBinding::Subprocess {
            binary: binary.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            allowed_paths: Vec::new(),
            denied_binaries: Vec::new(),
        }
    }

    fn make_capability(id: &str, name: &str) -> ExecutionCapability {
        ExecutionCapability {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            input_schema: HashMap::new(),
            output_schema: HashMap::new(),
            required_permissions: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_resolve_capability_finds_first() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let grep = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];
        registry.register(grep.clone(), caps).await.unwrap();

        let resolver = super::DefaultToolResolver::new(registry);
        let binding = resolver.resolve_capability("text.search").await.unwrap();
        assert_eq!(binding, grep);
    }

    #[tokio::test]
    async fn test_resolve_capability_none_found() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let resolver = super::DefaultToolResolver::new(registry);
        let result = resolver.resolve_capability("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_with_fallback_prefers_backend() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let sub = make_subprocess_binding("/usr/bin/grep");
        let wasm = ToolBinding::Wasm {
            module_path: "/modules/grep.wasm".into(),
            function_name: "search".into(),
            input_format: execution_core::WasmInputFormat::Json,
            fuel_limit: None,
            precompile: false,
        };
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(sub.clone(), caps.clone()).await.unwrap();
        registry.register(wasm.clone(), caps).await.unwrap();

        let resolver = super::DefaultToolResolver::new(registry);
        let preferred = vec!["wasm".into()];
        let result = resolver
            .resolve_with_fallback("text.search", &preferred)
            .await
            .unwrap();
        assert_eq!(result, wasm);
    }

    #[tokio::test]
    async fn test_resolve_with_fallback_falls_back() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let sub = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];
        registry.register(sub.clone(), caps).await.unwrap();

        let resolver = super::DefaultToolResolver::new(registry);
        let preferred = vec!["container".into()];
        let result = resolver
            .resolve_with_fallback("text.search", &preferred)
            .await
            .unwrap();
        assert_eq!(result, sub);
    }

    #[tokio::test]
    async fn test_resolve_with_fallback_no_match() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let resolver = super::DefaultToolResolver::new(registry);
        let result = resolver.resolve_with_fallback("nonexistent", &[]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_check_capability() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];
        registry.register(binding.clone(), caps).await.unwrap();

        let resolver = super::DefaultToolResolver::new(registry);
        assert!(resolver.check_capability("text.search", &binding).await);
    }

    #[tokio::test]
    async fn test_check_capability_false() {
        let registry = Arc::new(crate::InMemoryToolRegistry::new());
        let binding = make_subprocess_binding("/usr/bin/grep");
        let resolver = super::DefaultToolResolver::new(registry);
        assert!(!resolver.check_capability("text.search", &binding).await);
    }
}
