use criterion::{Criterion, black_box, criterion_group, criterion_main};

use memory_core::MemoryObject;
use memory_working::{LruWorkingMemory, WorkingMemory};

fn make_obj(content_type: &str, content: Vec<u8>) -> MemoryObject {
    MemoryObject::builder()
        .content_type(content_type)
        .content(content)
        .build()
}

fn bench_store_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("store (single object)", |b| {
        b.to_async(&rt).iter(|| async {
            let wm = LruWorkingMemory::new(10_000, 100_000_000);
            let obj = make_obj(black_box("text/plain"), black_box(b"hello world".to_vec()));
            wm.store(obj).await.unwrap();
        });
    });
}

fn bench_recall_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("recall (existing object)", |b| {
        b.to_async(&rt).iter(|| async {
            let wm = LruWorkingMemory::new(10_000, 100_000_000);
            let obj = make_obj("text/plain", b"hello world".to_vec());
            let id = obj.id;
            wm.store(obj).await.unwrap();
            let result = wm.recall(black_box(&id)).await.unwrap();
            black_box(result);
        });
    });
}

fn bench_search_performance(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let wm = LruWorkingMemory::new(10_000, 100_000_000);
    rt.block_on(async {
        for i in 0..1000 {
            let obj = make_obj(
                if i % 2 == 0 {
                    "text/plain"
                } else {
                    "application/json"
                },
                format!("content-{i}").into_bytes(),
            )
            .with_tag(if i % 3 == 0 { "important" } else { "normal" });
            wm.store(obj).await.unwrap();
        }
    });

    c.bench_function("search (1000 objects)", |b| {
        b.to_async(&rt).iter(|| async {
            let results = wm
                .search(black_box("important"), black_box(10))
                .await
                .unwrap();
            black_box(results);
        });
    });
}

fn bench_concurrent_access(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("concurrent store (10 tasks)", |b| {
        b.to_async(&rt).iter(|| async {
            let wm = std::sync::Arc::new(LruWorkingMemory::new(10_000, 100_000_000));

            let mut handles = Vec::new();
            for i in 0..10 {
                let wm = std::sync::Arc::clone(&wm);
                handles.push(tokio::spawn(async move {
                    let obj = make_obj("text", format!("data-{i}").into_bytes());
                    wm.store(obj).await.unwrap();
                }));
            }

            for h in handles {
                h.await.unwrap();
            }
        });
    });
}

fn bench_store_batch(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("store (100 objects sequential)", |b| {
        b.to_async(&rt).iter(|| async {
            let wm = LruWorkingMemory::new(10_000, 100_000_000);
            for i in 0..100 {
                let obj = make_obj("text/plain", format!("content-{i}").into_bytes());
                wm.store(obj).await.unwrap();
            }
        });
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default();
    targets =
        bench_store_throughput,
        bench_recall_throughput,
        bench_search_performance,
        bench_concurrent_access,
        bench_store_batch,
);
criterion_main!(benches);

/// Helper trait to add tags to a MemoryObject for benchmarking.
trait WithTag {
    fn with_tag(self, tag: &str) -> MemoryObject;
}

impl WithTag for MemoryObject {
    fn with_tag(mut self, tag: &str) -> MemoryObject {
        self.tags.push(tag.to_string());
        self
    }
}
