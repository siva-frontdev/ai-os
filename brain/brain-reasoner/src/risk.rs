use crate::errors::ReasonerResult;
use crate::types::{Hypothesis, RiskAssessment};
use brain_core::context::ReasoningContext;

pub struct RiskAnalyzer;

impl RiskAnalyzer {
    pub fn new() -> Self { Self }

    pub fn assess(&self, _ctx: &ReasoningContext, hypothesis: &Hypothesis) -> ReasonerResult<RiskAssessment> {
        let risk = 1.0 - hypothesis.confidence.raw() as f64;
        Ok(RiskAssessment {
            risk_id: format!("risk-{}", hypothesis.id),
            description: format!("risk assessment for: {}", hypothesis.description),
            probability: risk * 0.7,
            impact: risk * 10.0,
            mitigation: "increase evidence collection".into(),
            residual_risk: risk * 0.3,
        })
    }
}

impl Default for RiskAnalyzer {
    fn default() -> Self { Self::new() }
}
