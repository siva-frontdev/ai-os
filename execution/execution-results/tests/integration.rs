use execution_core::{
    types::ExecutionResult as ExecutionResultStruct, ExecutionArtifact, ExecutionId,
    ExecutionMetrics, ExecutionState, OutputParser, OutputRouter, ResultCollector,
    RouteDestination, RoutingRule,
};
use execution_results::{
    DefaultOutputParser, DefaultOutputRouter, DefaultResultCollector, JsonParser, RawParser,
};
use memory_core::Timestamp;

fn make_output(exit_code: i32, stdout: &[u8], stderr: &[u8]) -> execution_core::RunnerOutput {
    execution_core::RunnerOutput {
        execution_id: ExecutionId::new(),
        exit_code,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
        metrics: ExecutionMetrics::default(),
        artifacts: Vec::new(),
    }
}

fn make_result(state: ExecutionState) -> ExecutionResultStruct {
    ExecutionResultStruct {
        execution_id: ExecutionId::new(),
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

#[tokio::test]
async fn collect_output_from_successful_run() {
    let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(RawParser)];
    let collector = DefaultResultCollector::new(parsers, 1024 * 1024);
    let output = make_output(0, b"stdout content", b"stderr content");

    let result = collector.collect(output).await.unwrap();

    assert_eq!(result.state, ExecutionState::Collecting);
    assert_eq!(result.stdout, b"stdout content");
    assert_eq!(result.stderr, b"stderr content");
    assert_eq!(result.exit_code, Some(0));
}

#[tokio::test]
async fn collect_with_json_parser() {
    let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
    let collector = DefaultResultCollector::new(parsers, 1024 * 1024);
    let output = make_output(0, b"{\"key\": \"value\"}", b"");

    let result = collector
        .collect_with_parse(output, &JsonParser)
        .await
        .unwrap();

    let parsed = result.parsed_output.expect("parsed_output should be set");
    assert_eq!(parsed["key"], "value");
}

#[tokio::test]
async fn collect_with_empty_output() {
    let parsers: Vec<Box<dyn OutputParser>> = vec![Box::new(RawParser)];
    let collector = DefaultResultCollector::new(parsers, 1024);
    let output = make_output(0, b"", b"");

    let result = collector.collect(output).await.unwrap();

    assert!(result.parsed_output.is_some());
    assert_eq!(result.parsed_output.unwrap(), "");
}

#[tokio::test]
async fn route_result_to_caller() {
    let router = DefaultOutputRouter::new();
    let result = make_result(ExecutionState::Completed);

    let route_result = router
        .route_to(&result, &RouteDestination::Caller)
        .await
        .unwrap();

    assert!(route_result.success);
    assert_eq!(route_result.destination, "caller");
    assert!(route_result.error.is_none());
}

#[tokio::test]
async fn route_with_matching_rule() {
    let router = DefaultOutputRouter::new();
    let rule = RoutingRule {
        condition: "Completed".into(),
        destinations: vec![RouteDestination::Log, RouteDestination::Caller],
    };
    router.add_rule(rule);

    let result = make_result(ExecutionState::Completed);
    let results = router.route(&result).await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.success));
}

#[tokio::test]
async fn store_and_retrieve_artifact() {
    let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
    let collector = DefaultResultCollector::new(parsers, 1024);
    let id = ExecutionId::new();

    let artifact = ExecutionArtifact {
        name: "output.txt".into(),
        mime_type: "text/plain".into(),
        size_bytes: 11,
        data: b"hello world".to_vec(),
    };

    collector.store_artifact(&id, artifact).await.unwrap();

    let artifacts = collector.get_artifacts(&id).unwrap();
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].name, "output.txt");
    assert_eq!(artifacts[0].data, b"hello world");
}

#[tokio::test]
async fn truncate_output_when_size_exceeds_max() {
    let parsers: Vec<Box<dyn OutputParser>> = Vec::new();
    let collector = DefaultResultCollector::new(parsers, 5);
    let output = make_output(0, b"hello world, this is long", b"");

    let result = collector.collect(output).await.unwrap();

    assert_eq!(result.stdout.len(), 5);
    assert_eq!(&result.stdout[..], b"hello");
}

#[tokio::test]
async fn default_parser_tries_json_then_raw() {
    let parser = DefaultOutputParser::default();

    let json_result = parser.parse(b"{\"a\": 1}").await.unwrap();
    assert!(json_result.is_some());
    assert_eq!(json_result.unwrap()["a"], 1);

    let raw_result = parser.parse(b"plain text").await.unwrap();
    assert!(raw_result.is_some());
    assert_eq!(raw_result.unwrap(), "plain text");
}

#[tokio::test]
async fn route_to_memory_brain() {
    let router = DefaultOutputRouter::new();
    let result = make_result(ExecutionState::Completed);

    let memory = router
        .route_to(&result, &RouteDestination::Memory)
        .await
        .unwrap();
    assert!(memory.success);

    let brain = router
        .route_to(&result, &RouteDestination::Brain)
        .await
        .unwrap();
    assert!(brain.success);
}
