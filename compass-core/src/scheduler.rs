//! 衰减调度器（T2.1）：tokio 定时每日 02:00 对所有实体执行 interest 衰减。
//!
//! 衰减规格（PRD §5.2）：
//! - new_interest = max(interest * floor, interest * daily_rate ^ days_inactive)
//! - 只衰 interest，strategy/consensus 不衰减
//! - 跳过条件：status=archived；last_boosted_at 距今 < boost_protection_days；layer=direction 衰减减半
//! - 衰减后重算 composite，写回 frontmatter，更新 SQLite，记录 score_history

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use chrono::{DateTime, Duration, TimeZone, Utc};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::config::DecayConfig;
use crate::db::{Db, EntityRow, ScoreHistoryRow};
use crate::frontmatter;
use crate::models::{Score, Weights};
use crate::scoring;

/// 衰减调度器
pub struct DecayScheduler {
    db: Arc<Mutex<Db>>,
    vault: PathBuf,
    weights: Weights,
    decay: DecayConfig,
}

/// 衰减变化（纯计算结果）
pub(crate) struct DecayChange {
    new_interest: f64,
    days_inactive: i64,
}

pub(crate) enum DecayOutcome {
    Decayed,
    Skipped,
}

/// 单次衰减执行结果
#[derive(Debug, Default, Clone)]
pub struct DecayResult {
    pub total: u32,
    pub decayed: u32,
    pub skipped_archived: u32,
    pub skipped_boost_protection: u32,
    pub errors: u32,
}

impl DecayScheduler {
    pub fn new(db: Arc<Mutex<Db>>, vault: PathBuf, weights: Weights, decay: DecayConfig) -> Self {
        Self {
            db,
            vault,
            weights,
            decay,
        }
    }

    /// 启动定时调度（每日 02:00 执行）
    pub async fn start(&self) -> Result<()> {
        info!("DecayScheduler started (daily at 02:00)");
        let db = self.db.clone();
        let vault = self.vault.clone();
        let weights = self.weights;
        let decay = self.decay.clone();

        tokio::spawn(async move {
            loop {
                // 计算到下次 02:00 的间隔
                let now = Utc::now();
                let next = next_run_time(now);
                let wait = (next - now).to_std().unwrap_or_default();
                info!(
                    next_run = %next.format("%Y-%m-%d %H:%M:%S UTC"),
                    wait_secs = wait.as_secs(),
                    "next decay scheduled"
                );
                tokio::time::sleep(wait).await;

                let scheduler =
                    DecayScheduler::new(db.clone(), vault.clone(), weights, decay.clone());
                match scheduler.run_once().await {
                    Ok(result) => {
                        info!(
                            total = result.total,
                            decayed = result.decayed,
                            skipped_archived = result.skipped_archived,
                            skipped_boost = result.skipped_boost_protection,
                            errors = result.errors,
                            "decay run complete"
                        );
                    }
                    Err(e) => {
                        warn!(err = %e, "decay run failed");
                    }
                }
            }
        });
        Ok(())
    }

    /// 执行一次衰减（可手动触发，用于测试）
    pub async fn run_once(&self) -> Result<DecayResult> {
        let now = Utc::now();
        let mut result = DecayResult::default();
        let db = self.db.lock().await;
        let entities = db.list_entities()?;
        result.total = entities.len() as u32;

        for entity in entities {
            // 跳过 archived
            if entity.status.as_deref() == Some("archived") {
                result.skipped_archived += 1;
                continue;
            }
            match self.process_one(&db, &entity, now) {
                Ok(DecayOutcome::Decayed) => result.decayed += 1,
                Ok(DecayOutcome::Skipped) => result.skipped_boost_protection += 1,
                Err(e) => {
                    warn!(id = %entity.id, err = %e, "decay failed");
                    result.errors += 1;
                }
            }
        }
        Ok(result)
    }

    /// 处理单个实体：读 frontmatter -> 计算衰减 -> 写回 + 更新 db + 历史
    fn process_one(&self, db: &Db, entity: &EntityRow, now: DateTime<Utc>) -> Result<DecayOutcome> {
        let file_path = self.vault.join(&entity.file_path);
        let note = frontmatter::read_note(&file_path)?;
        let mut score = frontmatter::get_score(&note.frontmatter)?
            .ok_or_else(|| anyhow::anyhow!("笔记无 score 块: {}", entity.id))?;

        let change = match self.compute_decay(entity, &score, now)? {
            Some(c) => c,
            None => return Ok(DecayOutcome::Skipped),
        };

        let old_interest = score.interest;
        score.interest = change.new_interest;
        let w = score.weights.unwrap_or(self.weights);
        score.composite = scoring::composite(score.interest, score.strategy, score.consensus, &w);
        let now_str = now.to_rfc3339();
        score.updated_at = now_str.clone();

        frontmatter::write_score(&file_path, &score)?;
        let _ = db.insert_score_history(&ScoreHistoryRow {
            entity_id: entity.id.clone(),
            dimension: Some("interest".to_string()),
            old: Some(old_interest),
            new: Some(change.new_interest),
            reason: Some("decay".to_string()),
            trigger: Some("Decay".to_string()),
            created_at: now_str.clone(),
        });
        let updated = EntityRow {
            id: entity.id.clone(),
            file_path: entity.file_path.clone(),
            title: entity.title.clone(),
            layer: entity.layer.clone(),
            status: entity.status.clone(),
            interest: Some(score.interest),
            strategy: Some(score.strategy),
            consensus: Some(score.consensus),
            composite: Some(score.composite),
            access_count: score.access_count,
            last_boosted_at: Some(score.last_boosted_at.clone()),
            content_hash: entity.content_hash.clone(),
            updated_at: Some(score.updated_at.clone()),
        };
        db.upsert_entity(&updated, &note.body)?;
        info!(id = %entity.id, old = old_interest, new = change.new_interest, days = change.days_inactive, composite = score.composite, "decayed");
        Ok(DecayOutcome::Decayed)
    }

    /// 计算衰减变化（纯计算，不碰 db/frontmatter）。返回 Some(DecayChange) 表示需衰减。
    fn compute_decay(
        &self,
        entity: &EntityRow,
        score: &Score,
        now: DateTime<Utc>,
    ) -> Result<Option<DecayChange>> {
        if entity.status.as_deref() == Some("archived") {
            return Ok(None);
        }
        let last_boosted = parse_rfc3339(&score.last_boosted_at)?;
        let days_inactive = (now - last_boosted).num_days();
        if days_inactive < self.decay.boost_protection_days {
            return Ok(None);
        }
        let is_direction = entity.layer.as_deref() == Some("direction");
        let effective_rate = if is_direction {
            self.decay
                .daily_rate
                .powf(self.decay.direction_layer_factor)
        } else {
            self.decay.daily_rate
        };
        let new_interest = scoring::decay_interest(
            score.interest,
            days_inactive,
            effective_rate,
            self.decay.floor,
        );
        if (new_interest - score.interest).abs() < 1e-9 {
            return Ok(None);
        }
        Ok(Some(DecayChange {
            new_interest,
            days_inactive,
        }))
    }
}

/// 计算下次 02:00 UTC 的时间
fn next_run_time(now: DateTime<Utc>) -> DateTime<Utc> {
    let next = now.date_naive().and_hms_opt(2, 0, 0).unwrap();
    let next = Utc.from_utc_datetime(&next);
    if next > now {
        next
    } else {
        next + Duration::days(1)
    }
}

fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(s)
        .map_err(|e| anyhow::anyhow!("解析时间失败 {s}: {e}"))?
        .with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup(vault: &std::path::Path) -> (DecayScheduler, Arc<Mutex<Db>>) {
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let scheduler = DecayScheduler::new(
            db.clone(),
            vault.to_path_buf(),
            Weights::default(),
            DecayConfig {
                daily_rate: 0.98,
                floor: 0.5,
                boost_protection_days: 3,
                direction_layer_factor: 0.5,
            },
        );
        (scheduler, db)
    }

    fn md_with_score(
        id: &str,
        interest: f64,
        last_boosted: &str,
        layer: &str,
        status: &str,
    ) -> String {
        format!(
            "---\nid: {id}\ntitle: Test\nlayer: {layer}\nstatus: {status}\nscore:\n  interest: {interest}\n  strategy: 50.0\n  consensus: 50.0\n  composite: 50.0\n  updated_at: '2026-07-01T00:00:00Z'\n  last_boosted_at: '{last_boosted}'\n  access_count: 0\n---\nbody\n"
        )
    }

    async fn insert_entity(
        db: &Arc<Mutex<Db>>,
        id: &str,
        fp: &str,
        interest: f64,
        last_boosted: &str,
        layer: &str,
    ) {
        let db = db.lock().await;
        let entity = EntityRow {
            id: id.to_string(),
            file_path: fp.to_string(),
            title: Some("T".to_string()),
            layer: Some(layer.to_string()),
            status: Some("active".to_string()),
            interest: Some(interest),
            strategy: Some(50.0),
            consensus: Some(50.0),
            composite: Some(50.0),
            access_count: 0,
            last_boosted_at: Some(last_boosted.to_string()),
            content_hash: Some("abc".to_string()),
            updated_at: Some("2026-07-01T00:00:00Z".to_string()),
        };
        db.upsert_entity(&entity, "body").unwrap();
    }

    #[tokio::test]
    async fn test_decay_reduces_interest() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // last_boosted 30 天前
        fs::write(
            vault.join("know-1.md"),
            md_with_score(
                "know-1",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "active",
            ),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "knowledge",
        )
        .await;

        // now = 2026-07-01
        let now = DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        let result = scheduler.process_one(&db_guard, &entity, now).unwrap();
        assert!(matches!(result, DecayOutcome::Decayed));

        // 验证 interest 下降
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        let new_interest = entity.interest.unwrap();
        assert!(new_interest < 80.0, "interest 应下降");
        // 80 * 0.98^30 = 80 * 0.5455 = 43.6
        assert!(
            (new_interest - 43.64).abs() < 0.5,
            "interest 约为 43.6，实际 {new_interest}"
        );
    }

    #[tokio::test]
    async fn test_decay_skips_archived() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        fs::write(
            vault.join("know-1.md"),
            md_with_score(
                "know-1",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "archived",
            ),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "knowledge",
        )
        .await;
        // db 中 status 改 archived
        let db_guard = db.lock().await;
        let mut entity = db_guard.get_entity("know-1").unwrap().unwrap();
        entity.status = Some("archived".to_string());
        db_guard.upsert_entity(&entity, "body").unwrap();

        let now = Utc::now();
        let result = scheduler.process_one(&db_guard, &entity, now).unwrap();
        assert!(matches!(result, DecayOutcome::Skipped));
    }

    #[tokio::test]
    async fn test_decay_skips_boost_protection() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // last_boosted 1 天前（< boost_protection_days=3）
        let recent = Utc::now() - Duration::days(1);
        fs::write(
            vault.join("know-1.md"),
            md_with_score("know-1", 80.0, &recent.to_rfc3339(), "knowledge", "active"),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            80.0,
            &recent.to_rfc3339(),
            "knowledge",
        )
        .await;

        let now = Utc::now();
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        let result = scheduler.process_one(&db_guard, &entity, now).unwrap();
        assert!(matches!(result, DecayOutcome::Skipped));
    }

    #[tokio::test]
    async fn test_decay_direction_layer_halved() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        fs::write(
            vault.join("dir-1.md"),
            md_with_score("dir-1", 80.0, "2026-06-01T00:00:00Z", "direction", "active"),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "dir-1",
            "dir-1.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "direction",
        )
        .await;

        let now = DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("dir-1").unwrap().unwrap();
        let result = scheduler.process_one(&db_guard, &entity, now).unwrap();
        assert!(matches!(result, DecayOutcome::Decayed));

        let entity = db_guard.get_entity("dir-1").unwrap().unwrap();
        let new_interest = entity.interest.unwrap();
        // direction 衰减减半：0.98^(30*0.5) = 0.98^15 = 0.7386
        // 80 * 0.7386 = 59.1
        assert!(new_interest > 50.0, "direction 衰减减半，应 > 50");
        assert!(
            (new_interest - 59.1).abs() < 0.5,
            "约 59.1，实际 {new_interest}"
        );
    }

    #[tokio::test]
    async fn test_decay_floor_enforced() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // interest=100, 200 天前 -> 应到地板 50
        fs::write(
            vault.join("know-1.md"),
            md_with_score(
                "know-1",
                100.0,
                "2025-12-01T00:00:00Z",
                "knowledge",
                "active",
            ),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            100.0,
            "2025-12-01T00:00:00Z",
            "knowledge",
        )
        .await;

        let now = DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        scheduler.process_one(&db_guard, &entity, now).unwrap();

        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        let new_interest = entity.interest.unwrap();
        assert!(
            (new_interest - 50.0).abs() < 1e-6,
            "应到地板 50，实际 {new_interest}"
        );
    }

    #[tokio::test]
    async fn test_decay_writes_back_frontmatter() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        let path = vault.join("know-1.md");
        fs::write(
            &path,
            md_with_score(
                "know-1",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "active",
            ),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "knowledge",
        )
        .await;

        let now = DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        scheduler.process_one(&db_guard, &entity, now).unwrap();

        // 验证 frontmatter 已写回
        let content = fs::read_to_string(&path).unwrap();
        let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
        let score = frontmatter::get_score(&fm).unwrap().unwrap();
        assert!(score.interest < 80.0, "frontmatter interest 应已衰减");
    }

    #[tokio::test]
    async fn test_decay_records_history() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        fs::write(
            vault.join("know-1.md"),
            md_with_score(
                "know-1",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "active",
            ),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-1",
            "know-1.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "knowledge",
        )
        .await;

        let now = DateTime::parse_from_rfc3339("2026-07-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let db_guard = db.lock().await;
        let entity = db_guard.get_entity("know-1").unwrap().unwrap();
        scheduler.process_one(&db_guard, &entity, now).unwrap();

        // 验证 score_history 记录
        let last = db_guard.last_trigger_time("know-1", "Decay").unwrap();
        assert!(last.is_some(), "应记录 Decay 历史");
    }

    #[tokio::test]
    async fn test_run_once_multiple_entities() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // 三个实体：一个正常衰减、一个 archived、一个 boost 保护
        fs::write(
            vault.join("a.md"),
            md_with_score(
                "know-a",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "active",
            ),
        )
        .unwrap();
        fs::write(
            vault.join("b.md"),
            md_with_score(
                "know-b",
                80.0,
                "2026-06-01T00:00:00Z",
                "knowledge",
                "archived",
            ),
        )
        .unwrap();
        let recent = Utc::now() - Duration::days(1);
        fs::write(
            vault.join("c.md"),
            md_with_score("know-c", 80.0, &recent.to_rfc3339(), "knowledge", "active"),
        )
        .unwrap();

        let (scheduler, db) = setup(&vault);
        insert_entity(
            &db,
            "know-a",
            "a.md",
            80.0,
            "2026-06-01T00:00:00Z",
            "knowledge",
        )
        .await;
        {
            let b = EntityRow {
                id: "know-b".to_string(),
                file_path: "b.md".to_string(),
                title: Some("T".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("archived".to_string()),
                interest: Some(80.0),
                strategy: Some(50.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: Some("2026-06-01T00:00:00Z".to_string()),
                content_hash: Some("abc".to_string()),
                updated_at: Some("2026-07-01T00:00:00Z".to_string()),
            };
            db.lock().await.upsert_entity(&b, "body").unwrap();
        }
        insert_entity(
            &db,
            "know-c",
            "c.md",
            80.0,
            &recent.to_rfc3339(),
            "knowledge",
        )
        .await;

        let result = scheduler.run_once().await.unwrap();
        assert_eq!(result.total, 3);
        assert_eq!(result.decayed, 1, "只有 know-a 应衰减");
        assert_eq!(result.skipped_archived, 1, "know-b archived");
        assert_eq!(result.skipped_boost_protection, 1, "know-c boost 保护");
    }

    #[test]
    fn test_next_run_time() {
        // 01:00 -> 当天 02:00
        let now = DateTime::parse_from_rfc3339("2026-07-09T01:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = next_run_time(now);
        assert_eq!(next.format("%H").to_string(), "02");

        // 03:00 -> 次日 02:00
        let now = DateTime::parse_from_rfc3339("2026-07-09T03:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = next_run_time(now);
        assert_eq!(next.format("%H").to_string(), "02");
        assert_eq!(next.format("%d").to_string(), "10");
    }
}
