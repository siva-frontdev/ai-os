use brain_core::model::*;
use brain_core::types::Confidence;
use std::collections::HashMap;
use std::sync::RwLock;

/// Manages the Brain's World Model — its structured knowledge about
/// the environment, system state, user, and learned facts.
///
/// The World Model is continuously updated through observation and
/// reflection. It provides the context for all cognitive operations.
#[derive(Debug)]
pub struct WorldModelService {
    model: RwLock<WorldModel>,
    resource: RwLock<ResourceModel>,
}

impl WorldModelService {
    pub fn new() -> Self {
        Self {
            model: RwLock::new(WorldModel::default()),
            resource: RwLock::new(ResourceModel::default()),
        }
    }

    // ── App tracking ─────────────────────────────────────────

    pub fn register_app(&self, name: &str, version: &str, description: &str) {
        let mut model = self.model.write().expect("world model lock");
        model.known_apps.push(AppEntry {
            name: name.into(),
            version: version.into(),
            description: description.into(),
            executable_path: None,
            categories: Vec::new(),
            confidence: Confidence::new(0.7),
        });
    }

    pub fn known_apps(&self) -> Vec<AppEntry> {
        let model = self.model.read().expect("world model lock");
        model.known_apps.clone()
    }

    // ── Software tracking ────────────────────────────────────

    pub fn register_software(&self, name: &str, version: &str, kind: SoftwareKind) {
        let mut model = self.model.write().expect("world model lock");
        model.known_software.push(SoftwareEntry {
            name: name.into(),
            version: version.into(),
            kind,
            confidence: Confidence::new(0.7),
        });
    }

    pub fn known_software(&self) -> Vec<SoftwareEntry> {
        let model = self.model.read().expect("world model lock");
        model.known_software.clone()
    }

    // ── User tracking ────────────────────────────────────────

    pub fn register_user(&self, username: &str, display_name: Option<&str>) {
        let mut model = self.model.write().expect("world model lock");
        if let Some(existing) = model.users.iter_mut().find(|u| u.username == username) {
            if let Some(dn) = display_name {
                existing.display_name = Some(dn.into());
            }
            return;
        }
        model.users.push(UserProfile {
            username: username.into(),
            display_name: display_name.map(|s| s.into()),
            preferences: HashMap::new(),
            known_capabilities: Vec::new(),
            confidence: Confidence::new(0.5),
        });
    }

    pub fn set_user_preference(&self, username: &str, key: &str, value: &str) {
        let mut model = self.model.write().expect("world model lock");
        if let Some(user) = model.users.iter_mut().find(|u| u.username == username) {
            user.preferences.insert(key.into(), value.into());
        }
    }

    pub fn get_user_preference(&self, username: &str, key: &str) -> Option<String> {
        let model = self.model.read().expect("world model lock");
        model
            .users
            .iter()
            .find(|u| u.username == username)
            .and_then(|u| u.preferences.get(key).cloned())
    }

    pub fn users(&self) -> Vec<UserProfile> {
        let model = self.model.read().expect("world model lock");
        model.users.clone()
    }

    // ── Environment ──────────────────────────────────────────

    pub fn set_env(&self, key: &str, value: &str) {
        let mut model = self.model.write().expect("world model lock");
        model.environment.insert(key.into(), value.into());
    }

    pub fn get_env(&self, key: &str) -> Option<String> {
        let model = self.model.read().expect("world model lock");
        model.environment.get(key).cloned()
    }

    // ── Learned facts ────────────────────────────────────────

    pub fn add_fact(&self, fact: &str, source: &str, confidence: f32) {
        let mut model = self.model.write().expect("world model lock");
        model.learned_facts.push(LearnedFact {
            fact: fact.into(),
            source: source.into(),
            confidence: Confidence::new(confidence),
            learned_at: memory_core::Timestamp::now().as_nanos(),
        });
    }

    pub fn learned_facts(&self) -> Vec<LearnedFact> {
        let model = self.model.read().expect("world model lock");
        model.learned_facts.clone()
    }

    // ── Resource tracking ────────────────────────────────────

    pub fn update_resources(&self, resources: ResourceModel) {
        let mut current = self.resource.write().expect("resource model lock");
        *current = resources;
    }

    pub fn resource_model(&self) -> ResourceModel {
        let model = self.resource.read().expect("resource model lock");
        model.clone()
    }

    pub fn resource_state(&self) -> ResourceState {
        let model = self.resource.read().expect("resource model lock");
        if model.cpu.usage_percent > 95.0
            || model.memory.usage_percent > 95.0
            || model.disk.usage_percent > 95.0
        {
            ResourceState::Exhausted
        } else if model.cpu.usage_percent > 80.0
            || model.memory.usage_percent > 80.0
            || model.disk.usage_percent > 80.0
        {
            ResourceState::Critical
        } else if model.cpu.usage_percent > 60.0
            || model.memory.usage_percent > 60.0
            || model.disk.usage_percent > 60.0
        {
            ResourceState::Degraded
        } else {
            ResourceState::Healthy
        }
    }

    // ── Snapshot ─────────────────────────────────────────────

    pub fn snapshot(&self) -> (WorldModel, ResourceModel) {
        let model = self.model.read().expect("world model lock");
        let resource = self.resource.read().expect("resource model lock");
        (model.clone(), resource.clone())
    }

    pub fn restore(&self, model: WorldModel, resource: ResourceModel) {
        let mut m = self.model.write().expect("world model lock");
        let mut r = self.resource.write().expect("resource model lock");
        *m = model;
        *r = resource;
    }
}

impl Default for WorldModelService {
    fn default() -> Self {
        Self::new()
    }
}
