use crate::errors::ReasonerResult;
use crate::types::Hypothesis;
use brain_core::budget::CognitiveBudget;
use brain_core::context::ReasoningContext;
use brain_core::types::Confidence;

pub struct HypothesisGenerator;

impl HypothesisGenerator {
    pub fn new() -> Self { Self }

    pub fn generate(&self, ctx: &ReasoningContext, budget: &CognitiveBudget) -> ReasonerResult<Vec<Hypothesis>> {
        let mut hypotheses = Vec::new();
        let max = budget.max_branches.min(5);
        for i in 0..max {
            let hyp = Hypothesis {
                id: format!("hyp-{}-{}", ctx.thought_id, i),
                description: format!("hypothesis {} derived from {} observations", i, ctx.observations.len()),
                confidence: Confidence::new(0.5),
                evidence_ids: ctx.observations.iter().take(i + 1).map(|o| o.observation_id.clone()).collect(),
                source: "reasoning-engine".into(),
                is_falsified: false,
            };
            hypotheses.push(hyp);
        }
        Ok(hypotheses)
    }
}

impl Default for HypothesisGenerator {
    fn default() -> Self { Self::new() }
}
