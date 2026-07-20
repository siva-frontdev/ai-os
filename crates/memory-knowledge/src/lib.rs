#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-knowledge
//!
//! Unified knowledge layer for the AI-native OS Memory Platform.
//!
//! Merges information from Working Memory, Episodic Memory, and Semantic Memory
//! into a coherent knowledge graph. Supports knowledge queries, cross-memory
//! reasoning, reference tracking, and source attribution.
//!
//! ## Public traits
//! - [`KnowledgeBase`] — unified CRUD for entities and facts
//! - [`KnowledgeQuery`] — complex queries across memory layers
//! - [`KnowledgeProvider`] — merge/query from multiple memory sources
//! - [`KnowledgeGraph`] — graph traversal and path queries
//!
//! ## Thread model
//! All shared state is `std::sync::RwLock<HashMap<…>>`.
//! Critical sections are short and never held across `.await` points.

mod error;
mod event;
pub mod knowledge_base;
pub mod knowledge_graph;
pub mod knowledge_provider;
pub mod knowledge_query;

pub use error::{KnowledgeError, KnowledgeResult};
pub use event::{KnowledgeAdded, KnowledgeMerged, KnowledgeRemoved};
pub use knowledge_base::{
    DefaultKnowledgeBase, Entity, Fact, InMemoryKnowledgeBase, KnowledgeBase,
};
pub use knowledge_graph::{DefaultKnowledgeGraph, KnowledgeEdge, KnowledgeGraph};
pub use knowledge_provider::{DefaultKnowledgeProvider, KnowledgeProvider};
pub use knowledge_query::{DefaultKnowledgeQuery, KnowledgeQuery as KQuery, QueryResult};
