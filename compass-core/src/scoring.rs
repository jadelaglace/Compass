//! 评分引擎核心：综合分 + 衰减 + 触发器（T1.1 + T1.3）。
//!
//! 不变量：
//! 1. composite = interest*0.40 + strategy*0.35 + consensus*0.25（默认权重）
//! 2. 衰减只作用于 interest：new = max(interest*floor, interest*rate^days)
//! 3. 触发器/访问 boost 只增不减（除衰减外），各维度 clamp 到 [0,100]

use anyhow::Result;
use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::models::{Score, Weights};

// ============ 综合分（T1.1） ============

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

// ============ 触发器与访问深度（T1.3，PRD §5.3） ============

/// 评分触发器类型（PRD §5.3 触发器表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Trigger {
    /// 被引用 → consensus +2，冷却 1 天
    Cited,
    /// 创建关联链接 → interest +1，冷却 7 天
    Linked,
    /// 添加案例（理论被实践验证）→ strategy +3，无冷却
    CaseAdded,
    /// 手动标记重点 → interest +10，无冷却
    ManualMark,
    /// 完成复习 → consensus +2，冷却 7 天
    ReviewCompleted,
}

impl Trigger {
    /// 返回 (interest, strategy, consensus) 的调整值。
    pub fn deltas(&self) -> (f64, f64, f64) {
        match self {
            Trigger::Cited => (0.0, 0.0, 2.0),
            Trigger::Linked => (1.0, 0.0, 0.0),
            Trigger::CaseAdded => (0.0, 3.0, 0.0),
            Trigger::ManualMark => (10.0, 0.0, 0.0),
            Trigger::ReviewCompleted => (0.0, 0.0, 2.0),
        }
    }

    /// 冷却期（天）；None 表示无冷却。
    pub fn cooldown_days(&self) -> Option<i64> {
        match self {
            Trigger::Cited => Some(1),
            Trigger::Linked => Some(7),
            Trigger::CaseAdded => None,
            Trigger::ManualMark => None,
            Trigger::ReviewCompleted => Some(7),
        }
    }
}

/// 访问深度（PRD §5.3 访问深度 boost）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum AccessDepth {
    /// 浏览标题：interest +0，consensus +0.1
    Glance,
    /// 完整阅读：interest +1，consensus +0.5（默认）
    #[default]
    Read,
    /// 深度学习：interest +3，consensus +1
    Study,
    /// 实际应用：interest +2，strategy +5，consensus +2
    Apply,
}

impl AccessDepth {
    /// 返回 (interest, strategy, consensus) 的调整值。
    pub fn deltas(&self) -> (f64, f64, f64) {
        match self {
            AccessDepth::Glance => (0.0, 0.0, 0.1),
            AccessDepth::Read => (1.0, 0.0, 0.5),
            AccessDepth::Study => (3.0, 0.0, 1.0),
            AccessDepth::Apply => (2.0, 5.0, 2.0),
        }
    }
}

/// 应用触发器 boost，返回新 Score（维度调整 + composite 重算 + last_boosted_at 更新）。
/// **不做冷却检查**——冷却由调用方根据 per-type 历史用 `in_cooldown` 判断（历史存于 score_history，T1.4）。
/// 便捷封装见 `apply_trigger_if_eligible`。
/// `now` 须为合法 RFC3339 时间戳（本函数不校验，仅存入）。
pub fn apply_trigger(score: &Score, trigger: Trigger, now: &str) -> Score {
    let (di, ds, dc) = trigger.deltas();
    let w = score.weights.unwrap_or_default();
    let mut new = score.clone();
    new.interest = clamp_score(new.interest + di);
    new.strategy = clamp_score(new.strategy + ds);
    new.consensus = clamp_score(new.consensus + dc);
    new.last_boosted_at = now.to_string();
    new.updated_at = now.to_string();
    new.composite = composite(new.interest, new.strategy, new.consensus, &w);
    new
}

/// 应用访问深度 boost，返回新 Score（维度调整 + access_count+1 + composite 重算）。
pub fn apply_access(score: &Score, depth: AccessDepth, now: &str) -> Score {
    let (di, ds, dc) = depth.deltas();
    let w = score.weights.unwrap_or_default();
    let mut new = score.clone();
    new.interest = clamp_score(new.interest + di);
    new.strategy = clamp_score(new.strategy + ds);
    new.consensus = clamp_score(new.consensus + dc);
    new.access_count += 1;
    new.last_boosted_at = now.to_string();
    new.updated_at = now.to_string();
    new.composite = composite(new.interest, new.strategy, new.consensus, &w);
    new
}

/// 应用触发器 boost，但先检查冷却：若 `last_for_type`（该触发器类型上次触发时间，
/// 来自 score_history T1.4）存在且仍在冷却期，返回 `None`（应跳过）；否则 apply。
/// `last_for_type` = `None` 表示该类型从未触发，直接 apply。无冷却期的触发器总是 apply。
pub fn apply_trigger_if_eligible(
    score: &Score,
    trigger: Trigger,
    now: &str,
    last_for_type: Option<&str>,
) -> Result<Option<Score>> {
    if let (Some(cd), Some(last)) = (trigger.cooldown_days(), last_for_type) {
        if in_cooldown(last, now, cd)? {
            return Ok(None);
        }
    }
    Ok(Some(apply_trigger(score, trigger, now)))
}

/// 冷却检查：`last_triggered` 距 `now` 不足 `cooldown_days` 天则仍在冷却期（返回 true，应跳过）。
/// 时间为 RFC3339。elapsed 用整数天（截断：23h=0, 25h=1），
/// 故 1 天冷却 = 满 24h 后可触发。
pub fn in_cooldown(last_triggered: &str, now: &str, cooldown_days: i64) -> Result<bool> {
    let last = parse_ts(last_triggered)?;
    let now = parse_ts(now)?;
    let elapsed = (now - last).num_days();
    Ok(elapsed < cooldown_days)
}

fn parse_ts(s: &str) -> Result<DateTime<chrono::Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow::anyhow!("解析时间失败 {s}: {e}"))?
        .with_timezone(&chrono::Utc))
}

fn clamp_score(v: f64) -> f64 {
    v.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weights {
        Weights::default()
    }

    fn base_score() -> Score {
        Score {
            interest: 5.0,
            strategy: 5.0,
            consensus: 5.0,
            composite: 5.0,
            weights: None,
            updated_at: "2026-01-01T00:00:00Z".into(),
            last_boosted_at: "2026-01-01T00:00:00Z".into(),
            access_count: 0,
        }
    }

    // ---- T1.1 综合分/衰减 ----

    #[test]
    fn test_default_weights_sum_to_one() {
        let w = w();
        assert!(w.is_normalized(), "默认权重应归一，和为 {}", w.sum());
    }

    #[test]
    fn test_composite_default_weights() {
        let c = composite(8.0, 10.0, 4.0, &w());
        assert!((c - 7.7).abs() < 1e-6, "composite 应为 7.7，实际 {c}");
    }

    #[test]
    fn test_composite_custom_weights_override() {
        let w = Weights {
            interest: 0.5,
            strategy: 0.3,
            consensus: 0.2,
        };
        assert!(w.is_normalized());
        let c = composite(8.0, 0.0, 0.0, &w);
        assert!((c - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_composite_clamps_overflow() {
        let w = Weights {
            interest: 0.5,
            strategy: 0.5,
            consensus: 0.5,
        };
        let c = composite(100.0, 100.0, 100.0, &w);
        assert!((c - 100.0).abs() < 1e-6);
    }

    #[test]
    fn test_composite_clamps_negative() {
        let c = composite(-50.0, -50.0, -50.0, &w());
        assert!((c - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_decay_30_days_half() {
        let new = decay_interest(100.0, 30, 0.98, 0.5);
        assert!(new > 50.0 && new < 55.0);
    }

    #[test]
    fn test_decay_floor_enforced() {
        let new = decay_interest(100.0, 200, 0.98, 0.5);
        assert!((new - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_decay_zero_days_noop() {
        let new = decay_interest(85.0, 0, 0.98, 0.5);
        assert!((new - 85.0).abs() < 1e-9);
    }

    #[test]
    fn test_decay_negative_days_noop() {
        let new = decay_interest(85.0, -5, 0.98, 0.5);
        assert!((new - 85.0).abs() < 1e-9);
    }

    #[test]
    fn test_weights_is_normalized_detects_bad() {
        assert!(Weights::default().is_normalized());
        assert!(!Weights {
            interest: 0.5,
            strategy: 0.5,
            consensus: 0.5
        }
        .is_normalized());
    }

    // ---- T1.3 触发器 deltas / 冷却 ----

    #[test]
    fn test_trigger_deltas() {
        assert_eq!(Trigger::Cited.deltas(), (0.0, 0.0, 2.0));
        assert_eq!(Trigger::Linked.deltas(), (1.0, 0.0, 0.0));
        assert_eq!(Trigger::CaseAdded.deltas(), (0.0, 3.0, 0.0));
        assert_eq!(Trigger::ManualMark.deltas(), (10.0, 0.0, 0.0));
        assert_eq!(Trigger::ReviewCompleted.deltas(), (0.0, 0.0, 2.0));
    }

    #[test]
    fn test_trigger_cooldown_days() {
        assert_eq!(Trigger::Cited.cooldown_days(), Some(1));
        assert_eq!(Trigger::Linked.cooldown_days(), Some(7));
        assert_eq!(Trigger::CaseAdded.cooldown_days(), None);
        assert_eq!(Trigger::ManualMark.cooldown_days(), None);
        assert_eq!(Trigger::ReviewCompleted.cooldown_days(), Some(7));
    }

    #[test]
    fn test_access_deltas() {
        assert_eq!(AccessDepth::Glance.deltas(), (0.0, 0.0, 0.1));
        assert_eq!(AccessDepth::Read.deltas(), (1.0, 0.0, 0.5));
        assert_eq!(AccessDepth::Study.deltas(), (3.0, 0.0, 1.0));
        assert_eq!(AccessDepth::Apply.deltas(), (2.0, 5.0, 2.0));
    }

    // ---- T1.3 apply_trigger ----

    #[test]
    fn test_apply_trigger_cited() {
        let s = apply_trigger(&base_score(), Trigger::Cited, "2026-07-06T00:00:00Z");
        assert!((s.consensus - 7.0).abs() < 1e-9, "consensus 应 +2 = 7");
        assert!((s.interest - 5.0).abs() < 1e-9);
        assert!((s.strategy - 5.0).abs() < 1e-9);
        assert_eq!(s.last_boosted_at, "2026-07-06T00:00:00Z");
        // composite 重算：5*0.4+5*0.35+7*0.25 = 2+1.75+1.75 = 5.5
        assert!((s.composite - 5.5).abs() < 1e-6);
    }

    #[test]
    fn test_apply_trigger_manual_mark() {
        let s = apply_trigger(&base_score(), Trigger::ManualMark, "2026-07-06T00:00:00Z");
        assert!((s.interest - 15.0).abs() < 1e-9, "interest 应 +10 = 15");
    }

    #[test]
    fn test_apply_trigger_clamps_to_100() {
        let mut s = base_score();
        s.interest = 95.0;
        let s = apply_trigger(&s, Trigger::ManualMark, "2026-07-06T00:00:00Z");
        assert!((s.interest - 100.0).abs() < 1e-9, "95+10 应 clamp 到 100");
    }

    #[test]
    fn test_apply_trigger_case_added() {
        let s = apply_trigger(&base_score(), Trigger::CaseAdded, "2026-07-06T00:00:00Z");
        assert!((s.strategy - 8.0).abs() < 1e-9, "strategy 应 +3 = 8");
    }

    #[test]
    fn test_apply_trigger_preserves_and_uses_custom_weights() {
        let mut s = base_score();
        s.weights = Some(Weights {
            interest: 0.5,
            strategy: 0.3,
            consensus: 0.2,
        });
        let s = apply_trigger(&s, Trigger::Linked, "2026-07-06T00:00:00Z");
        // interest +1 = 6；composite 用自定义权重：6*0.5+5*0.3+5*0.2 = 3+1.5+1 = 5.5
        assert!((s.interest - 6.0).abs() < 1e-9);
        assert!(s.weights.is_some(), "weights 应保留");
        assert!((s.composite - 5.5).abs() < 1e-6);
    }

    #[test]
    fn test_apply_trigger_does_not_change_access_count() {
        let s = apply_trigger(&base_score(), Trigger::Cited, "2026-07-06T00:00:00Z");
        assert_eq!(s.access_count, 0, "trigger 不应改 access_count");
    }

    // ---- T1.3 apply_access ----

    #[test]
    fn test_apply_access_study() {
        let s = apply_access(&base_score(), AccessDepth::Study, "2026-07-06T00:00:00Z");
        assert!((s.interest - 8.0).abs() < 1e-9, "interest +3 = 8");
        assert!((s.consensus - 6.0).abs() < 1e-9, "consensus +1 = 6");
        assert_eq!(s.access_count, 1, "access_count 应 +1");
    }

    #[test]
    fn test_apply_access_apply() {
        let s = apply_access(&base_score(), AccessDepth::Apply, "2026-07-06T00:00:00Z");
        assert!((s.interest - 7.0).abs() < 1e-9);
        assert!((s.strategy - 10.0).abs() < 1e-9);
        assert!((s.consensus - 7.0).abs() < 1e-9);
        assert_eq!(s.access_count, 1);
    }

    #[test]
    fn test_apply_access_glance() {
        let s = apply_access(&base_score(), AccessDepth::Glance, "2026-07-06T00:00:00Z");
        assert!((s.interest - 5.0).abs() < 1e-9, "glance interest +0");
        assert!((s.consensus - 5.1).abs() < 1e-6, "consensus +0.1 = 5.1");
        assert_eq!(s.access_count, 1);
    }

    // ---- T1.3 in_cooldown ----

    #[test]
    fn test_in_cooldown_within() {
        // last 12 小时前，冷却 1 天 → 仍在冷却
        let r = in_cooldown("2026-07-05T12:00:00Z", "2026-07-06T00:00:00Z", 1).unwrap();
        assert!(r, "12 小时 < 1 天，应在冷却期");
    }

    #[test]
    fn test_in_cooldown_expired() {
        // last 2 天前，冷却 1 天 → 已过冷却
        let r = in_cooldown("2026-07-04T00:00:00Z", "2026-07-06T00:00:00Z", 1).unwrap();
        assert!(!r, "2 天 >= 1 天，应过冷却");
    }

    #[test]
    fn test_in_cooldown_7day() {
        // last 3 天前，冷却 7 天 → 仍在冷却
        let r = in_cooldown("2026-07-03T00:00:00Z", "2026-07-06T00:00:00Z", 7).unwrap();
        assert!(r);
    }

    #[test]
    fn test_in_cooldown_bad_timestamp() {
        assert!(in_cooldown("not-a-time", "2026-07-06T00:00:00Z", 1).is_err());
    }

    // ---- F1: apply_trigger_if_eligible ----

    #[test]
    fn test_eligible_in_cooldown_returns_none() {
        // Cited 冷却 1 天；last 12h 前 → 冷却，None
        let r = apply_trigger_if_eligible(
            &base_score(),
            Trigger::Cited,
            "2026-07-06T00:00:00Z",
            Some("2026-07-05T12:00:00Z"),
        )
        .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn test_eligible_expired_applies() {
        // last 2 天前，冷却 1 天 → 过冷却，apply
        let r = apply_trigger_if_eligible(
            &base_score(),
            Trigger::Cited,
            "2026-07-06T00:00:00Z",
            Some("2026-07-04T00:00:00Z"),
        )
        .unwrap();
        let s = r.unwrap();
        assert!((s.consensus - 7.0).abs() < 1e-9);
    }

    #[test]
    fn test_eligible_no_cooldown_type_always_applies() {
        // ManualMark 无冷却，即使 last 提供也 apply
        let r = apply_trigger_if_eligible(
            &base_score(),
            Trigger::ManualMark,
            "2026-07-06T00:00:00Z",
            Some("2026-07-05T00:00:00Z"),
        )
        .unwrap();
        assert!(r.is_some());
    }

    #[test]
    fn test_eligible_never_triggered_applies() {
        // last_for_type None → apply
        let r =
            apply_trigger_if_eligible(&base_score(), Trigger::Cited, "2026-07-06T00:00:00Z", None)
                .unwrap();
        assert!(r.is_some());
    }

    // ---- F5: apply_access clamp ----

    #[test]
    fn test_apply_access_clamps_to_100() {
        let mut s = base_score();
        s.strategy = 98.0;
        let s = apply_access(&s, AccessDepth::Apply, "2026-07-06T00:00:00Z");
        assert!((s.strategy - 100.0).abs() < 1e-9, "98+5 应 clamp 到 100");
    }
}
