use async_trait::async_trait;
use execution_core::{
    types::ExecutionResult as ExecutionResultStruct, ExecutionError, OutputRouter,
    RouteDestination, RouteResult, RoutingRule,
};
use std::sync::RwLock;

pub struct DefaultOutputRouter {
    rules: RwLock<Vec<RoutingRule>>,
}

impl std::fmt::Debug for DefaultOutputRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultOutputRouter").finish()
    }
}

impl Default for DefaultOutputRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl DefaultOutputRouter {
    pub fn new() -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
        }
    }

    fn condition_matches(condition: &str, result: &ExecutionResultStruct) -> bool {
        let state_str = result.state.to_string();
        state_str.eq_ignore_ascii_case(condition)
            || condition.eq_ignore_ascii_case("always")
            || condition.eq_ignore_ascii_case("*")
    }
}

#[async_trait]
impl OutputRouter for DefaultOutputRouter {
    async fn route(
        &self,
        result: &ExecutionResultStruct,
    ) -> Result<Vec<RouteResult>, ExecutionError> {
        let snapshot: Vec<RoutingRule> = self
            .rules
            .read()
            .map_err(|_| ExecutionError::ConfigurationError("lock poisoned".into()))?
            .clone();

        let mut all_results = Vec::new();

        for rule in &snapshot {
            if Self::condition_matches(&rule.condition, result) {
                for destination in &rule.destinations {
                    let route_result = self.route_to(result, destination).await?;
                    all_results.push(route_result);
                }
            }
        }

        Ok(all_results)
    }

    async fn route_to(
        &self,
        _result: &ExecutionResultStruct,
        destination: &RouteDestination,
    ) -> Result<RouteResult, ExecutionError> {
        match destination {
            RouteDestination::Caller => Ok(RouteResult {
                destination: "caller".into(),
                success: true,
                error: None,
            }),
            RouteDestination::Memory => Ok(RouteResult {
                destination: "memory".into(),
                success: true,
                error: None,
            }),
            RouteDestination::Brain => Ok(RouteResult {
                destination: "brain".into(),
                success: true,
                error: None,
            }),
            RouteDestination::Pipe(name) => Ok(RouteResult {
                destination: format!("pipe:{name}"),
                success: true,
                error: None,
            }),
            RouteDestination::FileSystem(path) => Ok(RouteResult {
                destination: format!("filesystem:{path}"),
                success: true,
                error: None,
            }),
            RouteDestination::EventBus(topic) => Ok(RouteResult {
                destination: format!("eventbus:{topic}"),
                success: true,
                error: None,
            }),
            RouteDestination::Log => Ok(RouteResult {
                destination: "log".into(),
                success: true,
                error: None,
            }),
            RouteDestination::Broadcast(sub_destinations) => {
                let mut combined = RouteResult {
                    destination: "broadcast".into(),
                    success: true,
                    error: None,
                };

                for sub in sub_destinations {
                    let sub_result = self.route_to(_result, sub).await?;
                    if !sub_result.success {
                        combined.success = false;
                        combined.error = Some(
                            combined.error.unwrap_or_default()
                                + &format!("{} failed; ", sub_result.destination),
                        );
                    }
                }

                Ok(combined)
            }
        }
    }

    fn add_rule(&self, rule: RoutingRule) {
        if let Ok(mut rules) = self.rules.write() {
            rules.push(rule);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use execution_core::ExecutionState;
    use memory_core::Timestamp;

    fn make_result(state: ExecutionState) -> ExecutionResultStruct {
        ExecutionResultStruct {
            execution_id: execution_core::ExecutionId::new(),
            state,
            exit_code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
            parsed_output: None,
            artifacts: Vec::new(),
            metrics: execution_core::ExecutionMetrics::default(),
            error: None,
            routing_results: Vec::new(),
            started_at: Timestamp::now(),
            completed_at: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn test_route_to_caller() {
        let router = DefaultOutputRouter::new();
        let result = make_result(ExecutionState::Completed);
        let route_result = router
            .route_to(&result, &RouteDestination::Caller)
            .await
            .unwrap();
        assert!(route_result.success);
        assert_eq!(route_result.destination, "caller");
    }

    #[tokio::test]
    async fn test_route_to_log() {
        let router = DefaultOutputRouter::new();
        let result = make_result(ExecutionState::Completed);
        let route_result = router
            .route_to(&result, &RouteDestination::Log)
            .await
            .unwrap();
        assert!(route_result.success);
    }

    #[tokio::test]
    async fn test_route_to_broadcast() {
        let router = DefaultOutputRouter::new();
        let result = make_result(ExecutionState::Completed);
        let dest =
            RouteDestination::Broadcast(vec![RouteDestination::Caller, RouteDestination::Log]);
        let route_result = router.route_to(&result, &dest).await.unwrap();
        assert!(route_result.success);
        assert_eq!(route_result.destination, "broadcast");
    }

    #[tokio::test]
    async fn test_route_with_matching_rule() {
        let router = DefaultOutputRouter::new();
        let rule = RoutingRule {
            condition: "Completed".into(),
            destinations: vec![RouteDestination::Caller],
        };
        router.add_rule(rule);

        let result = make_result(ExecutionState::Completed);
        let results = router.route(&result).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].success);
    }

    #[tokio::test]
    async fn test_route_with_non_matching_rule() {
        let router = DefaultOutputRouter::new();
        let rule = RoutingRule {
            condition: "Failed".into(),
            destinations: vec![RouteDestination::Caller],
        };
        router.add_rule(rule);

        let result = make_result(ExecutionState::Completed);
        let results = router.route(&result).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_route_to_pipe() {
        let router = DefaultOutputRouter::new();
        let result = make_result(ExecutionState::Completed);
        let route_result = router
            .route_to(&result, &RouteDestination::Pipe("chain".into()))
            .await
            .unwrap();
        assert!(route_result.success);
        assert_eq!(route_result.destination, "pipe:chain");
    }

    #[tokio::test]
    async fn test_add_rule() {
        let router = DefaultOutputRouter::new();
        let rule = RoutingRule {
            condition: "always".into(),
            destinations: vec![RouteDestination::Log],
        };
        router.add_rule(rule);

        let result = make_result(ExecutionState::Completed);
        let results = router.route(&result).await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
