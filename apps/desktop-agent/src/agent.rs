use std::collections::HashMap;
use std::sync::Arc;

use ai_os_core::application::Application;
use brain_coordinator::{AutonomousRuntime, BrainOrchestrator};
use brain_core::ids::ToolCapabilityId;
use brain_core::tool::{ToolCandidate, ToolRegistry, ToolValue};
use brain_goals::InMemoryGoalStore;
use intelligence_coordinator::{DefaultCoordinator, IntelligenceReasoningService};
use osal_core::KernelFacade;
use perception_coordinator::{
    CoordinatorConfig, DefaultPerceptionCoordinator, PerceptionCoordinator,
};
use perception_observer::OsalDesktopObserver;

use crate::operator::{Action, DesktopOperator};

/// A real ToolRegistry that registers primitive desktop capabilities with the Brain.
struct DesktopToolRegistry {
    capabilities: Vec<(
        &'static str,
        &'static str,
        Vec<(&'static str, &'static str)>,
    )>,
}

impl DesktopToolRegistry {
    fn new() -> Self {
        Self {
            capabilities: vec![
                (
                    "desktop.window.focus",
                    "Focus a window by title",
                    vec![("title", "string")],
                ),
                (
                    "desktop.window.close",
                    "Close a window",
                    vec![("id", "string")],
                ),
                (
                    "desktop.app.launch",
                    "Launch an application",
                    vec![("app", "string")],
                ),
                (
                    "input.mouse.move",
                    "Move the mouse cursor",
                    vec![("x", "string"), ("y", "string")],
                ),
                (
                    "input.mouse.click",
                    "Click a mouse button",
                    vec![("button", "string")],
                ),
                (
                    "input.keyboard.type",
                    "Type text at the keyboard",
                    vec![("text", "string")],
                ),
                (
                    "input.keyboard.combo",
                    "Press a key combination",
                    vec![("keys", "string")],
                ),
                (
                    "input.keyboard.shortcut",
                    "Press a keyboard shortcut",
                    vec![("shortcut", "string")],
                ),
                (
                    "desktop.clipboard.set",
                    "Set clipboard content",
                    vec![("text", "string")],
                ),
                ("desktop.clipboard.get", "Get clipboard content", vec![]),
                ("desktop.window.list", "List open windows", vec![]),
                ("desktop.process.list", "List running processes", vec![]),
                (
                    "desktop.notification.send",
                    "Send a desktop notification",
                    vec![("title", "string"), ("body", "string")],
                ),
            ],
        }
    }
}

#[async_trait::async_trait]
impl ToolRegistry for DesktopToolRegistry {
    async fn find_candidates(
        &self,
        capability: ToolCapabilityId,
    ) -> brain_core::BrainResult<Vec<ToolCandidate>> {
        let cap_str = capability.to_string();
        let matches: Vec<ToolCandidate> = self
            .capabilities
            .iter()
            .filter(|(id, _, _)| *id == cap_str || id.contains(&cap_str))
            .map(|(_, _, _)| {
                ToolCandidate::new(
                    brain_core::ids::ToolId::new(),
                    "desktop-agent",
                    capability,
                    brain_core::types::Confidence::new(0.9),
                )
            })
            .collect();
        if matches.is_empty() {
            Ok(vec![ToolCandidate::new(
                brain_core::ids::ToolId::new(),
                "desktop-agent",
                capability,
                brain_core::types::Confidence::new(0.5),
            )])
        } else {
            Ok(matches)
        }
    }

    async fn register(&self, _candidate: ToolCandidate) -> brain_core::BrainResult<()> {
        Ok(())
    }

    async fn deregister(&self, _tool_id: brain_core::ids::ToolId) -> brain_core::BrainResult<()> {
        Ok(())
    }

    async fn list_capabilities(
        &self,
    ) -> brain_core::BrainResult<Vec<brain_core::tool::ToolCapability>> {
        Ok(self
            .capabilities
            .iter()
            .map(|(id, desc, _)| {
                let cap_id = ToolCapabilityId::new();
                brain_core::tool::ToolCapability::new(cap_id, *id, *desc)
            })
            .collect())
    }
}

struct PermissivePolicyEvaluator;

#[async_trait::async_trait]
impl brain_policy::traits::PolicyEvaluator for PermissivePolicyEvaluator {
    async fn applicable(
        &self,
        _policies: &[brain_policy::types::RuleSet],
        _ctx: &brain_core::context::DecisionContext,
    ) -> brain_policy::PolicyResult<Vec<brain_policy::types::PolicyRule>> {
        Ok(Vec::new())
    }
    async fn evaluate_policy(
        &self,
        _policy: &brain_policy::types::RuleSet,
        _ctx: &brain_core::context::DecisionContext,
        _plan: &brain_core::tool::ExecutablePlan,
    ) -> brain_policy::PolicyResult<brain_policy::types::PolicyEvaluation> {
        Ok(brain_policy::types::PolicyEvaluation {
            passed: true,
            violated_rules: vec![],
            warnings: vec![],
            allowance: 1.0,
        })
    }
    async fn aggregate(
        &self,
        _evaluations: &[brain_policy::types::PolicyEvaluation],
    ) -> brain_policy::PolicyResult<brain_policy::types::PolicyEvaluation> {
        Ok(brain_policy::types::PolicyEvaluation {
            passed: true,
            violated_rules: vec![],
            warnings: vec![],
            allowance: 1.0,
        })
    }
}

/// Convert a brain `ToolValue` into a plain string suitable for the operator.
fn tool_value_to_string(v: &ToolValue) -> String {
    match v {
        ToolValue::String(s) => s.clone(),
        ToolValue::Number(n) => n.to_string(),
        ToolValue::Bool(b) => b.to_string(),
        ToolValue::FilePath(p) => p.clone(),
        ToolValue::DurationMs(ms) => ms.to_string(),
        ToolValue::Null => String::new(),
        ToolValue::List(items) => {
            let strs: Vec<String> = items.iter().map(tool_value_to_string).collect();
            strs.join(",")
        }
        ToolValue::Map(m) => {
            let strs: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}={}", k, tool_value_to_string(v)))
                .collect();
            strs.join("&")
        }
    }
}

pub struct DesktopAgent {
    brain: Arc<BrainOrchestrator>,
    runtime: AutonomousRuntime,
    operator: DesktopOperator,
}

impl DesktopAgent {
    pub async fn new(_app: Application, kernel: KernelFacade) -> Result<Self, anyhow::Error> {
        let intelligence: Arc<dyn intelligence_core::traits::IntelligenceCoordinator> =
            Arc::new(DefaultCoordinator::new());
        let reasoning = Arc::new(IntelligenceReasoningService::new(intelligence));
        let goal_store = Arc::new(InMemoryGoalStore::new());
        let tool_registry = Arc::new(DesktopToolRegistry::new());
        let policy_evaluator: Box<dyn brain_policy::traits::PolicyEvaluator + Send + Sync> =
            Box::new(PermissivePolicyEvaluator);

        let brain = Arc::new(BrainOrchestrator::new(
            policy_evaluator,
            tool_registry,
            goal_store,
            reasoning,
        ));

        let runtime = AutonomousRuntime::new(brain.clone(), brain.goal_manager().clone());

        let kernel = Arc::new(kernel);

        let perception: Arc<dyn PerceptionCoordinator> = Arc::new(
            DefaultPerceptionCoordinator::new(CoordinatorConfig::default()),
        );
        let desktop_observer = Arc::new(OsalDesktopObserver::new(kernel.clone()));
        perception
            .register_desktop_observer(desktop_observer)
            .await
            .map_err(|e| anyhow::anyhow!("failed to register desktop observer: {e}"))?;

        let operator = DesktopOperator::new(kernel, perception);

        if let Err(e) = runtime.start().await {
            tracing::warn!("Autonomous runtime start skipped: {e}");
        } else {
            tracing::info!("Autonomous runtime started");
        }

        Ok(Self {
            brain,
            runtime,
            operator,
        })
    }

    pub async fn shutdown(&self) {
        self.runtime.stop().await.ok();
        tracing::info!("Autonomous runtime stopped");
    }

    /// Process a user request.
    ///
    /// 1. Brain understands the request and creates a goal + plan
    /// 2. For each tool requirement in the Brain's plan:
    ///    a. Convert to an Action
    ///    b. Observe → Execute → Verify → Return state
    /// 3. Return summary of reasoning + execution results
    pub async fn process_input(&self, input: &str) -> Result<String, String> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        if matches!(trimmed, "help" | "?") {
            return Ok("AI-OS Desktop Agent\n\
                 Describe what you'd like done in natural language.\n\
                 The Brain will understand, plan, and request actions.\n\
                 The Desktop Operator executes each action one at a time."
                .into());
        }

        tracing::info!("========== REQUEST ==========");
        tracing::info!("Raw User Input: {}", trimmed);

        // 1. BRAIN: understand, set goal, create plan
        tracing::info!("↓");
        tracing::info!("INTELLIGENCE: reasoning about input...");
        let result = self
            .brain
            .process_user_input(trimmed)
            .await
            .map_err(|e| format!("Brain processing failed: {e}"))?;

        tracing::info!(
            "Structured JSON: summary=\"{}\" intent=\"{}\" priority=\"{:?}\"",
            result.summary,
            result.reasoning_result.intent,
            result.reasoning_result.suggested_priority
        );
        tracing::info!("  entities: {:?}", result.reasoning_result.entities);
        tracing::info!(
            "  required_capabilities: {:?}",
            result.reasoning_result.required_capabilities
        );

        // 2. BRAIN: retrieve the current plan from the session
        tracing::info!("↓");
        tracing::info!(
            "BRAIN: goal_id={:?} plan_id={:?} state={:?}",
            result.goal_id,
            result.plan_id,
            result.state
        );

        let session = self
            .brain
            .get_session()
            .map_err(|e| format!("failed to read brain session: {e}"))?;

        let plan = match session.current_plan {
            Some(ref p) => p,
            None => {
                tracing::info!("BRAIN: no plan generated, returning summary only");
                return Ok(result.summary);
            }
        };

        tracing::info!(
            "Tool Requirements ({} total):",
            plan.tool_requirements.len()
        );
        for (i, req) in plan.tool_requirements.iter().enumerate() {
            let cap = req
                .inputs
                .get("__cap")
                .map(tool_value_to_string)
                .unwrap_or_default();
            let inputs: Vec<String> = req
                .inputs
                .iter()
                .filter(|(k, _)| *k != "__cap")
                .map(|(k, v)| format!("{}={}", k, tool_value_to_string(v)))
                .collect();
            tracing::info!("  [{}] capability=\"{}\" inputs={:?}", i, cap, inputs);
        }

        // 3. EXECUTE: for each tool requirement, execute one action at a time
        let mut outputs: Vec<String> = Vec::new();
        for requirement in &plan.tool_requirements {
            let cap = requirement
                .inputs
                .get("__cap")
                .map(tool_value_to_string)
                .unwrap_or_else(|| format!("{:?}", requirement.capability));
            let params: HashMap<String, String> = requirement
                .inputs
                .iter()
                .filter(|(k, _)| *k != "__cap")
                .map(|(k, v)| (k.clone(), tool_value_to_string(v)))
                .collect();
            let action = Action {
                capability_id: cap,
                params,
            };

            tracing::info!("↓");
            tracing::info!(
                "EXECUTION: capability=\"{}\" params={:?}",
                action.capability_id,
                action.params
            );

            let action_result = self.operator.execute_action(&action).await;

            tracing::info!(
                "  strategy_used={} attempts={} success={}",
                action_result.strategy_used,
                action_result.attempts,
                action_result.success
            );
            tracing::info!("  verification={:?}", action_result.verification);
            if let Some(ref e) = action_result.error {
                tracing::info!("  error={}", e);
            }

            if action_result.success {
                outputs.push(format!(
                    "  [ok] {} (strategy: {}, {} attempt(s))",
                    action.capability_id, action_result.strategy_used, action_result.attempts
                ));
            } else {
                outputs.push(format!(
                    "  [!!] {} (failed after {} attempt(s): {})",
                    action.capability_id,
                    action_result.attempts,
                    action_result.error.as_deref().unwrap_or("unknown"),
                ));
            }
        }

        tracing::info!("=============================");

        if outputs.is_empty() {
            Ok(result.summary)
        } else {
            Ok(format!(
                "{}\n\nActions:\n{}",
                result.summary,
                outputs.join("\n")
            ))
        }
    }
}
