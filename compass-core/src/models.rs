//! 共享数据结构。

use serde::{Deserialize, Serialize};

/// 三维权重（默认 0.40 / 0.35 / 0.25）。
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct Weights {
    pub interest: f64,
    pub strategy: f64,
    pub consensus: f64,
}

impl Weights {
    /// 权重和是否归一（== 1.0，容差 1e-6）。
    pub fn is_normalized(&self) -> bool {
        (self.interest + self.strategy + self.consensus - 1.0).abs() < 1e-6
    }

    /// 权重和。
    pub fn sum(&self) -> f64 {
        self.interest + self.strategy + self.consensus
    }
}

impl Default for Weights {
    fn default() -> Self {
        Self { interest: 0.40, strategy: 0.35, consensus: 0.25 }
    }
}

/// 实体层级（三大界）。
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    Direction,
    Knowledge,
    Case,
    Log,
    Insight,
}

/// frontmatter 中的 score 块（Compass 写回，Dataview 可读）。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Score {
    pub interest: f64,
    pub strategy: f64,
    pub consensus: f64,
    pub composite: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weights: Option<Weights>,
    pub updated_at: String,
    pub last_boosted_at: String,
    #[serde(default)]
    pub access_count: i64,
}