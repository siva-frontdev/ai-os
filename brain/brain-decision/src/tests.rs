#[cfg(test)]
mod tests {
    use brain_core::budget::CognitiveBudget;
    use brain_core::context::{DecisionContext, OptionRef, PolicyRef};
    use brain_core::ids::DecisionId;
    use brain_core::tool::ExecutablePlan;
    use brain_core::types::Confidence;
    use brain_policy::types::{PolicyEvaluation, PolicyRule, RuleSet, ViolatedRule};
    use brain_policy::PolicyEvaluator;
    use memory_core::Timestamp;
    use std::sync::Arc;
    use uuid::Uuid;
    use crate::decision::DecisionMaker;

    fn make_decision_id(n: u8) -> DecisionId {
        let mut buf = [0u8; 16];
        buf[0] = n;
        buf[15] = n;
        DecisionId::from(Uuid::from_bytes(buf))
    }

    fn make_option(id: &str, benefit: f64, cost: f64, risk: f64, conf: f32) -> OptionRef {
        OptionRef {
            option_id: id.into(),
            description: format!("option {}", id),
            estimated_cost: cost,
            estimated_benefit: benefit,
            estimated_risk: risk,
            confidence: Confidence::new(conf),
        }
    }

    struct MockPolicyEvaluator;

    #[async_trait::async_trait]
    impl PolicyEvaluator for MockPolicyEvaluator {
        async fn applicable(&self, _policies: &[RuleSet], _ctx: &DecisionContext) -> brain_policy::PolicyResult<Vec<PolicyRule>> {
            Ok(Vec::new())
        }
        async fn evaluate_policy(&self, _policy: &RuleSet, _ctx: &DecisionContext, _plan: &ExecutablePlan) -> brain_policy::PolicyResult<PolicyEvaluation> {
            Ok(PolicyEvaluation {
                passed: true,
                violated_rules: vec![],
                warnings: vec![],
                allowance: 1.0,
            })
        }
        async fn aggregate(&self, _evaluations: &[PolicyEvaluation]) -> brain_policy::PolicyResult<PolicyEvaluation> {
            Ok(PolicyEvaluation {
                passed: true,
                violated_rules: vec![],
                warnings: vec![],
                allowance: 1.0,
            })
        }
    }

    fn make_context(options: Vec<OptionRef>) -> DecisionContext {
        DecisionContext {
            decision_id: make_decision_id(1),
            plan_id: "plan-1".into(),
            options,
            policies: vec![],
            conflict: None,
            budget: CognitiveBudget::default(),
        }
    }

    #[test]
    fn test_decision_maker_selects_best_option() {
        let maker = DecisionMaker::new(0.3);
        let ctx = make_context(vec![
            make_option("opt-a", 10.0, 2.0, 1.0, 0.9),
            make_option("opt-b", 5.0, 1.0, 3.0, 0.5),
        ]);
        let budget = CognitiveBudget::default();
        let policy = MockPolicyEvaluator;
        let decision = maker.make_decision(&ctx, &budget, &policy).unwrap();
        assert_eq!(decision.chosen_option, "opt-a");
    }

    #[test]
    fn test_decision_maker_low_confidence() {
        let maker = DecisionMaker::new(0.9);
        let ctx = make_context(vec![
            make_option("opt-a", 1.0, 0.5, 0.5, 0.3),
        ]);
        let budget = CognitiveBudget::default();
        let policy = MockPolicyEvaluator;
        let result = maker.make_decision(&ctx, &budget, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_decision_maker_no_options() {
        let maker = DecisionMaker::new(0.3);
        let ctx = make_context(vec![]);
        let budget = CognitiveBudget::default();
        let policy = MockPolicyEvaluator;
        let result = maker.make_decision(&ctx, &budget, &policy);
        assert!(result.is_err());
    }

    #[test]
    fn test_confidence_scoring() {
        let maker = DecisionMaker::new(0.3);
        let ctx = make_context(vec![
            make_option("opt-a", 10.0, 2.0, 1.0, 0.8),
            make_option("opt-b", 5.0, 1.0, 2.0, 0.6),
        ]);
        let score = maker.compute_confidence(&ctx);
        assert!(score.overall > 0.0);
        assert!(score.overall <= 1.0);
    }

    #[test]
    fn test_explanation_generation() {
        let maker = DecisionMaker::new(0.3);
        let ctx = make_context(vec![
            make_option("opt-a", 10.0, 2.0, 1.0, 0.9),
        ]);
        let score = maker.compute_confidence(&ctx);
        let chosen = &ctx.options[0];
        let policy = MockPolicyEvaluator;
        let explanation = maker.generate_explanation(&ctx, chosen, &score, &policy);
        assert!(!explanation.summary.is_empty());
        assert!(!explanation.key_factors.is_empty());
    }
}
