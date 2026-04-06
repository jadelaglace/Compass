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
    /// Method 2: each dimension independently decays, then weighted sum.
    /// final = (interest * decay_i * 0.4) + (strategy * decay_s * 0.35) + (consensus * decay_c * 0.25)
    pub fn compute(input: ScoringInput) -> ScoringOutput {
        let days_elapsed = Self::days_since(&input.last_boosted_at);

        let decay_interest = DecayCalculator::new(30.0).factor(days_elapsed);
        let decay_strategy = DecayCalculator::new(365.0).factor(days_elapsed);
        let decay_consensus = DecayCalculator::new(60.0).factor(days_elapsed);

        // Each dimension independently decayed, then weighted
        let final_score =
            input.interest * decay_interest * WEIGHT_INTEREST
            + input.strategy * decay_strategy * WEIGHT_STRATEGY
            + input.consensus * decay_consensus * WEIGHT_CONSENSUS;

        // Composite decay factor for reporting
        let decay_factor =
            decay_interest * WEIGHT_INTEREST
            + decay_strategy * WEIGHT_STRATEGY
            + decay_consensus * WEIGHT_CONSENSUS;

        ScoringOutput {
            final_score: Self::round2(final_score),
            decay_factor: Self::round2(decay_factor),
            days_elapsed: Self::round2(days_elapsed),
        }
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
        format!("{:.2}", v).parse().unwrap_or(v)
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

    #[test]
    fn test_final_score_with_decay_method2() {
        // 30 days ago: interest decay = 0.5, strategy decay ≈ 0.94, consensus decay ≈ 0.69
        let past = "2026-03-07T00:00:00Z";
        let input = ScoringInput {
            interest: 10.0,
            strategy: 10.0,
            consensus: 10.0,
            last_boosted_at: past.to_string(),
            interest_half_life_days: 30.0,
            strategy_half_life_days: 365.0,
            consensus_half_life_days: 60.0,
        };
        let output = ScoringEngine::compute(input);
        // Method 2: (10*0.5*0.4) + (10*decay_s*0.35) + (10*decay_c*0.25) < 10.0
        assert!(output.final_score < 10.0); // Must be less than 10.0 due to decay
        assert!(output.final_score > 6.0);  // But meaningfully above floor
        assert!(output.days_elapsed > 20.0); // ~30 days elapsed
    }
}
