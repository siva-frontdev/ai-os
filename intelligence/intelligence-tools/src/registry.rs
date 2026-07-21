use super::error::*;
use async_trait::async_trait;
use dashmap::DashMap;
use intelligence_core::error::ModelError;
use intelligence_core::traits::ToolRegistry;
use intelligence_core::types::ToolDefinition;
use std::sync::Arc;

#[derive(Debug, Default)]
pub struct DefaultToolRegistry {
    by_name: DashMap<String, ToolDefinition>,
}

impl DefaultToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

impl From<ToolsError> for ModelError {
    fn from(e: ToolsError) -> Self {
        ModelError::Io(e.to_string())
    }
}

#[async_trait]
impl ToolRegistry for DefaultToolRegistry {
    async fn register(
        &self,
        definition: ToolDefinition,
    ) -> intelligence_core::error::ModelResult<()> {
        self.by_name.insert(definition.name.clone(), definition);
        Ok(())
    }
    async fn deregister(&self, name: &str) -> intelligence_core::error::ModelResult<()> {
        self.by_name.remove(name);
        Ok(())
    }
    async fn get(
        &self,
        name: &str,
    ) -> intelligence_core::error::ModelResult<Option<ToolDefinition>> {
        Ok(self.by_name.get(name).map(|e| e.clone()))
    }
    async fn list(&self) -> intelligence_core::error::ModelResult<Vec<ToolDefinition>> {
        Ok(self.by_name.iter().map(|e| e.clone()).collect())
    }
    fn to_provider_format(&self, provider_kind: &str) -> serde_json::Value {
        let tools: Vec<ToolDefinition> = self.by_name.iter().map(|e| e.clone()).collect();
        serde_json::json!({ "provider": provider_kind, "tools": tools })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::types::ToolDefinition;

    #[tokio::test]
    async fn register_and_get() {
        let reg = DefaultToolRegistry::new();
        let def = ToolDefinition {
            name: "calculator".into(),
            description: "math".into(),
            parameters_schema: serde_json::json!({}),
            enabled: true,
        };
        reg.register(def.clone()).await.unwrap();
        let got = reg.get("calculator").await.unwrap();
        assert!(got.is_some());
    }
}
