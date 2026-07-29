use crate::types::Confidence;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The Brain's structured knowledge about its environment.
///
/// The World Model is the Brain's persistent, structured representation of
/// everything it knows about the system it controls and the user it serves.
/// It is updated continuously through observation and reflection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldModel {
    /// Known applications installed on the system
    pub known_apps: Vec<AppEntry>,
    /// Known software libraries and runtimes
    pub known_software: Vec<SoftwareEntry>,
    /// Known files and directories (indexed paths)
    pub known_files: Vec<FileEntry>,
    /// Known projects and workspaces
    pub known_projects: Vec<ProjectEntry>,
    /// Known containers and services
    pub known_containers: Vec<ContainerEntry>,
    /// Known servers and endpoints
    pub known_servers: Vec<ServerEntry>,
    /// Cloud service connections
    pub cloud_connections: Vec<CloudEntry>,
    /// User profiles and preferences
    pub users: Vec<UserProfile>,
    /// Environment variables and system configuration
    pub environment: HashMap<String, String>,
    /// Free-form learned knowledge
    pub learned_facts: Vec<LearnedFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub executable_path: Option<String>,
    pub categories: Vec<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoftwareEntry {
    pub name: String,
    pub version: String,
    pub kind: SoftwareKind,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoftwareKind {
    Runtime,
    Library,
    Tool,
    Framework,
    Service,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub file_type: String,
    pub size_bytes: Option<u64>,
    pub last_modified: Option<i64>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub root_path: String,
    pub language: Option<String>,
    pub build_system: Option<String>,
    pub description: Option<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerEntry {
    pub id: Option<String>,
    pub name: String,
    pub image: String,
    pub status: String,
    pub ports: Vec<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEntry {
    pub hostname: String,
    pub port: u16,
    pub protocol: String,
    pub service_type: String,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudEntry {
    pub provider: String,
    pub service: String,
    pub region: Option<String>,
    pub connected: bool,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub username: String,
    pub display_name: Option<String>,
    pub preferences: HashMap<String, String>,
    pub known_capabilities: Vec<String>,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedFact {
    pub fact: String,
    pub source: String,
    pub confidence: Confidence,
    pub learned_at: i64,
}

impl Default for WorldModel {
    fn default() -> Self {
        Self {
            known_apps: Vec::new(),
            known_software: Vec::new(),
            known_files: Vec::new(),
            known_projects: Vec::new(),
            known_containers: Vec::new(),
            known_servers: Vec::new(),
            cloud_connections: Vec::new(),
            users: Vec::new(),
            environment: HashMap::new(),
            learned_facts: Vec::new(),
        }
    }
}

/// The Brain's understanding of available system resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceModel {
    pub cpu: CpuResources,
    pub memory: MemoryResources,
    pub disk: DiskResources,
    pub network: NetworkResources,
    pub budget: BudgetResources,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuResources {
    pub total_cores: u32,
    pub available_cores: u32,
    pub load_average_1m: f64,
    pub load_average_5m: f64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryResources {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskResources {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkResources {
    pub interfaces: Vec<String>,
    pub is_connected: bool,
    pub download_speed_bps: Option<u64>,
    pub upload_speed_bps: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetResources {
    pub tokens_available: u32,
    pub tokens_used: u32,
    pub cost_cents_available: u64,
    pub cost_cents_used: u64,
    pub model_calls_remaining: usize,
}

/// Summary of the current resource state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceState {
    Healthy,
    Degraded,
    Critical,
    Exhausted,
}

impl Default for ResourceModel {
    fn default() -> Self {
        Self {
            cpu: CpuResources {
                total_cores: 0,
                available_cores: 0,
                load_average_1m: 0.0,
                load_average_5m: 0.0,
                usage_percent: 0.0,
            },
            memory: MemoryResources {
                total_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
            },
            disk: DiskResources {
                total_bytes: 0,
                available_bytes: 0,
                usage_percent: 0.0,
            },
            network: NetworkResources {
                interfaces: Vec::new(),
                is_connected: false,
                download_speed_bps: None,
                upload_speed_bps: None,
            },
            budget: BudgetResources {
                tokens_available: 0,
                tokens_used: 0,
                cost_cents_available: 0,
                cost_cents_used: 0,
                model_calls_remaining: 0,
            },
        }
    }
}
