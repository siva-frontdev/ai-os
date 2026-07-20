use crate::constraints::ConstraintSolver;
use crate::errors::ReasonerResult;
use crate::hypothesis::HypothesisGenerator;
use crate::inference::InferenceEngine;
use crate::risk::RiskAnalyzer;
use crate::tradeoff::TradeoffAnalyzer;
use crate::types::{Hypothesis, Inference, ReasoningResult};
use brain_core::budget::CognitiveBudget;
use brain_core::context::ReasoningContext;
use brain_core::types::Confidence;

pub struct ReasoningPipeline {
    hypothesis_gen: HypothesisGenerator,
    inference_engine: InferenceEngine,
    constraint_solver: ConstraintSolver,
    tradeoff_analyzer: TradeoffAnalyzer,
    risk_analyzer: RiskAnalyzer,
}

impl ReasoningPipeline {
    pub fn new() -> Self {
        Self {
            hypothesis_gen: HypothesisGenerator::new(),
            inference_engine: InferenceEngine::new(),
            constraint_solver: ConstraintSolver::new(),
            tradeoff_analyzer: TradeoffAnalyzer::new(),
            risk_analyzer: RiskAnalyzer::new(),
        }
    }

    pub async fn reason(
        &self,
        ctx: &ReasoningContext,
        budget: &CognitiveBudget,
    ) -> ReasonerResult<ReasoningResult> {
        let hypotheses = self.hypothesis_gen.generate(ctx, budget)?;
        let mut inferences = Vec::new();
        let mut constraints = Vec::new();
        let mut tradeoffs = Vec::new();
        let mut risks = Vec::new();
        for h in hypotheses.iter().take(budget.max_model_calls) {
            let hy_inferences = self.inference_engine.infer(ctx, h, budget)?;
            inferences.extend(hy_inferences);
        }

        for h in &hypotheses {
            let hy_constraints = self.constraint_solver.solve(ctx, h, budget)?;
            constraints.extend(hy_constraints);
        }

        if !hypotheses.is_empty() {
            let tradeoff = self.tradeoff_analyzer.analyze(&hypotheses, &constraints)?;
            tradeoffs.push(tradeoff);
        }

        for h in &hypotheses {
            let assessment = self.risk_analyzer.assess(ctx, h)?;
            risks.push(assessment);
        }

        let chain: Vec<String> = inferences.iter().map(|i| i.id.clone()).collect();
        let final_conf = self.compute_confidence(&hypotheses, &inferences);

        Ok(ReasoningResult {
            goal_id: ctx.goal_id,
            hypotheses,
            inferences,
            tradeoffs,
            risks,
            final_confidence: final_conf,
            reasoning_chain: chain,
        })
    }

    fn compute_confidence(&self, hypotheses: &[Hypothesis], inferences: &[Inference]) -> Confidence {
        if hypotheses.is_empty() {
            return Confidence::DEFAULT;
        }
        let avg_hy: f32 = hypotheses.iter().map(|h| h.confidence.raw()).sum::<f32>() / hypotheses.len() as f32;
        if inferences.is_empty() {
            return Confidence::new(avg_hy);
        }
        let avg_inf: f32 = inferences.iter().map(|i| i.confidence.raw()).sum::<f32>() / inferences.len() as f32;
        Confidence::new(avg_hy * 0.4 + avg_inf * 0.6)
    }
}

impl Default for ReasoningPipeline {
    fn default() -> Self { Self::new() }
}
