use brain_core::BrainError;
use brain_core::delegation::*;
use brain_core::ids::{AgentId, GoalId};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Manages available agents and routes delegation requests.
///
/// The Brain orchestrator uses this to find the right agent for
/// a given task and track delegation lifecycle.
#[derive(Debug)]
pub struct AgentRegistry {
    agents: RwLock<HashMap<AgentId, Arc<dyn AgentProvider>>>,
    active_delegations: RwLock<HashMap<GoalId, DelegationRequest>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            active_delegations: RwLock::new(HashMap::new()),
        }
    }

    /// Register an agent provider that the Brain can delegate to.
    pub fn register(&self, provider: Arc<dyn AgentProvider>) -> AgentId {
        let agent_id = AgentId::new();
        let mut agents = self.agents.write().expect("agent registry lock");
        agents.insert(agent_id, provider);
        agent_id
    }

    /// Unregister an agent.
    pub fn unregister(&self, agent_id: &AgentId) -> bool {
        let mut agents = self.agents.write().expect("agent registry lock");
        agents.remove(agent_id).is_some()
    }

    /// Find an agent by role.
    pub fn find_by_role(&self, role: AgentRole) -> Option<AgentId> {
        let agents = self.agents.read().expect("agent registry lock");
        agents
            .iter()
            .find(|(_, p)| p.role() == role)
            .map(|(id, _)| *id)
    }

    /// Find an agent capable of a specific task.
    pub fn find_by_capability(&self, capability: &str) -> Option<(AgentId, AgentRole)> {
        let agents = self.agents.read().expect("agent registry lock");
        for (id, provider) in agents.iter() {
            for cap in provider.capabilities() {
                if cap.name == capability {
                    return Some((*id, provider.role()));
                }
            }
        }
        None
    }

    /// List all registered agents.
    pub fn list_agents(&self) -> Vec<AgentRegistration> {
        let agents = self.agents.read().expect("agent registry lock");
        agents
            .iter()
            .map(|(id, provider)| AgentRegistration {
                agent_id: *id,
                name: provider.name().to_string(),
                role: provider.role(),
                capabilities: Vec::new(),
                is_available: true,
                max_concurrent_tasks: 5,
                current_tasks: 0,
            })
            .collect()
    }

    /// Check if an agent is registered.
    pub fn has_agent(&self, agent_id: &AgentId) -> bool {
        let agents = self.agents.read().expect("agent registry lock");
        agents.contains_key(agent_id)
    }

    /// Delegate a task to an agent.
    pub async fn delegate(
        &self,
        agent_id: &AgentId,
        request: DelegationRequest,
    ) -> Result<DelegationResult, BrainError> {
        let provider = {
            let agents = self.agents.read().expect("agent registry lock");
            agents
                .get(agent_id)
                .cloned()
                .ok_or(BrainError::AgentNotFound(*agent_id))?
        };

        let result = provider.delegate(request.clone()).await?;

        {
            let mut active = self.active_delegations.write().expect("delegations lock");
            if result.success {
                active.remove(&request.delegation_id);
            } else {
                active.insert(request.delegation_id, request);
            }
        }

        Ok(result)
    }

    /// Cancel an active delegation.
    pub async fn cancel_delegation(&self, delegation_id: &GoalId) -> Result<(), BrainError> {
        let request = {
            let mut active = self.active_delegations.write().expect("delegations lock");
            active.remove(delegation_id)
        };
        if let Some(req) = request {
            let provider = {
                let agents = self.agents.read().expect("agent registry lock");
                agents.get(&req.agent_id).cloned()
            };
            if let Some(p) = provider {
                p.cancel(delegation_id).await?;
            }
        }
        Ok(())
    }

    /// Query delegation status.
    pub async fn delegation_status(
        &self,
        delegation_id: &GoalId,
    ) -> Result<DelegationStatus, BrainError> {
        let active = self.active_delegations.read().expect("delegations lock");
        if active.contains_key(delegation_id) {
            Ok(DelegationStatus::InProgress)
        } else {
            Ok(DelegationStatus::Completed)
        }
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
