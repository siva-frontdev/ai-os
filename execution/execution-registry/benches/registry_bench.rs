use criterion::{criterion_group, criterion_main, Criterion};
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

fn bench_register(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("register_tool", |b| {
        b.to_async(&rt).iter(|| async {
            let registry = InMemoryToolRegistry::new();
            let binding = make_subprocess_binding("/usr/bin/grep");
            let caps = vec![make_capability("text.search", "Text Search")];
            registry.register(binding, caps).await.unwrap();
        });
    });
}

fn bench_resolve(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("resolve_capability", |b| {
        b.to_async(&rt).iter(|| async {
            let registry = InMemoryToolRegistry::new();
            let binding = make_subprocess_binding("/usr/bin/grep");
            let caps = vec![make_capability("text.search", "Text Search")];
            registry.register(binding, caps).await.unwrap();
            let _ = registry.resolve("text.search").await.unwrap();
        });
    });
}

criterion_group!(benches, bench_register, bench_resolve);
criterion_main!(benches);
