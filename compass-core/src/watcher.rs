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
use tracing::{debug, error, info, warn};

use crate::db::{Db, EntityRow};
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
        let weights = self.weights.clone();

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
                            if path.extension().and_then(|e| e.to_str()) == Some("md") {
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
        if let Some(id) = extract_id_from_path(vault, path) {
            info!(id = %id, path = %path.display(), "deleting entity");
            db.lock().await.delete_entity(&id)?;
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
                scoring::composite(score.interest, score.strategy, score.consensus, &weights);
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
                weights: Some(weights.clone()),
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
        last_boosted_at: Some(score.last_boosted_at),
        content_hash: Some(content_hash),
        updated_at: Some(score.updated_at),
    };

    db.lock().await.upsert_entity(&entity, &note.body)?;

    debug!(id = %id, composite = %score.composite, "entity upserted");

    Ok(())
}

/// 从路径提取 id（vault 相对路径的文件名）
fn extract_id_from_path(vault: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(vault).ok()?;
    let file_name = rel.file_stem()?.to_str()?;
    // 文件名格式：id.md（如 know-000001.md）
    let id = file_name.split('.').next()?;
    if id.is_empty() {
        None
    } else {
        Some(id.to_string())
    }
}

/// 从 frontmatter 提取 id
fn extract_id_from_frontmatter(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(&serde_yaml::Value::String("id".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 title
fn extract_title(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(&serde_yaml::Value::String("title".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 layer
fn extract_layer(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(&serde_yaml::Value::String("layer".into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// 从 frontmatter 提取 status
fn extract_status(frontmatter: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(&serde_yaml::Value::String("status".into()))
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
    fn test_extract_id_from_path() {
        let vault = PathBuf::from("/vault");
        let path = PathBuf::from("/vault/knowledge/know-000001.md");
        assert_eq!(
            extract_id_from_path(&vault, &path),
            Some("know-000001".to_string())
        );

        // 无 id 的文件名
        let path_no_id = PathBuf::from("/vault/inbox/temp.md");
        assert_eq!(
            extract_id_from_path(&vault, &path_no_id),
            Some("temp".to_string())
        );
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
    async fn test_process_single_file_delete() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();

        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = Weights::default();

        // 先 upsert 一个实体
        let entity = EntityRow {
            id: "know-000003".to_string(),
            file_path: "know-000003.md".to_string(),
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
        let path = vault.join("know-000003.md");
        process_single_file(&vault, &db, &weights, &path)
            .await
            .unwrap();

        // 验证实体已删除
        assert!(db.lock().await.get_entity("know-000003").unwrap().is_none());
    }
}
