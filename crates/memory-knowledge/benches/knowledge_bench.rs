use criterion::{Criterion, criterion_group, criterion_main};
use memory_knowledge::{
    DefaultKnowledgeGraph, InMemoryKnowledgeBase, KnowledgeBase, KnowledgeEdge, KnowledgeGraph,
};

pub fn bench_entity_crud(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let kb = InMemoryKnowledgeBase::new();
    c.bench_function("knowledge_insert_entity", |b| {
        b.to_async(&rt).iter(|| async {
            kb.insert_entity(memory_knowledge::Entity {
                id: uuid::Uuid::now_v7(),
                name: format!("entity-{}", uuid::Uuid::new_v4()),
                entity_type: "person".into(),
                timestamp: memory_core::Timestamp::now().as_nanos() as i64,
                sources: vec![],
            })
            .await
            .unwrap();
        })
    });
}

pub fn bench_graph_edges(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let graph = DefaultKnowledgeGraph::new();
    rt.block_on(async {
        let nodes: Vec<uuid::Uuid> = (0..10).map(|_| uuid::Uuid::new_v4()).collect();
        for i in 0..nodes.len().saturating_sub(1) {
            graph
                .add_edge(KnowledgeEdge {
                    id: uuid::Uuid::now_v7(),
                    source: nodes[i],
                    target: nodes[i + 1],
                    relation: "CONNECTED".into(),
                    weight: 0.8,
                })
                .await
                .unwrap();
        }
    });
    c.bench_function("knowledge_shortest_path", |b| {
        b.to_async(&rt).iter(|| async {
            let g = DefaultKnowledgeGraph::new();
            let a = uuid::Uuid::new_v4();
            let b = uuid::Uuid::new_v4();
            g.add_edge(KnowledgeEdge {
                id: uuid::Uuid::now_v7(),
                source: a,
                target: b,
                relation: "R".into(),
                weight: 1.0,
            })
            .await
            .unwrap();
            let _ = g.shortest_path(a, b).await;
        })
    });
}

criterion_group!(benches, bench_entity_crud, bench_graph_edges);
criterion_main!(benches);
