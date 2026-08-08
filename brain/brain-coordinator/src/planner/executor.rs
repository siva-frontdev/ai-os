use std::collections::VecDeque;
use std::sync::Arc;

use brain_core::types::Decision;
use intelligence_core::traits::IntelligenceCoordinator;
use intelligence_core::types::{CapabilityKind, ModelInput, ModelRequest, RequestId};
use memory_core::wm::Entity;
use memory_storage::wm_store::WorldModelStore;

use super::Plan;

/// Executes planner actions.
///
/// Each planner action type maps to a concrete Rust method.
/// Adding a new action type requires:
/// 1. Register the capability in `CapabilityRegistry`
/// 2. Add a handler method here
/// No parser changes. No keyword rules.
#[derive(Debug)]
pub struct ActionExecutor {
    store: Arc<dyn WorldModelStore>,
    coordinator: Arc<dyn IntelligenceCoordinator>,
}

impl ActionExecutor {
    pub fn new(
        store: Arc<dyn WorldModelStore>,
        coordinator: Arc<dyn IntelligenceCoordinator>,
    ) -> Self {
        Self { store, coordinator }
    }

    /// Execute a plan and return the final Decision.
    ///
    /// Each action is executed in sequence. The last action's
    /// result determines the Decision returned.
    ///
    /// `runtime_context` carries the confirmed results of any runtime
    /// capabilities that were dispatched before this call. It grounds the
    /// final `respond` step so the model never claims an external action
    /// happened unless the runtime confirmed it.
    pub async fn execute(
        &self,
        plan: &Plan,
        user_message: &str,
        recent_messages: &VecDeque<crate::cognitive_loop::ConversationTurn>,
        memory_context: &str,
        context_entities: &[Entity],
        has_prior_knowledge: bool,
        is_duplicate: Option<String>,
        is_new_session: bool,
        runtime_context: &str,
    ) -> Decision {
        let mut decision = Decision::Wait;
        let mut memory_result: Option<String> = None;
        let mut conversation_result: Option<String> = None;

        for action in &plan.actions {
            match action.action_type.as_str() {
                "search_memory" => {
                    memory_result = Some(self.search_memory(&action.topics).await);
                }
                "search_recent_conversation" => {
                    conversation_result =
                        Some(self.search_recent_conversation(recent_messages, &action.topics));
                }
                "respond" => {
                    decision = self
                        .respond(
                            user_message,
                            recent_messages,
                            memory_context,
                            context_entities,
                            has_prior_knowledge,
                            is_duplicate.clone(),
                            is_new_session,
                            memory_result.as_deref(),
                            conversation_result.as_deref(),
                            runtime_context,
                        )
                        .await;
                }
                "ask_clarification" => {
                    let question = action
                        .topics
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "Could you clarify what you mean?".to_string());
                    decision = Decision::Communicate {
                        recipient: "user".into(),
                        message: question,
                        reason: action.reason.clone(),
                    };
                }
                "ignore" => {
                    decision = Decision::Wait;
                }
                "observe" => {
                    decision = Decision::Wait;
                }
                "schedule" => {
                    tracing::info!("schedule action requested but not yet implemented");
                    decision = Decision::Wait;
                }
                other => {
                    tracing::warn!("Unknown planner action type: {other}");
                }
            }
        }

        decision
    }

    /// Search stored memory for relevant topics.
    /// Returns a natural-language summary of what was found.
    async fn search_memory(&self, topics: &[String]) -> String {
        let all = self.store.all_entities().await;
        if all.is_empty() {
            return "I don't have any stored knowledge yet.".into();
        }

        // Search by topic keywords or return everything if no specific topics
        let relevant: Vec<&Entity> = if topics.is_empty() || topics.iter().any(|t| t == "all") {
            all.iter().collect()
        } else {
            let lower_topics: Vec<String> = topics.iter().map(|t| t.to_lowercase()).collect();
            all.iter()
                .filter(|e| {
                    let name_lower = e.name.to_lowercase();
                    lower_topics.iter().any(|t| name_lower.contains(t))
                        || lower_topics
                            .iter()
                            .any(|t| e.entity_type.to_lowercase().contains(t))
                })
                .collect()
        };

        if relevant.is_empty() {
            return "I don't have any knowledge matching those topics.".into();
        }

        // Build a natural-language description
        let mut parts: Vec<String> = Vec::new();
        let mut people: Vec<&str> = Vec::new();
        let mut projects: Vec<String> = Vec::new();
        let mut skills: Vec<&str> = Vec::new();
        let mut other: Vec<&str> = Vec::new();

        for entity in &relevant {
            match entity.entity_type.as_str() {
                "person" | "user" | "human" => people.push(entity.name.as_str()),
                "project" | "task" | "goal" | "plan" => {
                    let label = match entity.properties.get("status").and_then(|v| v.as_str()) {
                        Some(s) if !s.is_empty() => format!("{} ({})", entity.name, s),
                        _ => entity.name.clone(),
                    };
                    projects.push(label);
                }
                "skill" | "interest" | "hobby" => skills.push(entity.name.as_str()),
                _ => other.push(entity.name.as_str()),
            }
        }

        if !people.is_empty() {
            parts.push(format!("people: {}", people.join(", ")));
        }
        if !projects.is_empty() {
            parts.push(format!("projects: {}", projects.join(", ")));
        }
        if !skills.is_empty() {
            parts.push(format!("interests: {}", skills.join(", ")));
        }
        if !other.is_empty() {
            parts.push(format!("other: {}", other.join(", ")));
        }

        parts.join("; ")
    }

    /// Search recent conversation history for relevant topics.
    fn search_recent_conversation(
        &self,
        messages: &VecDeque<crate::cognitive_loop::ConversationTurn>,
        topics: &[String],
    ) -> String {
        if messages.is_empty() {
            return String::new();
        }

        let msg_text: Vec<String> = messages
            .iter()
            .map(|t| {
                let who = if t.is_from_user { "User" } else { "Companion" };
                format!("{who}: {}", t.message)
            })
            .collect();

        if topics.is_empty() {
            return msg_text.join("\n");
        }

        let lower_topics: Vec<String> = topics.iter().map(|t| t.to_lowercase()).collect();
        let relevant_strs: Vec<&str> = msg_text
            .iter()
            .filter(|msg| {
                let lower = msg.to_lowercase();
                lower_topics.iter().any(|t| lower.contains(t))
            })
            .map(|s| s.as_str())
            .collect();

        if relevant_strs.is_empty() {
            return msg_text.join("\n");
        }
        relevant_strs.join("\n")
    }

    /// Generate a natural-language response.
    #[allow(unused_variables)]
    async fn respond(
        &self,
        user_message: &str,
        recent_messages: &VecDeque<crate::cognitive_loop::ConversationTurn>,
        memory_context: &str,
        context_entities: &[Entity],
        has_prior_knowledge: bool,
        is_duplicate: Option<String>,
        is_new_session: bool,
        memory_result: Option<&str>,
        conversation_result: Option<&str>,
        runtime_context: &str,
    ) -> Decision {
        let recent_history: Vec<String> = recent_messages
            .iter()
            .rev()
            .take(4)
            .map(|t| {
                let who = if t.is_from_user { "User" } else { "You" };
                format!("{who}: {}", t.message)
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let history_block = if recent_history.is_empty() {
            String::new()
        } else {
            format!("\nRecent conversation:\n{}\n", recent_history.join("\n"))
        };

        let context_block = if memory_context.is_empty() {
            String::new()
        } else {
            format!("\nContext from memory: {}\n", memory_context)
        };

        let search_block = match memory_result {
            Some(result) if !result.is_empty() => {
                format!("\nMemory search results: {}\n", result)
            }
            _ => String::new(),
        };

        let conversation_block = match conversation_result {
            Some(result) if !result.is_empty() => {
                format!("\nConversation search results:\n{}\n", result)
            }
            _ => String::new(),
        };

        let continuity_block = if is_new_session && has_prior_knowledge {
            "\nThis is the first message after a gap. The user may have returned after being away. Naturally reconnect.\n".into()
        } else {
            String::new()
        };

        let duplicate_block = match is_duplicate {
            Some(ref orig) if orig != user_message => {
                format!(
                    "\nNote: the user said something similar before ({:.50}). Vary the response.\n",
                    orig
                )
            }
            Some(_) => {
                "\nNote: the user repeated themselves. Acknowledge briefly and vary the response.\n"
                    .into()
            }
            None => String::new(),
        };

        let runtime_block = if runtime_context.trim().is_empty() {
            String::new()
        } else {
            format!(
                "\nConfirmed action results (ground truth — only these external actions actually happened):\n{runtime_context}\n"
            )
        };

        let prompt = format!(
            r#"You are a helpful companion. Respond naturally and conversationally.

The user's message is below. Answer their question, acknowledge their statement, or continue the conversation naturally. Be warm, concise, and helpful. Do NOT list entities or internal state.{history_block}{context_block}{search_block}{conversation_block}{continuity_block}{duplicate_block}{runtime_block}
IMPORTANT: If the user asked you to perform an external action (send an email, post a message, create a file, etc.), ONLY claim it was done if the confirmed action results above list it as SUCCEEDED. If an external action is not confirmed, or if no action results are shown, tell the user honestly that it could not be completed. Never claim an external action succeeded unless you have confirmed proof above.

User message: {user_message}"#,
            history_block = history_block,
            context_block = context_block,
            search_block = search_block,
            conversation_block = conversation_block,
            continuity_block = continuity_block,
            duplicate_block = duplicate_block,
            runtime_block = runtime_block,
            user_message = user_message,
        );

        let request = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text(prompt),
            parameters: Default::default(),
            temperature: Some(0.7),
            top_p: None,
            max_tokens: Some(512),
            stream: false,
        };

        match self.coordinator.request(request).await {
            Ok(response) => {
                let content = response.content.trim().to_string();
                if content.is_empty() {
                    return Decision::Wait;
                }
                Decision::Communicate {
                    recipient: "user".into(),
                    message: content,
                    reason: "response to user message".into(),
                }
            }
            Err(e) => {
                tracing::warn!("LLM response failed: {e}");
                Decision::Wait
            }
        }
    }
}
