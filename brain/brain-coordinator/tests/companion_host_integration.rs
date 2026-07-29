use std::sync::Arc;
use std::sync::Mutex;

use brain_coordinator::companion_host::sources::ObservationSource;
use brain_coordinator::companion_host::{
    CompanionHost, ObservationConfig, PersistenceManager, UiConfig,
};
use memory_core::wm::{Entity, Relationship};
use memory_storage::wm_store::{InMemoryWorldModelStore, WorldModelStore};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

// ── Mock observation source ─────────────────────────────────

#[derive(Debug, Clone)]
struct MockSource {
    name: &'static str,
    values: Arc<Mutex<Vec<String>>>,
    index: Arc<Mutex<usize>>,
}

impl MockSource {
    fn new(name: &'static str, values: Vec<String>) -> Self {
        Self {
            name,
            values: Arc::new(Mutex::new(values)),
            index: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ObservationSource for MockSource {
    fn name(&self) -> &'static str {
        self.name
    }

    async fn poll(&self) -> Option<String> {
        let mut idx = self.index.lock().unwrap();
        let values = self.values.lock().unwrap();
        if *idx >= values.len() {
            return None;
        }
        let val = values[*idx].clone();
        *idx += 1;
        Some(val)
    }
}

// ── Existing tests ──────────────────────────────────────────

#[tokio::test]
async fn test_save_and_load_cycle() {
    let tmp_dir = std::env::temp_dir().join("observation_test_save_load");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    {
        let store = InMemoryWorldModelStore::new();
        let alice = store.insert_entity(Entity::new("person", "Alice")).await;
        let bob = store.insert_entity(Entity::new("person", "Bob")).await;
        let rel = Relationship::new("knows", alice, bob);
        store.insert_relationship(&rel).await;

        let persistence = PersistenceManager::new(&wm_path);
        let entities = store.all_entities().await;
        let relationships = store.all_relationships().await;
        persistence.save(&entities, &relationships).await.unwrap();
    }

    let host = CompanionHost::load(&wm_path).await.unwrap();
    let entities = host.store().all_entities().await;
    let relationships = host.store().all_relationships().await;

    assert_eq!(entities.len(), 2);
    assert_eq!(relationships.len(), 1);

    let names: Vec<_> = entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Alice"));
    assert!(names.contains(&"Bob"));

    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_cold_start_no_file() {
    let tmp_dir = std::env::temp_dir().join("observation_test_cold_start");
    let wm_path = tmp_dir.join("nonexistent_wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let host = CompanionHost::load(&wm_path).await.unwrap();
    let entities = host.store().all_entities().await;
    assert_eq!(entities.len(), 0);

    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_persistence_manager_path() {
    let path = std::env::temp_dir()
        .join("observation_test_path")
        .join("wm.json");
    let pm = PersistenceManager::new(&path);
    assert_eq!(pm.path(), &path);
}

#[tokio::test]
async fn test_host_start_stop() {
    let tmp_dir = std::env::temp_dir().join("observation_test_start_stop");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let host = CompanionHost::load(&wm_path).await.unwrap();
    assert!(!host.is_running());

    host.start().await.unwrap();
    assert!(host.is_running());

    host.stop().await.unwrap();
    assert!(!host.is_running());

    assert!(wm_path.exists());
    let _ = std::fs::remove_file(&wm_path);
}

// ── Observation loop tests ──────────────────────────────────

#[tokio::test]
async fn test_observation_loop_start_stop() {
    let tmp_dir = std::env::temp_dir().join("observation_test_loop_start_stop");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();
    host.with_default_observation(1);
    host.start().await.unwrap();
    assert!(host.is_running());

    // Let it run for one tick
    tokio::time::sleep(tokio::time::Duration::from_millis(1100)).await;

    host.stop().await.unwrap();
    assert!(!host.is_running());
    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_observation_debug_info() {
    let tmp_dir = std::env::temp_dir().join("observation_test_debug");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();

    // Use a mock source that produces unique observations
    let source = MockSource::new(
        "test_src",
        vec!["first observation".into(), "second observation".into()],
    );
    let config = ObservationConfig {
        interval_secs: 1,
        max_history: 100,
    };
    host.with_observation_loop(config, vec![Box::new(source)]);
    host.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

    host.stop().await.unwrap();

    let debug = host.observation_debug();
    assert!(debug.is_some(), "observation_debug should return Some");
    let info = debug.unwrap();
    assert_eq!(
        info.total_observations, 2,
        "should have 2 unique observations"
    );
    assert!(info.sources.contains(&"test_src".to_string()));

    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_observation_loop_change_detection() {
    let tmp_dir = std::env::temp_dir().join("observation_test_change");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();

    // Source returns the SAME value repeatedly (tests change detection)
    // Uses a cloneable source so that poll() always returns the same value.
    #[derive(Debug, Clone)]
    struct ConstantSource;

    #[async_trait::async_trait]
    impl ObservationSource for ConstantSource {
        fn name(&self) -> &'static str {
            "constant"
        }
        async fn poll(&self) -> Option<String> {
            Some("same value every time".into())
        }
    }

    let source = ConstantSource;
    let config = ObservationConfig {
        interval_secs: 1,
        max_history: 100,
    };
    host.with_observation_loop(config, vec![Box::new(source)]);
    host.start().await.unwrap();

    // Run for ~3 ticks
    tokio::time::sleep(tokio::time::Duration::from_millis(3500)).await;

    host.stop().await.unwrap();

    // After the first tick, remaining ticks should be skipped (same content)
    let debug = host.observation_debug().unwrap();
    assert_eq!(
        debug.total_observations, 1,
        "only 1 unique observation should pass"
    );
    assert!(debug.total_skipped > 0, "duplicates should be skipped");

    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_observation_loop_multiple_sources() {
    let tmp_dir = std::env::temp_dir().join("observation_test_multi");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();

    let src_a = MockSource::new("src_a", vec!["event from a".into()]);
    let src_b = MockSource::new("src_b", vec!["event from b".into()]);
    let config = ObservationConfig {
        interval_secs: 1,
        max_history: 100,
    };
    host.with_observation_loop(config, vec![Box::new(src_a), Box::new(src_b)]);
    host.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(2500)).await;

    host.stop().await.unwrap();

    let debug = host.observation_debug().unwrap();
    // Both sources should have produced an observation
    assert!(
        debug.total_observations >= 2,
        "should observe from both sources"
    );
    assert!(debug.sources.contains(&"src_a".to_string()));
    assert!(debug.sources.contains(&"src_b".to_string()));

    let _ = std::fs::remove_file(&wm_path);
}

// ── UI / Companion Web Interface Tests ──────────────────────

#[tokio::test]
async fn test_ui_config_default_port() {
    let config = UiConfig::default();
    assert_eq!(config.port, 9876, "default port should be 9876");
}

#[tokio::test]
async fn test_ui_api_status() {
    let tmp_dir = std::env::temp_dir().join("companion_test_ui_api");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();
    host.with_ui(UiConfig { port: 19876 });
    host.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:19876")
        .await
        .expect("connect to UI server");
    let request = b"GET /api/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    stream.write_all(request).await.unwrap();

    let (reader, _) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut response = String::new();
    buf_reader.read_to_string(&mut response).await.unwrap();

    assert!(
        response.contains("200 OK"),
        "expected 200 OK, got: {response:.80}"
    );
    assert!(
        response.contains("running"),
        "expected 'running' in JSON body"
    );
    assert!(
        response.contains("entity_count"),
        "expected 'entity_count' in JSON"
    );

    host.stop().await.unwrap();
    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_ui_chat_endpoint() {
    let tmp_dir = std::env::temp_dir().join("companion_test_ui_chat");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();
    host.with_ui(UiConfig { port: 19877 });
    host.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let body = r#"{"message":"hello"}"#;
    let request = format!(
        "POST /chat HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:19877")
        .await
        .expect("connect to UI server");
    stream.write_all(request.as_bytes()).await.unwrap();

    let (reader, _) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut response = String::new();
    buf_reader.read_to_string(&mut response).await.unwrap();

    assert!(
        response.contains("200 OK"),
        "chat API should return 200, got: {response:.80}"
    );
    assert!(
        response.contains("decision"),
        "chat response should contain 'decision', got: {response:.80}"
    );

    host.stop().await.unwrap();
    let _ = std::fs::remove_file(&wm_path);
}

#[tokio::test]
async fn test_ui_sse_endpoint() {
    let tmp_dir = std::env::temp_dir().join("companion_test_ui_sse");
    let wm_path = tmp_dir.join("wm.json");
    let _ = std::fs::remove_file(&wm_path);

    let mut host = CompanionHost::load(&wm_path).await.unwrap();
    host.with_ui(UiConfig { port: 19878 });
    host.start().await.unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    let mut stream = tokio::net::TcpStream::connect("127.0.0.1:19878")
        .await
        .expect("connect to UI server");
    let request = b"GET /events HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n";
    stream.write_all(request).await.unwrap();

    // Read only the first line to check the SSE response header
    let (reader, _) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    let mut first_line = String::new();
    buf_reader.read_line(&mut first_line).await.unwrap();

    assert!(
        first_line.contains("200 OK"),
        "SSE should return 200, got: {first_line:.80}"
    );

    // Read headers until empty line
    let mut found_sse = false;
    loop {
        let mut header = String::new();
        if buf_reader.read_line(&mut header).await.unwrap() == 0 {
            break;
        }
        if header.trim().is_empty() {
            break;
        }
        if header.to_lowercase().contains("text/event-stream") {
            found_sse = true;
        }
    }
    assert!(found_sse, "SSE should have text/event-stream Content-Type");

    host.stop().await.unwrap();
    let _ = std::fs::remove_file(&wm_path);
}
