use async_trait::async_trait;
use execution_core::{ExecutionCapability, ExecutionResult, ToolBinding, ToolRegistry};
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct InMemoryToolRegistry {
    tools: RwLock<HashMap<String, (ToolBinding, Vec<ExecutionCapability>)>>,
    capability_index: RwLock<HashMap<String, Vec<String>>>,
}

impl InMemoryToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            capability_index: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistry for InMemoryToolRegistry {
    async fn register(
        &self,
        binding: ToolBinding,
        capabilities: Vec<ExecutionCapability>,
    ) -> ExecutionResult<()> {
        let tool_id = tool_binding_id(&binding);
        let mut tools = self.tools.write().await;
        if tools.contains_key(&tool_id) {
            return Err(execution_core::ExecutionError::NotFound(format!(
                "tool already registered: {tool_id}"
            )));
        }
        let cap_ids: Vec<String> = capabilities.iter().map(|c| c.id.clone()).collect();
        tools.insert(tool_id.clone(), (binding, capabilities));

        let mut index = self.capability_index.write().await;
        for cap_id in &cap_ids {
            index
                .entry(cap_id.clone())
                .or_insert_with(Vec::new)
                .push(tool_id.clone());
        }

        Ok(())
    }

    async fn unregister(&self, tool_id: &str) -> ExecutionResult<()> {
        let mut tools = self.tools.write().await;
        let (_, capabilities) = tools.remove(tool_id).ok_or_else(|| {
            execution_core::ExecutionError::NotFound(format!("tool not found: {tool_id}"))
        })?;

        let mut index = self.capability_index.write().await;
        for cap in &capabilities {
            if let Some(entries) = index.get_mut(&cap.id) {
                entries.retain(|id| id != tool_id);
                if entries.is_empty() {
                    index.remove(&cap.id);
                }
            }
        }

        Ok(())
    }

    async fn resolve(
        &self,
        capability_id: &str,
    ) -> ExecutionResult<Vec<execution_core::ToolBinding>> {
        let index = self.capability_index.read().await;
        let tool_ids = match index.get(capability_id) {
            Some(ids) => ids.clone(),
            None => return Ok(Vec::new()),
        };
        drop(index);

        let tools = self.tools.read().await;
        let mut result = Vec::new();
        for id in &tool_ids {
            if let Some((binding, _)) = tools.get(id) {
                result.push(binding.clone());
            }
        }
        Ok(result)
    }

    async fn list_capabilities(&self) -> ExecutionResult<Vec<ExecutionCapability>> {
        let tools = self.tools.read().await;
        let mut seen = HashSet::new();
        let mut capabilities = Vec::new();
        for (_, caps) in tools.values() {
            for cap in caps {
                if seen.insert(cap.id.clone()) {
                    capabilities.push(cap.clone());
                }
            }
        }
        Ok(capabilities)
    }

    async fn find_by_name(
        &self,
        name: &str,
    ) -> ExecutionResult<Option<execution_core::ToolBinding>> {
        let tools = self.tools.read().await;
        for (binding, _) in tools.values() {
            if binding_matches_name(binding, name) {
                return Ok(Some(binding.clone()));
            }
        }
        Ok(None)
    }
}

fn tool_binding_id(binding: &execution_core::ToolBinding) -> String {
    match binding {
        execution_core::ToolBinding::Subprocess { binary, .. } => {
            format!("subprocess:{}", binary)
        }
        execution_core::ToolBinding::Wasm {
            module_path,
            function_name,
            ..
        } => format!("wasm:{}::{}", module_path, function_name),
        execution_core::ToolBinding::Container { image, command, .. } => {
            format!("container:{}:{}", image, command.join(" "))
        }
    }
}

fn binding_matches_name(binding: &execution_core::ToolBinding, name: &str) -> bool {
    match binding {
        execution_core::ToolBinding::Subprocess { binary, .. } => binary == name,
        execution_core::ToolBinding::Wasm { function_name, .. } => function_name == name,
        execution_core::ToolBinding::Container { image, .. } => image == name,
    }
}

// ── DefaultToolRegistry ─────────────────────────────────────

#[derive(Debug)]
pub struct DefaultToolRegistry;

#[async_trait]
impl ToolRegistry for DefaultToolRegistry {
    async fn register(
        &self,
        _binding: ToolBinding,
        _capabilities: Vec<ExecutionCapability>,
    ) -> ExecutionResult<()> {
        Err(execution_core::ExecutionError::NotFound(
            "DefaultToolRegistry: register not supported".into(),
        ))
    }

    async fn unregister(&self, _tool_id: &str) -> ExecutionResult<()> {
        Err(execution_core::ExecutionError::NotFound(
            "DefaultToolRegistry: unregister not supported".into(),
        ))
    }

    async fn resolve(
        &self,
        _capability_id: &str,
    ) -> ExecutionResult<Vec<execution_core::ToolBinding>> {
        Ok(Vec::new())
    }

    async fn list_capabilities(&self) -> ExecutionResult<Vec<ExecutionCapability>> {
        Ok(Vec::new())
    }

    async fn find_by_name(
        &self,
        _name: &str,
    ) -> ExecutionResult<Option<execution_core::ToolBinding>> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_core::{ExecutionCapability, ToolBinding};
    use std::collections::HashMap;

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
    async fn test_register_and_resolve() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(binding.clone(), caps).await.unwrap();

        let results = registry.resolve("text.search").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], binding);
    }

    #[tokio::test]
    async fn test_register_duplicate_fails() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry
            .register(binding.clone(), caps.clone())
            .await
            .unwrap();
        let result = registry.register(binding.clone(), caps).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_resolve_multiple_tools_same_capability() {
        let registry = InMemoryToolRegistry::new();
        let grep = make_subprocess_binding("/usr/bin/grep");
        let awk = make_subprocess_binding("/usr/bin/awk");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(grep.clone(), caps.clone()).await.unwrap();
        registry.register(awk.clone(), caps).await.unwrap();

        let results = registry.resolve("text.search").await.unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.contains(&grep));
        assert!(results.contains(&awk));
    }

    #[tokio::test]
    async fn test_unregister_removes_from_index() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(binding, caps).await.unwrap();
        registry
            .unregister("subprocess:/usr/bin/grep")
            .await
            .unwrap();

        let results = registry.resolve("text.search").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_unregister_unknown_fails() {
        let registry = InMemoryToolRegistry::new();
        let result = registry.unregister("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_capabilities() {
        let registry = InMemoryToolRegistry::new();
        let grep = make_subprocess_binding("/usr/bin/grep");
        let awk = make_subprocess_binding("/usr/bin/awk");

        registry
            .register(grep, vec![make_capability("text.search", "Text Search")])
            .await
            .unwrap();
        registry
            .register(awk, vec![make_capability("text.process", "Text Process")])
            .await
            .unwrap();

        let caps = registry.list_capabilities().await.unwrap();
        assert_eq!(caps.len(), 2);
        let ids: Vec<String> = caps.iter().map(|c| c.id.clone()).collect();
        assert!(ids.contains(&"text.search".into()));
        assert!(ids.contains(&"text.process".into()));
    }

    #[tokio::test]
    async fn test_find_by_name() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(binding.clone(), caps).await.unwrap();

        let found = registry
            .find_by_name("/usr/bin/grep")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found, binding);

        let not_found = registry.find_by_name("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_default_tool_registry_returns_errors() {
        let registry = DefaultToolRegistry;
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        assert!(registry.register(binding, caps).await.is_err());
        assert!(registry.unregister("x").await.is_err());
        assert!(registry.resolve("x").await.unwrap().is_empty());
        assert!(registry.list_capabilities().await.unwrap().is_empty());
        assert!(registry.find_by_name("x").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_capability_index_updated_on_register() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(binding, caps).await.unwrap();

        let index = registry.capability_index.read().await;
        let entries = index.get("text.search").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "subprocess:/usr/bin/grep");
    }

    #[tokio::test]
    async fn test_capability_index_updated_on_unregister() {
        let registry = InMemoryToolRegistry::new();
        let binding = make_subprocess_binding("/usr/bin/grep");
        let caps = vec![make_capability("text.search", "Text Search")];

        registry.register(binding, caps).await.unwrap();
        registry
            .unregister("subprocess:/usr/bin/grep")
            .await
            .unwrap();

        let index = registry.capability_index.read().await;
        assert!(index.get("text.search").is_none());
    }

    #[tokio::test]
    async fn test_resolve_unknown_capability_returns_empty() {
        let registry = InMemoryToolRegistry::new();
        let results = registry.resolve("nonexistent").await.unwrap();
        assert!(results.is_empty());
    }
}
