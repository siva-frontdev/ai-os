#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-semantic
//!
//! Knowledge storage independent of time for the AI-native OS Memory Platform.
//!
//! Stores facts, concepts, rules, definitions, capabilities, patterns, goals,
//! and constraints — the generalized knowledge the platform has abstracted from
//! episodic experience.
//!
//! ## Public traits
//! - [`SemanticMemory`] — primary interface for facts and concepts
//! - [`ConceptStore`] — stores/retrieves concepts with versioning
//! - [`RelationshipStore`] — typed relationships between concepts
//! - [`OntologyProvider`] — ontology/category queries
//!
//! ## Thread model
//! All shared state is `std::sync::RwLock<HashMap<…>>`.
//! Critical sections are short and never held across `.await` points.

pub mod concept_store;
mod error;
mod event;
pub mod ontology;
pub mod relationship_store;
pub mod semantic_memory;

pub use concept_store::{Concept, ConceptStore, DefaultConceptStore, InMemoryConceptStore};
pub use error::{SemanticError, SemanticResult};
pub use event::{ConceptCreated, ConceptUpdated, FactCreated, FactUpdated};
pub use ontology::{CategoryDescriptor, DefaultOntologyProvider, OntologyNode, OntologyProvider};
pub use relationship_store::{
    ConceptRelation, DefaultRelationshipStore, InMemoryRelationshipStore, RelationshipStore,
};
pub use semantic_memory::{
    DefaultSemanticMemory, Fact, InMemorySemanticMemory, SemanticMemory, SemanticStats,
};
