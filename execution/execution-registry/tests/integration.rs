use execution_core::{ExecutionCapability, ToolBinding, ToolRegistry};
use execution_registry::InMemoryToolRegistry;
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
async fn test_register_subprocess_and_resolve_by_capability() {
    let registry = InMemoryToolRegistry::new();
    let binding = make_subprocess_binding("/usr/bin/grep");
    let caps = vec![make_capability("text.search", "Text Search")];

    registry.register(binding.clone(), caps).await.unwrap();

    let results = registry.resolve("text.search").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], binding);
}

#[tokio::test]
async fn test_multiple_tools_same_capability() {
    let registry = InMemoryToolRegistry::new();
    let grep = make_subprocess_binding("/usr/bin/grep");
    let awk = make_subprocess_binding("/usr/bin/awk");
    let caps = vec![make_capability("text.search", "Text Search")];

    registry.register(grep.clone(), caps.clone()).await.unwrap();
    registry.register(awk.clone(), caps).await.unwrap();

    let results = registry.resolve("text.search").await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_unregister_then_resolve_empty() {
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
async fn test_capability_index_register_unregister() {
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
