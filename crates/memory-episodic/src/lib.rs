#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! # memory-episodic
//!
//! Temporal event-sequence memory for the AI-native OS Memory Platform.
//!
//! Stores experiences — sessions, conversations, executed tasks, successes,
//! failures, user interactions, and environment changes — organized by time.
//!
//! ## Public traits
//! - [`EpisodicMemory`] — primary interface: record, recall, replay, query by time
//! - [`EpisodeStore`] — low-level CRUD for episodes by ID
//! - [`Timeline`] — time-range queries and chronological traversal
//! - [`ExperienceRecorder`] — records experiences with importance scoring
//!
//! ## Thread model
//! All shared state is `std::sync::RwLock<HashMap<…>>` or `RwLock<BTreeMap<…>>`.
//! Critical sections are short and never held across `.await` points.

pub mod episode_store;
pub mod episodic_memory;
mod error;
mod event;
pub mod experience_recorder;
pub mod timeline;

pub use episode_store::{DefaultEpisodeStore, EpisodeStore, InMemoryEpisodeStore};
pub use episodic_memory::{DefaultEpisodicMemory, EpisodicMemory, InMemoryEpisodicMemory};
pub use error::{EpisodicError, EpisodicResult};
pub use event::EpisodicEvent;
pub use experience_recorder::{
    DefaultExperienceRecorder, ExperienceRecorder, InMemoryExperienceRecorder,
};
pub use timeline::{DefaultTimeline, InMemoryTimeline, Timeline};
