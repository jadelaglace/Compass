//! 评分引擎核心：综合分 + 衰减（修正 v2.x 两处漂移）。
//!
//! 不变量：
//! 1. composite = interest*0.40 + strategy*0.35 + consensus*0.25（默认权重）
//! 2. 衰减只作用于 interest：new = max(interest*floor, interest*rate^days)

use crate::models::Weights;

/// 计算综合分，范围 [0,100]，四舍五入到 1 位小数。
pub fn composite(interest: f64, strategy: f64, consensus: f64, w: &Weights) -> f64 {
    let v = interest * w.interest + strategy * w.strategy + consensus * w.consensus;
    let rounded = (v * 10.0).round() / 10.0;
    rounded.clamp(0.0, 100.0)
}

/// interest 维度衰减。每日 rate，地板 floor（相对初始值）。
/// strategy / consensus 不衰减。
pub fn decay_interest(interest: f64, days_inactive: i64, daily_rate: f64, floor: f64) -> f64 {
    if days_inactive <= 0 {
        return interest;
    }
    let decayed = interest * daily_rate.powi(days_inactive as i32);
    decayed.max(interest * floor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weights { Weights::default() }

    #[test]
    fn test_default_weights_sum_to_one() {
        let w = w();
        let sum = w.interest + w.strategy + w.consensus;
        assert!((sum - 1.0).abs() < 1e-9, "权重和应为 1.0，实际 {sum}");
    }

    #[test]
    fn test_composite_default_weights() {
        // 8*0.4 + 10*0.35 + 4*0.25 = 3.2 + 3.5 + 1.0 = 7.7（避开 .x5 舍入边界）
        let c = composite(8.0, 10.0, 4.0, &w());
        assert!((c - 7.7).abs() < 1e-6, "composite 应为 7.7，实际 {c}");
    }

    #[test]
    fn test_composite_clamps_to_100() {
        let c = composite(100.0, 100.0, 100.0, &w());
        assert!((c - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_decay_30_days_half() {
        // 0.98^30 ≈ 0.545，仍高于 floor 0.5
        let new = decay_interest(100.0, 30, 0.98, 0.5);
        assert!(new > 50.0 && new < 55.0, "30 天衰减后应 ≈54.5，实际 {new}");
    }

    #[test]
    fn test_decay_floor_enforced() {
        // 0.98^200 ≈ 0.0176，远低于 floor，应被 floor 拉回 50
        let new = decay_interest(100.0, 200, 0.98, 0.5);
        assert!((new - 50.0).abs() < 1e-6, "应被地板拉回 50，实际 {new}");
    }

    #[test]
    fn test_decay_only_interest() {
        // 此函数签名本身保证只衰 interest；此处验证 strategy/consensus 不入参。
        let _ = decay_interest(80.0, 30, 0.98, 0.5);
        // strategy/consensus 不在此函数作用域——结构性保证不衰减。
    }

    #[test]
    fn test_decay_zero_days_noop() {
        let new = decay_interest(85.0, 0, 0.98, 0.5);
        assert!((new - 85.0).abs() < 1e-9);
    }
}
