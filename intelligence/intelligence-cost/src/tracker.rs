use super::error::*;
use async_trait::async_trait;
use intelligence_core::error::ModelError;
use intelligence_core::traits::CostTracker;
use intelligence_core::types::{
    BudgetCheck, CapabilityKind, ConversationId, CostAccount, CostPeriod, CostReport, ModelInput,
    ModelRequest, RequestId,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;
use uuid::Uuid;

fn cost_to_model_err(e: CostError) -> ModelError {
    ModelError::Io(e.to_string())
}

#[derive(Debug, Default)]
pub struct DefaultCostAccountant {
    daily_spend_cents: AtomicU64,
    conversation_spend: RwLock<HashMap<Uuid, f64>>,
}

impl DefaultCostAccountant {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CostTracker for DefaultCostAccountant {
    async fn record(&self, account: CostAccount) -> intelligence_core::error::ModelResult<()> {
        self.daily_spend_cents
            .fetch_add(account.total_tokens, Ordering::Relaxed);
        Ok(())
    }
    async fn check_budget(
        &self,
        _request: &ModelRequest,
    ) -> intelligence_core::error::ModelResult<BudgetCheck> {
        let spent = self.daily_spend_cents.load(Ordering::Relaxed);
        let daily_limit = 1_000_000;
        Ok(BudgetCheck {
            allowed: spent < daily_limit,
            remaining_cents: Some((daily_limit - spent) as f64),
            warning: (spent as f64) > (daily_limit as f64 * 0.8),
            reason: None,
        })
    }
    async fn get_report(
        &self,
        _period: CostPeriod,
    ) -> intelligence_core::error::ModelResult<CostReport> {
        let spent = self.daily_spend_cents.load(Ordering::Relaxed);
        Ok(CostReport {
            period: CostPeriod::Daily,
            total_tokens: spent,
            total_cost_cents: spent as f64,
        })
    }
    async fn get_conversation_cost(
        &self,
        conversation_id: &ConversationId,
    ) -> intelligence_core::error::ModelResult<f64> {
        let guard = self.conversation_spend.read().await;
        Ok(*guard.get(&conversation_id.0.into()).unwrap_or(&0.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use intelligence_core::error::ModelResult;
    #[tokio::test]
    async fn record_and_check() {
        let acct = DefaultCostAccountant::new();
        let account = CostAccount {
            account_id: intelligence_core::types::CostAccountId(uuid::Uuid::new_v4()),
            conversation_id: ConversationId::new(),
            total_tokens: 100,
            total_cost_cents: 0.5,
            period: CostPeriod::Daily,
        };
        acct.record(account).await.unwrap();
        let req = ModelRequest {
            request_id: RequestId::new(),
            capability: CapabilityKind::Chat,
            model_id: None,
            input: ModelInput::Text("hi".into()),
            parameters: Default::default(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            stream: false,
        };
        let check: ModelResult<BudgetCheck> = acct.check_budget(&req).await;
        assert!(check.is_ok());
    }
}
