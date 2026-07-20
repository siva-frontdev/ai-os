use crate::errors::ReasonerResult;
use crate::types::{Hypothesis, Inference, InferenceType};
use brain_core::budget::CognitiveBudget;
use brain_core::context::ReasoningContext;
use brain_core::types::Confidence;

pub struct InferenceEngine;

impl InferenceEngine {
    pub fn new() -> Self { Self }

    pub fn infer(
        &self,
        _ctx: &ReasoningContext,
        hypothesis: &Hypothesis,
        _budget: &CognitiveBudget,
    ) -> ReasonerResult<Vec<Inference>> {
        let inferences = vec![
            Inference {
                id: format!("inf-{}-deductive", hypothesis.id),
                premise: hypothesis.description.clone(),
                conclusion: format!("deduced from: {}", hypothesis.description),
                confidence: Confidence::new(hypothesis.confidence.raw() * 0.9),
                inference_type: InferenceType::Deductive,
            },
            Inference {
                id: format!("inf-{}-inductive", hypothesis.id),
                premise: hypothesis.description.clone(),
                conclusion: format!("generalized from: {}", hypothesis.description),
                confidence: Confidence::new(hypothesis.confidence.raw() * 0.7),
                inference_type: InferenceType::Inductive,
            },
        ];
        Ok(inferences)
    }
}

impl Default for InferenceEngine {
    fn default() -> Self { Self::new() }
}
