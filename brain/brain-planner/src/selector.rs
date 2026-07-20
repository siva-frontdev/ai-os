use brain_core::BrainResult;
use brain_core::tool::{ToolCandidate, ToolRegistry, ToolRequirement};

#[derive(Debug)]
pub struct ToolSelector;

impl ToolSelector {
    pub fn new() -> Self {
        Self
    }

    pub async fn select_for_requirement(
        &self,
        req: &ToolRequirement,
        registry: &dyn ToolRegistry,
    ) -> BrainResult<Vec<ToolCandidate>> {
        registry.find_candidates(req.capability).await
    }

    pub fn best_match<'a>(&self, candidates: &'a [ToolCandidate]) -> Option<&'a ToolCandidate> {
        candidates.iter().min_by(|a, b| {
            let cost_cmp = a
                .estimated_cost
                .partial_cmp(&b.estimated_cost)
                .unwrap_or(std::cmp::Ordering::Equal);
            if cost_cmp == std::cmp::Ordering::Equal {
                a.estimated_duration_ms.cmp(&b.estimated_duration_ms)
            } else {
                cost_cmp
            }
        })
    }

    pub fn score_match(&self, candidate: &ToolCandidate, _req: &ToolRequirement) -> f64 {
        let cost_score = 1.0 / (1.0 + candidate.estimated_cost);
        let duration_score = 1.0 / (1.0 + candidate.estimated_duration_ms as f64 / 1000.0);
        let confidence_score = candidate.confidence.raw() as f64;
        cost_score * 0.3 + duration_score * 0.3 + confidence_score * 0.4
    }
}

impl Default for ToolSelector {
    fn default() -> Self {
        Self::new()
    }
}
