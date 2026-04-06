//! Scoring engine with decay calculation.
//!
//! # Weights
//! - interest: 0.40
//! - strategy: 0.35
//! - consensus: 0.25
//!
//! # Decay Formula
//! `decay_factor = 0.5 ^ (days_elapsed / half_life_days)`

use crate::models::{ScoringInput, ScoringOutput};

const WEIGHT_INTEREST: f64 = 0.40;
const WEIGHT_STRATEGY: f64 = 0.35;
const WEIGHT_CONSENSUS: f64 = 0.25;

/// Decay calculator using exponential half-life model.
#[derive(Debug, Clone)]
pub struct DecayCalculator {
    half_life_days: f64,
}

impl DecayCalculator {
    pub fn new(half_life_days: f64) -> Self {
        Self { half_life_days }
    }

    /// Compute decay factor for a given number of days elapsed.
    /// Returns `0.5 ^ (days / half_life)`.
    pub fn factor(&self, days_elapsed: f64) -> f64 {
        if self.half_life_days <= 0.0 {
            return 1.0;
        }
        0.5_f64.powf(days_elapsed / self.half_life_days)
    }
}

/// Scoring engine — computes final scores with multi-dimension decay.
pub struct ScoringEngine;

impl ScoringEngine {
    /// Compute final score with decay applied to each dimension.
    pub fn compute(input: ScoringInput) -> ScoringOutput {
        let (days_elapsed, decay_factor) =
            Self::compute_decay_factor(&input.last_boosted_at);

        // Weighted base score (pre-decay)
        let base_score =
            input.interest * WEIGHT_INTEREST
            + input.strategy * WEIGHT_STRATEGY
            + input.consensus * WEIGHT_CONSENSUS;

        let final_score = base_score * decay_factor;

        ScoringOutput {
            final_score: Self::round2(final_score),
            decay_factor: Self::round6(decay_factor),
            days_elapsed: Self::round2(days_elapsed),
        }
    }

    /// Compute the composite decay factor from all three half-lives.
    fn compute_decay_factor(last_boosted_at: &str) -> (f64, f64) {
        let days_elapsed = Self::days_since(last_boosted_at);

        let decay_interest = DecayCalculator::new(30.0).factor(days_elapsed);
        let decay_strategy = DecayCalculator::new(365.0).factor(days_elapsed);
        let decay_consensus = DecayCalculator::new(60.0).factor(days_elapsed);

        // Composite decay = weighted average of individual decays
        let composite = decay_interest * WEIGHT_INTEREST
            + decay_strategy * WEIGHT_STRATEGY
            + decay_consensus * WEIGHT_CONSENSUS;

        (days_elapsed, composite)
    }

    /// Parse ISO 8601 timestamp and compute days since.
    fn days_since(timestamp: &str) -> f64 {

        let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) else {
            log::warn!("Failed to parse timestamp '{}', assuming 0 days", timestamp);
            return 0.0;
        };
        let dt = dt.with_timezone(&chrono::Utc);

        let now = chrono::Utc::now();
        let duration = now.signed_duration_since(dt);
        duration.num_seconds() as f64 / (24.0 * 3600.0)
    }

    fn round2(v: f64) -> f64 {
        (v * 100.0).round() / 100.0
    }

    fn round6(v: f64) -> f64 {
        (v * 1_000_000.0).round() / 1_000_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decay_half_life_exact() {
        let decay = DecayCalculator::new(30.0);
        let factor = decay.factor(30.0);
        assert!((factor - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_decay_quarter_life() {
        // 15 days ≈ sqrt(0.5) ≈ 0.707
        let decay = DecayCalculator::new(30.0);
        let factor = decay.factor(15.0);
        assert!((factor - 0.7071).abs() < 0.001);
    }

    #[test]
    fn test_final_score_zero_days() {
        let now = chrono::Utc::now().to_rfc3339();
        let input = ScoringInput {
            interest: 10.0,
            strategy: 10.0,
            consensus: 10.0,
            last_boosted_at: now,
            interest_half_life_days: 30.0,
            strategy_half_life_days: 365.0,
            consensus_half_life_days: 60.0,
        };
        let output = ScoringEngine::compute(input);
        // All weights sum to 1.0, decay_factor = 1.0 at 0 days
        assert!((output.final_score - 10.0).abs() < 0.01);
        assert!((output.decay_factor - 1.0).abs() < 0.0001);
    }

    #[test]
    fn test_final_score_formula_weights() {
        let now = chrono::Utc::now().to_rfc3339();
        let input = ScoringInput {
            interest: 8.0,
            strategy: 9.0,
            consensus: 4.0,
            last_boosted_at: now,
            interest_half_life_days: 30.0,
            strategy_half_life_days: 365.0,
            consensus_half_life_days: 60.0,
        };
        let output = ScoringEngine::compute(input);
        // 8*0.4 + 9*0.35 + 4*0.25 = 3.2 + 3.15 + 1.0 = 7.35
        assert!((output.final_score - 7.35).abs() < 0.01);
    }
}
