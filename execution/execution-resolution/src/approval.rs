use async_trait::async_trait;
use execution_core::*;
use memory_core::Timestamp;
use std::collections::VecDeque;
use tokio::sync::RwLock;
use tracing::warn;

#[derive(Debug)]
pub struct DefaultHumanApproval {
    pending: RwLock<VecDeque<ApprovalRecord>>,
    auto_approve: bool,
}

impl DefaultHumanApproval {
    pub fn new(auto_approve: bool) -> Self {
        Self {
            pending: RwLock::new(VecDeque::new()),
            auto_approve,
        }
    }
}

impl Default for DefaultHumanApproval {
    fn default() -> Self {
        Self::new(false)
    }
}

#[async_trait]
impl HumanApproval for DefaultHumanApproval {
    async fn request_approval(
        &self,
        capability_id: &str,
        description: &str,
        provider: &ProviderMetadata,
        trust: &TrustScore,
    ) -> execution_core::ExecutionResult<bool> {
        if self.auto_approve {
            let record = ApprovalRecord {
                capability_id: capability_id.into(),
                description: format!("auto-approved: {description}"),
                approved: true,
                approved_at: Timestamp::now(),
                reason: Some("auto-approve enabled".into()),
            };
            let mut pending = self.pending.write().await;
            pending.push_back(record);
            return Ok(true);
        }

        warn!(
            capability = %capability_id,
            provider = %provider.name,
            trust = trust.score,
            "human approval required for capability"
        );

        let record = ApprovalRecord {
            capability_id: capability_id.into(),
            description: description.into(),
            approved: false,
            approved_at: Timestamp::now(),
            reason: None,
        };
        let mut pending = self.pending.write().await;
        pending.push_back(record);

        Err(execution_core::ExecutionError::HumanApprovalRequired(
            capability_id.into(),
        ))
    }

    async fn request_destructive_approval(
        &self,
        capability_id: &str,
        operation: &str,
        details: &str,
    ) -> execution_core::ExecutionResult<bool> {
        warn!(
            capability = %capability_id,
            operation = %operation,
            "destructive operation requires human approval"
        );

        Err(
            execution_core::ExecutionError::DestructiveOperationRequiresApproval(format!(
                "{capability_id}: {operation} - {details}"
            )),
        )
    }

    async fn approval_history(
        &self,
        limit: usize,
    ) -> execution_core::ExecutionResult<Vec<ApprovalRecord>> {
        let pending = self.pending.read().await;
        Ok(pending.iter().rev().take(limit).cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_auto_approve() {
        let approval = DefaultHumanApproval::new(true);
        let result = approval
            .request_approval(
                "test.cap",
                "test description",
                &ProviderMetadata::new("test", "cli", "/test"),
                &TrustScore::new(CapabilityOrigin::Generated),
            )
            .await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_manual_approval_required() {
        let approval = DefaultHumanApproval::new(false);
        let result = approval
            .request_approval(
                "test.cap",
                "test",
                &ProviderMetadata::new("test", "cli", "/test"),
                &TrustScore::new(CapabilityOrigin::Generated),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_destructive_always_denied() {
        let approval = DefaultHumanApproval::new(true);
        let result = approval
            .request_destructive_approval("test.cap", "rm -rf /", "deletes everything")
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_approval_history() {
        let approval = DefaultHumanApproval::new(true);
        approval
            .request_approval(
                "cap1",
                "first",
                &ProviderMetadata::new("t1", "cli", "/t1"),
                &TrustScore::new(CapabilityOrigin::Generated),
            )
            .await
            .unwrap();
        approval
            .request_approval(
                "cap2",
                "second",
                &ProviderMetadata::new("t2", "cli", "/t2"),
                &TrustScore::new(CapabilityOrigin::Generated),
            )
            .await
            .unwrap();
        let history = approval.approval_history(10).await.unwrap();
        assert_eq!(history.len(), 2);
    }
}
