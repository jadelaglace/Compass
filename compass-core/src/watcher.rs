//! FileWatcher（T1.5）：notify 监听 vault → 解析 frontmatter → 索引 + 重算评分 → 写回。
//!
//! 设计：
//! - 使用 `notify` crate 监听 vault 目录（Create/Modify/Delete 事件）
//! - 事件去抖（debounce）避免频繁触发
//! - 读取 frontmatter → 解析 id/title/layer/score
//! - upsert entities + FTS5 索引
//! - 无 score 时计算默认评分并写回
//! - 删除事件清理 entities + FTS
//! - 跳过隐藏目录（.obsidian, .compass, .git）

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

use crate::db::{Db, EntityRow, ScoreHistoryRow};
use crate::frontmatter;
use crate::models::{Score, Weights};
use crate::scoring;

/// 默认去抖时间（毫秒）
const DEBOUNCE_MS: u64 = 500;

/// 默认 interest/strategy/consensus 初始值（无 score 时）
const DEFAULT_INTEREST: f64 = 5.0;
const DEFAULT_STRATEGY: f64 = 5.0;
const DEFAULT_CONSENSUS: f64 = 5.0;

/// 隐藏目录列表（跳过这些目录下的事件）
const HIDDEN_DIRS: &[&str] = &[".obsidian", ".compass", ".git"];

/// FileWatcher 配置
pub struct FileWatcher {
    vault: PathBuf,
    db: Arc<Mutex<Db>>,
    weights: Weights,
    watcher: Option<RecommendedWatcher>,
}

impl FileWatcher {
    /// 创建新的 FileWatcher
    pub fn new(vault: PathBuf, db: Arc<Mutex<Db>>, weights: Weights) -> Self {
        Self {
            vault,
            db,
            weights,
            watcher: None,
        }
    }

    /// 启动监听（异步）
    pub async fn start(&mut self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        // 创建 watcher
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(100)),
        )?;

        // 监听 vault 目录
        watcher.watch(&self.vault, RecursiveMode::Recursive)?;
        info!(vault = %self.vault.display(), "FileWatcher started");

        self.watcher = Some(watcher);

        // 处理事件（去抖）
        let vault = self.vault.clone();
        let db = self.db.clone();
        let weights = self.weights;

        tokio::spawn(async move {
            let mut last_events: HashSet<PathBuf> = HashSet::new();
            let mut debounce_timer: Option<tokio::time::Instant> = None;

            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        // 过滤隐藏目录
                        if is_hidden_path(&event.paths) {
                            continue;
                        }

                        // 去抖：收集事件，延迟处理
                        for path in &event.paths {
                            if path.extension().and_then(|e| e.to_str()) == Some("md")
                                || (matches!(&event.kind, EventKind::Remove(_))
                                    && !path.exists())
                            {
                                last_events.insert(path.clone());
                            }
                        }

                        // 重置去抖定时器
                        debounce_timer = Some(tokio::time::Instant::now());
                    }
                    _ = async {
                        if let Some(timer) = debounce_timer {
                            let elapsed = timer.elapsed();
                            if elapsed < Duration::from_millis(DEBOUNCE_MS) {
                                tokio::time::sleep(Duration::from_millis(DEBOUNCE_MS) - elapsed).await;
                            }
                        } else {
                            // 永远等待（无事件）
                            std::future::pending::<()>().await;
                        }
                    }, if debounce_timer.is_some() => {
                        // 去抖完成，处理收集的事件
                        if !last_events.is_empty() {
                            process_events(&vault, &db, &weights, &last_events).await;
                            last_events.clear();
                            debounce_timer = None;
                        }
                    }
                }
            }
        });

        Ok(())
    }
}

/// 处理收集的事件
async fn process_events(
    vault: &Path,
    db: &Arc<Mutex<Db>>,
    weights: &Weights,
    events: &HashSet<PathBuf>,
) {
    for path in events {
        match process_single_file(vault, db, weights, path).await {
            Ok(_) => debug!(path = %path.display(), "processed"),
            Err(e) => warn!(path = %path.display(), err = %e, "process failed"),
        }
    }
}

/// 处理单个文件
pub(crate) async fn process_single_file(
    vault: &Path,
    db: &Arc<Mutex<Db>>,
    weights: &Weights,
    path: &Path,
) -> Result<()> {
    // 文件不存在则删除索引
    if !path.exists() {
        let relative = rel_path(vault, path);
        let db = db.lock().await;
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(id) = db.entity_id_by_file_path(&relative)? {
                info!(id = %id, path = %path.display(), "deleting entity");
                db.delete_entity(&id)?;
            }
        } else {
            db.delete_entities_under_path(&relative)?;
        }
        return Ok(());
    }

    // 读取 frontmatter
    let note = frontmatter::read_note(path)?;

    // 解析 id（无 id 跳过）
    let id = match extract_id_from_frontmatter(&note.frontmatter) {
        Some(id) => id,
        None => {
            debug!(path = %path.display(), "no id, skipping");
            let relative = rel_path(vault, path);
            let db = db.lock().await;
            if let Some(existing_id) = db.entity_id_by_file_path(&relative)? {
                db.delete_entity(&existing_id)?;
            }
            return Ok(());
        }
    };

    // 解析其他字段
    let title = extract_title(&note.frontmatter).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    let layer = extract_layer(&note.frontmatter);
    let status = extract_status(&note.frontmatter).unwrap_or_else(|| "active".to_string());

    // 解析或计算 score
    let score = match frontmatter::get_score(&note.frontmatter)? {
        Some(score) => {
            // 已有 score，重算 composite（确保一致性）
            let composite =
                scoring::composite(score.interest, score.strategy, score.consensus, weights);
            Score {
                interest: score.interest,
                strategy: score.strategy,
                consensus: score.consensus,
                composite,
                weights: score.weights,
                updated_at: score.updated_at,
                last_boosted_at: score.last_boosted_at,
                access_count: score.access_count,
            }
        }
        None => {
            // 无 score，计算默认评分并写回
            info!(id = %id, path = %path.display(), "no score, calculating default");
            let now = chrono::Utc::now().to_rfc3339();
            let composite = scoring::composite(
                DEFAULT_INTEREST,
                DEFAULT_STRATEGY,
                DEFAULT_CONSENSUS,
                weights,
            );
            let score = Score {
                interest: DEFAULT_INTEREST,
                strategy: DEFAULT_STRATEGY,
                consensus: DEFAULT_CONSENSUS,
                composite,
                weights: Some(*weights),
                updated_at: now.clone(),
                last_boosted_at: now,
                access_count: 0,
            };

            // 写回 frontmatter
            frontmatter::write_score(path, &score)?;
            score
        }
    };

    // 内容 hash
    let content_hash = content_hash(&note.body);

    // 相对路径
    let file_path = rel_path(vault, path);

    // upsert entities + FTS
    let entity = EntityRow {
        id: id.clone(),
        file_path,
        title: Some(title),
        layer,
        status: Some(status),
        interest: Some(score.interest),
        strategy: Some(score.strategy),
        consensus: Some(score.consensus),
        composite: Some(score.composite),
        access_count: score.access_count,
        last_boosted_at: Some(score.last_boosted_at.clone()),
        content_hash: Some(content_hash),
        updated_at: Some(score.updated_at.clone()),
    };

    let tags = frontmatter::extract_tags(&note.frontmatter);
    let links = frontmatter::extract_refs(&note.body);
    db.lock()
        .await
        .upsert_entity_with_relationships(&entity, &note.body, &tags, &links)?;

    debug!(id = %id, composite = %score.composite, "entity upserted");

    // ---- ????????T1.3??Linked???+ Cited????????----
    // ????????rebuild_from_vault ???????????backfill ?????
    // ??? score_history per-type ????????? frontmatter?body/content_hash ????
    fire_link_and_cited_triggers(vault, db, &id, path, &score, &note, &entity).await?;

    Ok(())
}

/// ?? Linked????? outgoing refs?? Cited???????????????
/// ????????????????? frontmatter + ???? + ? score_history?
async fn fire_link_and_cited_triggers(
    vault: &Path,
    db: &Arc<Mutex<Db>>,
    src_id: &str,
    src_path: &Path,
    src_score: &Score,
    note: &frontmatter::Note,
    src_entity: &EntityRow,
) -> Result<()> {
    let refs = frontmatter::extract_refs(&note.body);
    if refs.is_empty() {
        return Ok(());
    }
    let now = chrono::Utc::now().to_rfc3339();
    let db = db.lock().await;

    // Linked?????????? -> interest +1?7 ????
    let last_linked = db.last_trigger_time(src_id, "Linked")?;
    if let Some(new_score) = scoring::apply_trigger_if_eligible(
        src_score,
        scoring::Trigger::Linked,
        &now,
        last_linked.as_deref(),
    )? {
        frontmatter::write_score(src_path, &new_score)?;
        let row = apply_score_to_row(src_entity.clone(), &new_score);
        db.upsert_entity(&row, &note.body)?;
        db.insert_score_history(&score_history_row(
            src_id,
            "Linked",
            "interest",
            src_score.interest,
            new_score.interest,
            &now,
        ))?;
        info!(id = %src_id, "trigger Linked: interest {} -> {}", src_score.interest, new_score.interest);
    }

    // Cited??????????? -> consensus +2?1 ????
    for target_id in &refs {
        let Some(trow) = db.get_entity(target_id)? else {
            continue;
        };
        let tpath = vault.join(&trow.file_path);
        let Ok(tnote) = frontmatter::read_note(&tpath) else {
            continue;
        };
        let Ok(Some(tscore)) = frontmatter::get_score(&tnote.frontmatter) else {
            continue;
        };
        let last_cited = db.last_trigger_time(target_id, "Cited")?;
        let Some(new_tscore) = scoring::apply_trigger_if_eligible(
            &tscore,
            scoring::Trigger::Cited,
            &now,
            last_cited.as_deref(),
        )?
        else {
            continue;
        };
        frontmatter::write_score(&tpath, &new_tscore)?;
        let trow = apply_score_to_row(trow.clone(), &new_tscore);
        db.upsert_entity(&trow, &tnote.body)?;
        db.insert_score_history(&score_history_row(
            target_id,
            "Cited",
            "consensus",
            tscore.consensus,
            new_tscore.consensus,
            &now,
        ))?;
        info!(id = %target_id, cited_by = %src_id, "trigger Cited: consensus {} -> {}", tscore.consensus, new_tscore.consensus);
    }

    Ok(())
}

/// ?? Score ?? EntityRow ??????id/file_path/title/layer/content_hash ????
fn apply_score_to_row(mut row: EntityRow, score: &Score) -> EntityRow {
    row.interest = Some(score.interest);
    row.strategy = Some(score.strategy);
    row.consensus = Some(score.consensus);
    row.composite = Some(score.composite);
    row.access_count = score.access_count;
    row.last_boosted_at = Some(score.last_boosted_at.clone());
    row.updated_at = Some(score.updated_at.clone());
    row
}

/// ???? score_history ???
fn score_history_row(
    entity_id: &str,
    trigger: &str,
    dimension: &str,
    old: f64,
    new: f64,
    now: &str,
) -> ScoreHistoryRow {
    ScoreHistoryRow {
        entity_id: entity_id.to_string(),
        dimension: Some(dimension.to_string()),
        old: Some(old),
        new: Some(new),
        reason: Some(format!("trigger:{trigger}")),
        trigger: Some(trigger.to_string()),
        created_at: now.to_string(),
    }
}

/// 从 frontmatter 提取 id
fn extract_id_from_frontmatter(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(serde_yaml::Value::String("id".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 title
fn extract_title(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(serde_yaml::Value::String("title".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 layer
fn extract_layer(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(serde_yaml::Value::String("layer".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 status
fn extract_status(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(serde_yaml::Value::String("status".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 内容指纹（DefaultHasher，固定种子，跨运行稳定）
fn content_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// 相对 vault 根的路径，统一正斜杠
fn rel_path(vault: &Path, p: &Path) -> String {
    match p.strip_prefix(vault) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => p.to_string_lossy().replace('\\', "/"),
    }
}

/// 检查路径是否在隐藏目录下
fn is_hidden_path(paths: &[PathBuf]) -> bool {
    for path in paths {
        for ancestor in path.ancestors() {
            if let Some(dir_name) = ancestor.file_name() {
                if let Some(dir_str) = dir_name.to_str() {
                    if HIDDEN_DIRS.contains(&dir_str) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_extract_id_from_frontmatter() {
        let fm = "id: know-000001\ntitle: Test\n";
        assert_eq!(
            extract_id_from_frontmatter(fm),
            Some("know-000001".to_string())
        );

        let fm_no_id = "title: No ID\n";
        assert_eq!(extract_id_from_frontmatter(fm_no_id), None);
    }

    #[test]
    fn test_extract_title() {
        let fm = "id: know-000001\ntitle: Game Theory\n";
        assert_eq!(extract_title(fm), Some("Game Theory".to_string()));
    }

    #[test]
    fn test_extract_layer() {
        let fm = "id: know-000001\nlayer: knowledge\n";
        assert_eq!(extract_layer(fm), Some("knowledge".to_string()));
    }

    #[test]
    fn test_is_hidden_path() {
        let hidden = vec![PathBuf::from("/vault/.obsidian/config.md")];
        assert!(is_hidden_path(&hidden));

        let visible = vec![PathBuf::from("/vault/knowledge/test.md")];
        assert!(!is_hidden_path(&visible));
    }

    #[test]
    fn test_content_hash_stable() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
    }

    #[test]
    fn test_rel_path() {
        let vault = PathBuf::from("/vault");
        let path = PathBuf::from("/vault/knowledge/test.md");
        assert_eq!(rel_path(&vault, &path), "knowledge/test.md");
    }

    #[tokio::test]
    async fn test_process_single_file_with_score() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // 创建带 score 的文件
        let md = "---\nid: know-000001\ntitle: Test\nlayer: knowledge\nscore:\n  interest: 85.0\n  strategy: 90.0\n  consensus: 80.0\n  composite: 85.5\n  updated_at: '2026-07-06T00:00:00Z'\n  last_boosted_at: '2026-07-06T00:00:00Z'\n  access_count: 5\n---\nBody content.\n";
        fs::write(vault.join("know-000001.md"), md).unwrap();

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        let path = vault.join("know-000001.md");
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();

        let entity = db.lock().await.get_entity("know-000001").unwrap().unwrap();
        assert_eq!(entity.title.as_deref(), Some("Test"));
        assert!((entity.composite.unwrap() - 85.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_process_single_file_rebuilds_tags_and_links() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let path = vault.join("know-000001.md");
        let first = "---\nid: know-000001\ntitle: Test\ntags:\n  - Rust\n  - '#rust'\nscore:\n  interest: 85.0\n  strategy: 90.0\n  consensus: 80.0\n  composite: 85.5\n  updated_at: '2026-07-06T00:00:00Z'\n  last_boosted_at: '2026-07-06T00:00:00Z'\n  access_count: 5\n---\nLinks [[know-000002]].\n";
        fs::write(&path, first).unwrap();

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();
        assert_eq!(
            db.lock().await.entity_tags("know-000001").unwrap(),
            vec!["Rust"]
        );
        assert_eq!(
            db.lock().await.entity_links("know-000001").unwrap(),
            vec!["know-000002"]
        );

        let second = first
            .replace("  - Rust\n  - '#rust'", "  - SQLite")
            .replace("[[know-000002]]", "[[know-000003]]");
        fs::write(&path, second).unwrap();
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();
        assert_eq!(
            db.lock().await.entity_tags("know-000001").unwrap(),
            vec!["SQLite"]
        );
        assert_eq!(
            db.lock().await.entity_links("know-000001").unwrap(),
            vec!["know-000003"]
        );
    }

    #[tokio::test]
    async fn test_process_single_file_without_score() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        // 创建无 score 的文件
        let md = "---\nid: know-000002\ntitle: New Note\nlayer: knowledge\n---\nBody.\n";
        fs::write(vault.join("know-000002.md"), md).unwrap();

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        let path = vault.join("know-000002.md");
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();

        let entity = db.lock().await.get_entity("know-000002").unwrap().unwrap();
        assert_eq!(entity.title.as_deref(), Some("New Note"));
        // 默认评分：interest=5, strategy=5, consensus=5 → composite=5.0
        assert!((entity.composite.unwrap() - 5.0).abs() < 1e-9);

        // 验证 score 已写回 frontmatter
        let content = fs::read_to_string(&path).unwrap();
        let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
        // 验证 score 已写回 frontmatter
        assert!(fm.contains("composite:"), "composite not found in: {fm}");
    }

    #[tokio::test]
    async fn test_trigger_linked_and_cited_fire_once() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        let md_b = "---\nid: know-b\ntitle: B\nlayer: knowledge\nscore:\n  interest: 50.0\n  strategy: 50.0\n  consensus: 50.0\n  composite: 50.0\n  updated_at: '2026-07-01T00:00:00Z'\n  last_boosted_at: '2026-07-01T00:00:00Z'\n  access_count: 0\n---\nB body.\n";
        fs::write(vault.join("know-b.md"), md_b).unwrap();
        process_single_file(&vault, &db, &weights, &vault.join("know-b.md"))
            .await
            .unwrap();

        let md_a = "---\nid: know-a\ntitle: A\nlayer: knowledge\nscore:\n  interest: 50.0\n  strategy: 50.0\n  consensus: 50.0\n  composite: 50.0\n  updated_at: '2026-07-01T00:00:00Z'\n  last_boosted_at: '2026-07-01T00:00:00Z'\n  access_count: 0\n---\nA cites [[know-b]].\n";
        let path_a = vault.join("know-a.md");
        fs::write(&path_a, md_a).unwrap();
        process_single_file(&vault, &db, &weights, &path_a)
            .await
            .unwrap();

        let a = db.lock().await.get_entity("know-a").unwrap().unwrap();
        assert!(
            (a.interest.unwrap() - 51.0).abs() < 1e-9,
            "Linked ?? A interest +1"
        );
        let b = db.lock().await.get_entity("know-b").unwrap().unwrap();
        assert!(
            (b.consensus.unwrap() - 52.0).abs() < 1e-9,
            "Cited ?? B consensus +2"
        );

        assert!(
            db.lock()
                .await
                .last_trigger_time("know-a", "Linked")
                .unwrap()
                .is_some(),
            "?? Linked ??"
        );
        assert!(
            db.lock()
                .await
                .last_trigger_time("know-b", "Cited")
                .unwrap()
                .is_some(),
            "?? Cited ??"
        );

        let content_b = fs::read_to_string(vault.join("know-b.md")).unwrap();
        assert!(
            content_b.contains("consensus: 52"),
            "B frontmatter ??? consensus 52???: {content_b}"
        );

        process_single_file(&vault, &db, &weights, &path_a)
            .await
            .unwrap();
        let a2 = db.lock().await.get_entity("know-a").unwrap().unwrap();
        assert!(
            (a2.interest.unwrap() - 51.0).abs() < 1e-9,
            "??? Linked ?????"
        );
        let b2 = db.lock().await.get_entity("know-b").unwrap().unwrap();
        assert!(
            (b2.consensus.unwrap() - 52.0).abs() < 1e-9,
            "??? Cited ?????"
        );
    }

    #[tokio::test]
    async fn test_trigger_cited_skips_nonexistent_target() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        let md_a = "---\nid: know-a\ntitle: A\nlayer: knowledge\nscore:\n  interest: 50.0\n  strategy: 50.0\n  consensus: 50.0\n  composite: 50.0\n  updated_at: '2026-07-01T00:00:00Z'\n  last_boosted_at: '2026-07-01T00:00:00Z'\n  access_count: 0\n---\nA cites [[know-ghost]].\n";
        let path_a = vault.join("know-a.md");
        fs::write(&path_a, md_a).unwrap();
        process_single_file(&vault, &db, &weights, &path_a)
            .await
            .unwrap();
        let a = db.lock().await.get_entity("know-a").unwrap().unwrap();
        assert!((a.interest.unwrap() - 51.0).abs() < 1e-9, "Linked ??? A +1");
    }

    #[tokio::test]
    async fn test_process_single_file_delete() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        // 先 upsert 一个实体
        let entity = EntityRow {
            id: "know-000003".to_string(),
            file_path: "custom-name.md".to_string(),
            title: Some("Test".to_string()),
            layer: Some("knowledge".to_string()),
            status: Some("active".to_string()),
            interest: Some(5.0),
            strategy: Some(5.0),
            consensus: Some(5.0),
            composite: Some(5.0),
            access_count: 0,
            last_boosted_at: Some("2026-07-06T00:00:00Z".to_string()),
            content_hash: Some("abc".to_string()),
            updated_at: Some("2026-07-06T00:00:00Z".to_string()),
        };
        db.lock().await.upsert_entity(&entity, "body").unwrap();

        // 删除文件（不存在）
        let path = vault.join("custom-name.md");
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();
        assert!(db.lock().await.get_entity("know-000003").unwrap().is_none());

        // 验证实体已删除
        assert!(db.lock().await.get_entity("know-000003").unwrap().is_none());
    }
}
