//! Conversation context definitions.
use super::*;
use serde::{Deserialize, Serialize};

/// In-memory conversation history.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InMemoryConversationContext {
    messages: Vec<Message>,
    max_turns: usize,
}

impl InMemoryConversationContext {
    pub fn new(max_turns: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_turns,
        }
    }
}

impl ConversationContext for InMemoryConversationContext {
    fn push(&mut self, message: Message) {
        self.messages.push(message);
        if self.messages.len() > self.max_turns * 2 {
            self.messages.drain(0..2);
        }
    }

    fn history(&self) -> Vec<Message> {
        self.messages.clone()
    }

    fn clear(&mut self) {
        self.messages.clear();
    }
}
