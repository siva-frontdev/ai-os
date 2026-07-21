use async_trait::async_trait;
use intelligence_core::error::ModelResult;
use intelligence_core::traits::SafetyEnforcer;
use intelligence_core::types::{
    ModelRequest, ModelResponse, SafetyAction, SafetyCategory, SafetyEvent, SafetyResult,
    SafetyRule, SafetyRuleId, SafetySeverity,
};

#[derive(Debug, Default)]
pub struct DefaultSafetyEnforcer;

#[async_trait]
impl SafetyEnforcer for DefaultSafetyEnforcer {
    async fn check_input(&self, _request: &ModelRequest) -> ModelResult<SafetyResult> {
        Ok(SafetyResult {
            passed: true,
            events: vec![],
            sanitized_input: None,
            sanitized_output: None,
        })
    }
    async fn check_output(&self, _response: &ModelResponse) -> ModelResult<SafetyResult> {
        Ok(SafetyResult {
            passed: true,
            events: vec![],
            sanitized_input: None,
            sanitized_output: None,
        })
    }
    async fn add_rule(&self, _rule: SafetyRule) -> ModelResult<()> {
        Ok(())
    }
    async fn remove_rule(&self, _rule_id: &SafetyRuleId) -> ModelResult<()> {
        Ok(())
    }
    async fn list_rules(&self) -> ModelResult<Vec<SafetyRule>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn smoke() {
        let _r = DefaultSafetyEnforcer::default();
    }
}
