//! Shared data structures for Rust-Python communication via JSON-RPC.
//!
//! All structures are serialized to/from JSON for subprocess communication.

use serde::{Deserialize, Serialize};

/// Input for scoring computation.
/// Passed via JSON-RPC `params` field.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScoringInput {
    pub interest: f64,
    pub strategy: f64,
    pub consensus: f64,
    /// ISO 8601 timestamp of last boost event.
    pub last_boosted_at: String,
    /// Half-life in days for interest decay. Default: 30.0
    #[serde(default = "default_interest_half_life")]
    pub interest_half_life_days: f64,
    /// Half-life in days for strategy decay. Default: 365.0
    #[serde(default = "default_strategy_half_life")]
    pub strategy_half_life_days: f64,
    /// Half-life in days for consensus decay. Default: 60.0
    #[serde(default = "default_consensus_half_life")]
    pub consensus_half_life_days: f64,
}

fn default_interest_half_life() -> f64 { 30.0 }
fn default_strategy_half_life() -> f64 { 365.0 }
fn default_consensus_half_life() -> f64 { 60.0 }

/// Output from scoring computation.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ScoringOutput {
    pub final_score: f64,
    pub decay_factor: f64,
    pub days_elapsed: f64,
}

/// Input for reference parsing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ReferenceInput {
    /// Raw markdown content to extract [[id]] references from.
    pub content: String,
    /// Optional: filter out self-references to this entity ID.
    pub current_entity_id: Option<String>,
}

/// Output from reference parsing.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ReferenceOutput {
    pub refs: Vec<String>,
}
