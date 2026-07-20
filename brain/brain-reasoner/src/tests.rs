#[cfg(test)]
mod tests {
    use crate::constraints::ConstraintSolver;
    use crate::engine::ReasoningPipeline;
    use crate::hypothesis::HypothesisGenerator;
    use crate::inference::InferenceEngine;
    use crate::risk::RiskAnalyzer;
    use crate::tradeoff::TradeoffAnalyzer;
    use crate::types::{ConstraintSeverity, Hypothesis, InferenceType};
    use brain_core::budget::CognitiveBudget;
    use brain_core::context::ReasoningContext;
    use brain_core::ids::{GoalId, ThoughtId};
    use uuid::Uuid;

    fn make_thought_id(n: u8) -> ThoughtId {
        let mut buf = [0u8; 16];
        buf[0] = n;
        buf[15] = n;
        ThoughtId::from(Uuid::from_bytes(buf))
    }

    fn make_goal_id(n: u8) -> GoalId {
        let mut buf = [0u8; 16];
        buf[0] = n;
        buf[15] = n;
        GoalId::from(Uuid::from_bytes(buf))
    }

    fn make_context() -> ReasoningContext {
        ReasoningContext {
            thought_id: make_thought_id(1),
            goal_id: make_goal_id(1),
            observations: vec![],
            hypotheses_in_progress: vec![],
            constraints: vec![],
            budget: CognitiveBudget::default(),
            session_id: "test-session".into(),
        }
    }

    #[test]
    fn test_hypothesis_generation() {
        let generator = HypothesisGenerator::new();
        let ctx = make_context();
        let budget = CognitiveBudget::default();
        let hypotheses = generator.generate(&ctx, &budget).unwrap();
        assert!(!hypotheses.is_empty());
        assert!(hypotheses.len() <= 5);
    }

    #[test]
    fn test_inference_engine() {
        let engine = InferenceEngine::new();
        let ctx = make_context();
        let budget = CognitiveBudget::default();
        let hyp = Hypothesis {
            id: "test-hyp".into(),
            description: "test hypothesis".into(),
            confidence: brain_core::types::Confidence::new(0.8),
            evidence_ids: vec![],
            source: "test".into(),
            is_falsified: false,
        };
        let inferences = engine.infer(&ctx, &hyp, &budget).unwrap();
        assert_eq!(inferences.len(), 2);
        assert!(
            inferences
                .iter()
                .any(|i| i.inference_type == InferenceType::Deductive)
        );
        assert!(
            inferences
                .iter()
                .any(|i| i.inference_type == InferenceType::Inductive)
        );
    }

    #[test]
    fn test_constraint_solver() {
        let solver = ConstraintSolver::new();
        let ctx = make_context();
        let budget = CognitiveBudget::default();
        let hyp = Hypothesis {
            id: "test-hyp".into(),
            description: "test".into(),
            confidence: brain_core::types::Confidence::new(0.5),
            evidence_ids: vec![],
            source: "test".into(),
            is_falsified: false,
        };
        let constraints = solver.solve(&ctx, &hyp, &budget).unwrap();
        assert!(!constraints.is_empty());
        assert!(
            constraints
                .iter()
                .any(|c| c.severity == ConstraintSeverity::Hard)
        );
    }

    #[test]
    fn test_tradeoff_analysis() {
        let analyzer = TradeoffAnalyzer::new();
        let hyps = vec![
            Hypothesis {
                id: "h1".into(),
                description: "option a".into(),
                confidence: brain_core::types::Confidence::new(0.9),
                evidence_ids: vec![],
                source: "test".into(),
                is_falsified: false,
            },
            Hypothesis {
                id: "h2".into(),
                description: "option b".into(),
                confidence: brain_core::types::Confidence::new(0.5),
                evidence_ids: vec![],
                source: "test".into(),
                is_falsified: false,
            },
        ];
        let analysis = analyzer.analyze(&hyps, &[]).unwrap();
        assert_eq!(analysis.options.len(), 2);
        assert!(analysis.recommended.is_some());
    }

    #[test]
    fn test_risk_assessment() {
        let analyzer = RiskAnalyzer::new();
        let ctx = make_context();
        let hyp = Hypothesis {
            id: "test-hyp".into(),
            description: "risky hypothesis".into(),
            confidence: brain_core::types::Confidence::new(0.5),
            evidence_ids: vec![],
            source: "test".into(),
            is_falsified: false,
        };
        let assessment = analyzer.assess(&ctx, &hyp).unwrap();
        assert!(assessment.probability > 0.0);
        assert!(assessment.impact > 0.0);
    }

    #[tokio::test]
    async fn test_reasoning_pipeline() {
        let pipeline = ReasoningPipeline::new();
        let ctx = make_context();
        let budget = CognitiveBudget::default();
        let result = pipeline.reason(&ctx, &budget).await.unwrap();
        assert!(!result.hypotheses.is_empty());
        assert!(!result.inferences.is_empty());
        assert!(!result.risks.is_empty());
        assert!(!result.tradeoffs.is_empty());
    }
}
