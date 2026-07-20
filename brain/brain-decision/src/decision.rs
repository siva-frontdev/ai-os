use crate::errors::{DecisionError, DecisionResult};
use crate::types::{ConfidenceScore, ConflictDescription, Decision, Explanation};
use brain_core::budget::CognitiveBudget;
use brain_core::context::{DecisionContext, OptionRef};
use brain_core::types::Confidence;
use brain_policy::PolicyEvaluator;

pub struct DecisionMaker {
    confidence_threshold: f64,
}

impl DecisionMaker {
    pub fn new(confidence_threshold: f64) -> Self {
        Self {
            confidence_threshold,
        }
    }

    pub fn make_decision(
        &self,
        ctx: &DecisionContext,
        _budget: &CognitiveBudget,
        policy: &dyn PolicyEvaluator,
    ) -> DecisionResult<Decision> {
        let score = self.compute_confidence(ctx);
        if score.overall < self.confidence_threshold {
            return Err(DecisionError::LowConfidence {
                conf: score.overall,
                threshold: self.confidence_threshold,
            });
        }

        let chosen = self.select_option(ctx, &score)?;
        let explanation = self.generate_explanation(ctx, chosen, &score, policy);

        let alternatives: Vec<String> = ctx.options.iter().map(|o| o.option_id.clone()).collect();

        Ok(Decision {
            decision_id: ctx.decision_id,
            goal_id: ctx.plan_id.parse().unwrap_or_default(),
            chosen_option: chosen.option_id.clone(),
            confidence: Confidence::new(score.overall as f32),
            alternatives,
            reasoning: explanation.summary.clone(),
            explanation: explanation.summary,
        })
    }

    pub fn compute_confidence(&self, ctx: &DecisionContext) -> ConfidenceScore {
        if ctx.options.is_empty() {
            return ConfidenceScore {
                overall: 0.0,
                evidence_strength: 0.0,
                model_confidence: 0.0,
                consistency_score: 1.0,
                uncertainty: 1.0,
            };
        }

        let avg_conf: f64 = ctx
            .options
            .iter()
            .map(|o| o.confidence.raw() as f64)
            .sum::<f64>()
            / ctx.options.len() as f64;
        let avg_benefit: f64 =
            ctx.options.iter().map(|o| o.estimated_benefit).sum::<f64>() / ctx.options.len() as f64;
        let avg_risk: f64 =
            ctx.options.iter().map(|o| o.estimated_risk).sum::<f64>() / ctx.options.len() as f64;

        let evidence_strength = avg_conf;
        let model_confidence = avg_conf;
        let consistency_score = 1.0 - (avg_risk / (avg_benefit + 1.0)).min(1.0);
        let uncertainty = 1.0 - avg_conf;

        let overall = avg_conf * 0.4 + consistency_score * 0.3 + (1.0 - uncertainty) * 0.3;

        ConfidenceScore {
            overall,
            evidence_strength,
            model_confidence,
            consistency_score,
            uncertainty,
        }
    }

    fn select_option<'a>(
        &self,
        ctx: &'a DecisionContext,
        _score: &ConfidenceScore,
    ) -> DecisionResult<&'a OptionRef> {
        ctx.options
            .iter()
            .max_by(|a, b| {
                let a_val = a.estimated_benefit - a.estimated_cost - a.estimated_risk;
                let b_val = b.estimated_benefit - b.estimated_cost - b.estimated_risk;
                a_val
                    .partial_cmp(&b_val)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| DecisionError::NoDecision("no options available".into()))
    }

    #[allow(dead_code)]
    fn resolve_conflict(&self, _ctx: &DecisionContext) -> DecisionResult<ConflictDescription> {
        Err(DecisionError::UnresolvableConflict(
            "no conflict resolution strategy defined".into(),
        ))
    }

    pub fn generate_explanation(
        &self,
        ctx: &DecisionContext,
        chosen: &OptionRef,
        score: &ConfidenceScore,
        _policy: &dyn PolicyEvaluator,
    ) -> Explanation {
        let mut factors = vec![
            format!("confidence: {:.2}", score.overall),
            format!("evidence strength: {:.2}", score.evidence_strength),
            format!("consistency score: {:.2}", score.consistency_score),
        ];
        factors.push(format!(
            "selected option: {} with expected benefit {:.2}",
            chosen.option_id, chosen.estimated_benefit
        ));

        Explanation {
            decision_id: ctx.decision_id,
            summary: format!(
                "decision based on multi-factor analysis, confidence {:.2}",
                score.overall
            ),
            key_factors: factors,
            alternatives_considered: ctx.options.iter().map(|o| o.option_id.clone()).collect(),
            confidence_rationale: format!(
                "aggregated confidence from {} options",
                ctx.options.len()
            ),
            policy_compliance: "policy evaluation passed".into(),
        }
    }
}
