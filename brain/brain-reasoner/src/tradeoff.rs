use crate::errors::ReasonerResult;
use crate::types::{Constraint, Hypothesis, TradeoffAnalysis, TradeoffOption};

pub struct TradeoffAnalyzer;

impl TradeoffAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn analyze(
        &self,
        hypotheses: &[Hypothesis],
        _constraints: &[Constraint],
    ) -> ReasonerResult<TradeoffAnalysis> {
        let options: Vec<TradeoffOption> = hypotheses
            .iter()
            .map(|h| {
                let conf = h.confidence.raw() as f64;
                TradeoffOption {
                    id: h.id.clone(),
                    label: h.description.clone(),
                    cost: (1.0 - conf) * 10.0,
                    benefit: conf * 10.0,
                    risk: (1.0 - conf) * 5.0,
                    confidence: h.confidence,
                }
            })
            .collect();

        let recommended = options
            .iter()
            .max_by(|a, b| {
                (a.benefit - a.cost)
                    .partial_cmp(&(b.benefit - b.cost))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| o.id.clone());

        Ok(TradeoffAnalysis {
            options,
            recommended,
            reasoning: "selected option with highest net benefit".into(),
        })
    }
}

impl Default for TradeoffAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
