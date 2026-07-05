//! 评分引擎核心：综合分 + 衰减（修正 v2.x 两处漂移）。
//!
//! 不变量：
//! 1. composite = interest*0.40 + strategy*0.35 + consensus*0.25（默认权重）
//! 2. 衰减只作用于 interest：new = max(interest*floor, interest*rate^days)
//!
//! 注：不变量 2 由 `decay_interest` 的签名结构性保证——它只接收 interest，
//! strategy/consensus 根本不入参，故无法被衰减。

use crate::models::Weights;

/// 计算综合分，范围 [0,100]，四舍五入到 1 位小数。
pub fn composite(interest: f64, strategy: f64, consensus: f64, w: &Weights) -> f64 {
    let v = interest * w.interest + strategy * w.strategy + consensus * w.consensus;
    let rounded = (v * 10.0).round() / 10.0;
    rounded.clamp(0.0, 100.0)
}

/// interest 维度衰减。每日 rate，地板 floor（相对初始值）。
/// strategy / consensus 不衰减（由签名结构性保证）。
/// days_inactive <= 0 视为无衰减，返回原值。
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
        assert!(w.is_normalized(), "默认权重应归一，和为 {}", w.sum());
    }

    #[test]
    fn test_composite_default_weights() {
        // 8*0.4 + 10*0.35 + 4*0.25 = 3.2 + 3.5 + 1.0 = 7.7（避开 .x5 舍入边界）
        let c = composite(8.0, 10.0, 4.0, &w());
        assert!((c - 7.7).abs() < 1e-6, "composite 应为 7.7，实际 {c}");
    }

    #[test]
    fn test_composite_custom_weights_override() {
        // F4: 自定义归一权重覆盖默认
        let w = Weights { interest: 0.5, strategy: 0.3, consensus: 0.2 };
        assert!(w.is_normalized());
        // 8*0.5 + 0*0.3 + 0*0.2 = 4.0（默认权重下会是 8*0.4=3.2，证明覆盖生效）
        let c = composite(8.0, 0.0, 0.0, &w);
        assert!((c - 4.0).abs() < 1e-6, "自定义权重下应为 4.0，实际 {c}");
    }

    #[test]
    fn test_composite_at_max_boundary() {
        // (100,100,100) + 归一权重 正好 100
        let c = composite(100.0, 100.0, 100.0, &w());
        assert!((c - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_composite_clamps_overflow() {
        // F4: 真正测超出上界——非归一权重(和=1.5)使 (100,100,100)->150，clamp 到 100
        let w = Weights { interest: 0.5, strategy: 0.5, consensus: 0.5 };
        assert!(!w.is_normalized());
        let c = composite(100.0, 100.0, 100.0, &w);
        assert!((c - 100.0).abs() < 1e-6, "超出上界应被 clamp 到 100，实际 {c}");
    }

    #[test]
    fn test_composite_clamps_negative() {
        // F4: 下界 clamp——负输入归一加权后为负，clamp 到 0
        let c = composite(-50.0, -50.0, -50.0, &w());
        assert!((c - 0.0).abs() < 1e-6, "负值应被 clamp 到 0，实际 {c}");
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
    fn test_decay_zero_days_noop() {
        let new = decay_interest(85.0, 0, 0.98, 0.5);
        assert!((new - 85.0).abs() < 1e-9);
    }

    #[test]
    fn test_decay_negative_days_noop() {
        // F4: 负天数边界——应视为 0，返回原值
        let new = decay_interest(85.0, -5, 0.98, 0.5);
        assert!((new - 85.0).abs() < 1e-9, "负天数应返回原值，实际 {new}");
    }

    #[test]
    fn test_weights_is_normalized_detects_bad() {
        // F3 配套：is_normalized 能识别非归一权重
        assert!(Weights::default().is_normalized());
        assert!(!Weights { interest: 0.5, strategy: 0.5, consensus: 0.5 }.is_normalized()); // 和=1.5 不归一
    }
}