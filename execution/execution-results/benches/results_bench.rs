use criterion::{criterion_group, criterion_main, Criterion};
use execution_core::{
    types::ExecutionResult as ExecutionResultStruct, ExecutionMetrics, ExecutionState,
    OutputParser, OutputRouter, ResultCollector, RouteDestination, RunnerOutput,
};
use execution_results::{DefaultOutputParser, DefaultOutputRouter, DefaultResultCollector};
use memory_core::Timestamp;
use tokio::runtime::Runtime;

fn make_output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> RunnerOutput {
    RunnerOutput {
        execution_id: execution_core::ExecutionId::new(),
        exit_code,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        metrics: ExecutionMetrics::default(),
        artifacts: Vec::new(),
    }
}

fn make_result(state: ExecutionState) -> ExecutionResultStruct {
    ExecutionResultStruct {
        execution_id: execution_core::ExecutionId::new(),
        state,
        exit_code: Some(0),
        stdout: Vec::new(),
        stderr: Vec::new(),
        parsed_output: None,
        artifacts: Vec::new(),
        metrics: ExecutionMetrics::default(),
        error: None,
        routing_results: Vec::new(),
        started_at: Timestamp::now(),
        completed_at: Timestamp::now(),
    }
}

fn bench_json_parse(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let parser = DefaultOutputParser::default();
    let json_data = br#"{"key": "value", "number": 42, "nested": {"a": [1,2,3]}}"#;

    c.bench_function("json_parse", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = parser.parse(json_data).await;
        })
    });
}

fn bench_raw_parse(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let parser = DefaultOutputParser::default();
    let text_data = b"The quick brown fox jumps over the lazy dog. This is a longer text.";

    c.bench_function("raw_parse", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = parser.parse(text_data).await;
        })
    });
}

fn bench_collect_output(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(DefaultOutputParser::default())];
    let collector = DefaultResultCollector::new(parsers, 1024 * 1024);
    let output = make_output(0, b"stdout content for benchmark", b"");

    c.bench_function("collect_output", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = collector.collect(output.clone()).await;
        })
    });
}

fn bench_route_to_caller(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let router = DefaultOutputRouter::new();
    let result = make_result(ExecutionState::Completed);

    c.bench_function("route_to_caller", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = router.route_to(&result, &RouteDestination::Caller).await;
        })
    });
}

fn bench_route_with_rules(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let router = DefaultOutputRouter::new();
    router.add_rule(execution_core::RoutingRule {
        condition: "Completed".into(),
        destinations: vec![RouteDestination::Caller, RouteDestination::Log],
    });
    let result = make_result(ExecutionState::Completed);

    c.bench_function("route_with_rules", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = router.route(&result).await;
        })
    });
}

fn bench_store_artifact(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
    let collector = DefaultResultCollector::new(parsers, 1024);
    let id = execution_core::ExecutionId::new();
    let artifact = execution_core::ExecutionArtifact {
        name: "benchmark.txt".into(),
        mime_type: "text/plain".into(),
        size_bytes: 13,
        data: b"benchmark data".to_vec(),
    };

    c.bench_function("store_artifact", |b| {
        b.to_async(&rt).iter(|| async {
            let _ = collector.store_artifact(&id, artifact.clone()).await;
        })
    });
}

criterion_group!(
    name = results_bench;
    config = Criterion::default().sample_size(100);
    targets = bench_json_parse, bench_raw_parse, bench_collect_output, bench_route_to_caller, bench_route_with_rules, bench_store_artifact
);
criterion_main!(results_bench);
