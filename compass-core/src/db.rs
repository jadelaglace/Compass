//! SQLite 索引层（T1.4）：entities / score_history / timeline / entities_fts。
//!
//! 设计（PRD_v3.0 §4.2）：
//! - frontmatter 是权威，SQLite 仅作索引/缓存/历史，删库可从 vault 重建。
//! - entities 表缓存三维评分（interest/strategy/consensus + composite），列表查询免读文件。
//! - score_history 记评分变更历史（frontmatter 不存），并供 T1.3 冷却 per-type 查询
//!   （`last_trigger_time` 按 `id DESC` 取最新——插入序即时间序，规避 RFC3339 时区排序问题）。
//! - entities_fts 用普通 FTS5 内部表（title, content），rowid 绑定 entities 隐式 rowid，
//!   支持 snippet；仅 Agent/Skill 用（Obsidian 用自身搜索）。
//! - rebuild 不清空 score_history/timeline（历史日志保留；孤儿记录可接受，T1.4 范围外）。

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::frontmatter;

/// entities 表的行镜像（vault 文件的索引缓存）。
#[derive(Debug, Clone, PartialEq)]
pub struct EntityRow {
    pub id: String,
    /// 相对 vault 根的路径，正斜杠分隔。
    pub file_path: String,
    pub title: Option<String>,
    pub layer: Option<String>,
    pub status: Option<String>,
    pub interest: Option<f64>,
    pub strategy: Option<f64>,
    pub consensus: Option<f64>,
    pub composite: Option<f64>,
    pub access_count: i64,
    pub last_boosted_at: Option<String>,
    pub content_hash: Option<String>,
    /// 评分写回时间（取自 frontmatter `score.updated_at`）。
    pub updated_at: Option<String>,
}

/// score_history 行（评分变更历史，frontmatter 不存）。
#[derive(Debug, Clone)]
pub struct ScoreHistoryRow {
    pub entity_id: String,
    pub dimension: Option<String>,
    pub old: Option<f64>,
    pub new: Option<f64>,
    pub reason: Option<String>,
    /// 触发器类型名（Cited/Linked/CaseAdded/ManualMark/ReviewCompleted/Decay/Access...）。
    pub trigger: Option<String>,
    pub created_at: String,
}

/// timeline 行（访问/引用/评分事件流）。
#[derive(Debug, Clone)]
pub struct TimelineRow {
    pub entity_id: String,
    pub event_type: String,
    pub intensity: Option<f64>,
    pub source: Option<String>,
    pub created_at: String,
}

/// FTS 搜索命中。
#[derive(Debug, Clone, PartialEq)]
pub struct FtsHit {
    pub id: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

/// 全量重建统计。
#[derive(Debug, Default, Clone)]
pub struct RebuildStats {
    pub indexed: u32,
    pub skipped: u32,
    pub duplicates: u32,
}

pub struct Db {
    conn: Connection,
}

const CURRENT_SCHEMA_VERSION: i64 = 2;

impl Db {
    /// 打开（或创建）数据库文件并初始化 schema。父目录自动创建。
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("创建 db 父目录失败 {}", parent.display()))?;
            }
        }
        backup_before_migration(path)?;
        let conn =
            Connection::open(path).with_context(|| format!("打开数据库失败 {}", path.display()))?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// 内存数据库（测试用）。
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
               id INTEGER PRIMARY KEY CHECK (id = 1),
               version INTEGER NOT NULL
             );",
        )?;
        let current = tx
            .query_row(
                "SELECT version FROM schema_version WHERE id = 1",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        if current > CURRENT_SCHEMA_VERSION {
            return Err(anyhow::anyhow!(
                "database schema version {current} is newer than supported version {CURRENT_SCHEMA_VERSION}"
            ));
        }
        for version in (current + 1)..=CURRENT_SCHEMA_VERSION {
            apply_migration(&tx, version)?;
        }
        tx.execute(
            "INSERT INTO schema_version (id, version) VALUES (1, ?1)
             ON CONFLICT(id) DO UPDATE SET version = excluded.version",
            params![CURRENT_SCHEMA_VERSION],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get(0),
        )?)
    }

    /// upsert entity 并同步 FTS（`fts_content` = Markdown 正文，供 FTS 索引与 snippet）。
    pub fn upsert_entity(&self, e: &EntityRow, fts_content: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO entities
               (id, file_path, title, layer, status, interest, strategy, consensus,
                composite, access_count, last_boosted_at, content_hash, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(id) DO UPDATE SET
               file_path=excluded.file_path, title=excluded.title, layer=excluded.layer,
               status=excluded.status, interest=excluded.interest, strategy=excluded.strategy,
               consensus=excluded.consensus, composite=excluded.composite,
               access_count=excluded.access_count, last_boosted_at=excluded.last_boosted_at,
               content_hash=excluded.content_hash, updated_at=excluded.updated_at",
            params![
                e.id,
                e.file_path,
                e.title,
                e.layer,
                e.status,
                e.interest,
                e.strategy,
                e.consensus,
                e.composite,
                e.access_count,
                e.last_boosted_at,
                e.content_hash,
                e.updated_at,
            ],
        )?;
        let rowid: i64 = tx.query_row(
            "SELECT rowid FROM entities WHERE id = ?1",
            params![e.id],
            |r| r.get(0),
        )?;
        // FTS5 内部表：先按 rowid 删旧（不存在也安全），再插新。
        tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rowid])?;
        tx.execute(
            "INSERT INTO entities_fts (rowid, title, content) VALUES (?1, ?2, ?3)",
            params![rowid, e.title.as_deref().unwrap_or(""), fts_content],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_entity(&self, id: &str) -> Result<Option<EntityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, title, layer, status, interest, strategy, consensus,
                    composite, access_count, last_boosted_at, content_hash, updated_at
             FROM entities WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        match rows.next()? {
            Some(r) => Ok(Some(row_to_entity(r)?)),
            None => Ok(None),
        }
    }

    /// 按 composite 降序返回（NULL 自然排最后）。
    pub fn list_entities(&self) -> Result<Vec<EntityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, title, layer, status, interest, strategy, consensus,
                    composite, access_count, last_boosted_at, content_hash, updated_at
             FROM entities ORDER BY composite DESC, id",
        )?;
        let rows = stmt.query_map([], row_to_entity)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// 删除实体并清理其 FTS 记录。
    pub fn delete_entity(&self, id: &str) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let rowid: Option<i64> = tx
            .query_row(
                "SELECT rowid FROM entities WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .ok();
        if let Some(rid) = rowid {
            tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rid])?;
        }
        tx.execute("DELETE FROM entity_tags WHERE entity_id = ?1", params![id])?;
        tx.execute(
            "DELETE FROM entity_links WHERE source_id = ?1 OR target_id = ?1",
            params![id],
        )?;
        tx.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    pub fn replace_entity_relationships(
        &self,
        entity_id: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM entity_tags WHERE entity_id = ?1",
            params![entity_id],
        )?;
        tx.execute(
            "DELETE FROM entity_links WHERE source_id = ?1",
            params![entity_id],
        )?;
        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO entity_tags (entity_id, tag, tag_key)
                 VALUES (?1, ?2, ?3)",
                params![entity_id, tag, tag.to_lowercase()],
            )?;
        }
        for target_id in links {
            tx.execute(
                "INSERT OR IGNORE INTO entity_links (source_id, target_id)
                 VALUES (?1, ?2)",
                params![entity_id, target_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn entity_tags(&self, entity_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM entity_tags WHERE entity_id = ?1 ORDER BY tag_key, tag")?;
        let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    #[allow(dead_code)]
    pub fn entity_links(&self, entity_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT target_id FROM entity_links WHERE source_id = ?1 ORDER BY target_id",
        )?;
        let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// FTS5 搜索。query 按空白拆词、各自加引号后 AND 连接（避免 `-`/`*` 等被当作 FTS 语法）。
    pub fn fts_search(&self, query: &str, limit: u32) -> Result<Vec<FtsHit>> {
        let q = fts_query(query);
        if q.is_empty() {
            return Ok(vec![]);
        }
        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.title, snippet(entities_fts, 1, '<b>', '</b>', '...', 16) AS snip
             FROM entities_fts f
             JOIN entities e ON e.rowid = f.rowid
             WHERE entities_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![q, limit as i64], |r| {
            Ok(FtsHit {
                id: r.get(0)?,
                title: r.get(1)?,
                snippet: r.get(2)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// 该实体某触发器类型最近一次触发时间（供 T1.3 冷却判断）。
    /// 按 score_history 自增 `id DESC` 取最新——插入序即时间序，规避 RFC3339 时区排序歧义。
    pub fn last_trigger_time(&self, entity_id: &str, trigger: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT created_at FROM score_history
             WHERE entity_id = ?1 AND trigger = ?2
             ORDER BY id DESC LIMIT 1",
        )?;
        let mut rows = stmt.query(params![entity_id, trigger])?;
        match rows.next()? {
            Some(r) => Ok(Some(r.get(0)?)),
            None => Ok(None),
        }
    }

    pub fn insert_score_history(&self, h: &ScoreHistoryRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO score_history
               (entity_id, dimension, old, new, reason, trigger, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                h.entity_id,
                h.dimension,
                h.old,
                h.new,
                h.reason,
                h.trigger,
                h.created_at
            ],
        )?;
        Ok(())
    }

    pub fn insert_timeline(&self, t: &TimelineRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO timeline (entity_id, event_type, intensity, source, created_at)
             VALUES (?1,?2,?3,?4,?5)",
            params![
                t.entity_id,
                t.event_type,
                t.intensity,
                t.source,
                t.created_at
            ],
        )?;
        Ok(())
    }

    /// 从 vault 全量重建索引（清空 entities + FTS，重新扫描写入）。
    /// score_history/timeline 不清空（历史保留）。无 `id` frontmatter 的笔记跳过。
    pub fn rebuild_from_vault(&self, vault: &Path) -> Result<RebuildStats> {
        {
            let tx = self.conn.unchecked_transaction()?;
            tx.execute("DELETE FROM entities", [])?;
            tx.execute("DELETE FROM entities_fts", [])?;
            tx.execute("DELETE FROM entity_tags", [])?;
            tx.execute("DELETE FROM entity_links", [])?;
            tx.commit()?;
        }
        let mut stats = RebuildStats::default();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for path in walk_md(vault)? {
            match parse_entity(vault, &path) {
                Ok(Some(parsed)) => {
                    if !seen.insert(parsed.row.id.clone()) {
                        tracing::warn!(id = %parsed.row.id, path = %path.display(), "重复 id，跳过后续文件");
                        stats.duplicates += 1;
                        continue;
                    }
                    if let Err(e) = self.upsert_entity(&parsed.row, &parsed.body) {
                        tracing::warn!(path = %path.display(), err = %e, "索引写入失败，跳过");
                        stats.skipped += 1;
                    } else {
                        self.replace_entity_relationships(
                            &parsed.row.id,
                            &parsed.tags,
                            &parsed.links,
                        )?;
                        stats.indexed += 1;
                    }
                }
                Ok(None) => stats.skipped += 1,
                Err(e) => {
                    tracing::warn!(path = %path.display(), err = %e, "解析失败，跳过");
                    stats.skipped += 1;
                }
            }
        }
        Ok(stats)
    }
}

fn apply_migration(tx: &Transaction<'_>, version: i64) -> Result<()> {
    match version {
        #[cfg(test)]
        99 => {
            tx.execute_batch("CREATE TABLE migration_probe (value TEXT);")?;
            return Err(anyhow::anyhow!("injected migration failure"));
        }
        1 => tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS entities (
               id TEXT PRIMARY KEY,
               file_path TEXT UNIQUE NOT NULL,
               title TEXT,
               layer TEXT,
               status TEXT,
               interest REAL,
               strategy REAL,
               consensus REAL,
               composite REAL,
               access_count INTEGER NOT NULL DEFAULT 0,
               last_boosted_at TEXT,
               content_hash TEXT,
               updated_at TEXT
             );
             CREATE TABLE IF NOT EXISTS score_history (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               entity_id TEXT NOT NULL,
               dimension TEXT,
               old REAL,
               new REAL,
               reason TEXT,
               trigger TEXT,
               created_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS timeline (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               entity_id TEXT NOT NULL,
               event_type TEXT NOT NULL,
               intensity REAL,
               source TEXT,
               created_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_score_history_entity ON score_history(entity_id);
             CREATE INDEX IF NOT EXISTS idx_score_history_trigger ON score_history(entity_id, trigger);
             CREATE INDEX IF NOT EXISTS idx_timeline_entity ON timeline(entity_id);
             CREATE VIRTUAL TABLE IF NOT EXISTS entities_fts USING fts5(title, content);
             CREATE TABLE IF NOT EXISTS suggestions (
               suggestion_id TEXT PRIMARY KEY,
               kind TEXT NOT NULL CHECK (kind IN ('tag', 'related')),
               entity_id TEXT NOT NULL,
               candidate TEXT NOT NULL,
               candidate_key TEXT NOT NULL,
               confidence REAL,
               reason TEXT NOT NULL,
               source TEXT NOT NULL,
               algorithm_version TEXT NOT NULL,
               content_hash TEXT NOT NULL,
               status TEXT NOT NULL CHECK (status IN ('pending', 'accepted', 'rejected', 'expired')),
               created_at TEXT NOT NULL,
               updated_at TEXT NOT NULL,
               UNIQUE(kind, entity_id, candidate_key, content_hash, algorithm_version, source)
             );
             CREATE INDEX IF NOT EXISTS idx_suggestions_entity_status
               ON suggestions(entity_id, status);",
        )?,
        2 => tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS entity_tags (
               entity_id TEXT NOT NULL,
               tag TEXT NOT NULL,
               tag_key TEXT NOT NULL,
               PRIMARY KEY (entity_id, tag_key),
               FOREIGN KEY (entity_id) REFERENCES entities(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_entity_tags_tag_key ON entity_tags(tag_key);
             CREATE TABLE IF NOT EXISTS entity_links (
               source_id TEXT NOT NULL,
               target_id TEXT NOT NULL,
               PRIMARY KEY (source_id, target_id),
               FOREIGN KEY (source_id) REFERENCES entities(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_entity_links_target ON entity_links(target_id);",
        )?,
        unsupported => {
            return Err(anyhow::anyhow!("unsupported schema migration {unsupported}"));
        }
    }
    Ok(())
}

fn backup_before_migration(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let conn = Connection::open(path)?;
    let current = conn
        .query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .unwrap_or(None);
    if current.unwrap_or(0) >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
    let backup_path = path.with_extension("db.pre-migration.bak");
    fs::copy(path, &backup_path).with_context(|| {
        format!(
            "create database migration backup failed {}",
            backup_path.display()
        )
    })?;
    Ok(())
}

fn row_to_entity(r: &Row) -> rusqlite::Result<EntityRow> {
    Ok(EntityRow {
        id: r.get(0)?,
        file_path: r.get(1)?,
        title: r.get(2)?,
        layer: r.get(3)?,
        status: r.get(4)?,
        interest: r.get(5)?,
        strategy: r.get(6)?,
        consensus: r.get(7)?,
        composite: r.get(8)?,
        access_count: r.get(9)?,
        last_boosted_at: r.get(10)?,
        content_hash: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

/// 内容指纹（DefaultHasher，固定种子，跨运行稳定；非加密，仅作变更检测）。
fn content_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// 相对 vault 根的路径，统一正斜杠。
fn rel_path(vault: &Path, p: &Path) -> String {
    match p.strip_prefix(vault) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => p.to_string_lossy().replace('\\', "/"),
    }
}

/// FTS MATCH 查询清洗：拆词、去引号、每词加双引号、AND 连接。
fn fts_query(q: &str) -> String {
    q.split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .filter(|w| w != "\"\"")
        .collect::<Vec<_>>()
        .join(" ")
}

/// 递归遍历 .md 文件，跳过隐藏目录/文件（.obsidian/.compass/.git 等）。
fn walk_md(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_md_inner(root, &mut out)?;
    Ok(out)
}

fn walk_md_inner(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_md_inner(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

/// 解析单个笔记 → EntityRow（+ 正文）。无 `id` 字段返回 None（跳过）。
struct ParsedEntity {
    row: EntityRow,
    body: String,
    tags: Vec<String>,
    links: Vec<String>,
}

fn parse_entity(vault: &Path, path: &Path) -> Result<Option<ParsedEntity>> {
    let note = frontmatter::read_note(path)?;
    let fm: serde_yaml::Value =
        serde_yaml::from_str(&note.frontmatter).context("解析 frontmatter 失败")?;
    let m = fm
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("frontmatter 不是 mapping"))?;

    let id = match m
        .get(serde_yaml::Value::String("id".into()))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return Ok(None),
    };

    let get_str = |k: &str| {
        m.get(serde_yaml::Value::String(k.into()))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };

    let score = frontmatter::get_score(&note.frontmatter)?;
    let row = EntityRow {
        id,
        file_path: rel_path(vault, path),
        title: get_str("title"),
        layer: get_str("layer"),
        status: get_str("status"),
        interest: score.as_ref().map(|s| s.interest),
        strategy: score.as_ref().map(|s| s.strategy),
        consensus: score.as_ref().map(|s| s.consensus),
        composite: score.as_ref().map(|s| s.composite),
        access_count: score.as_ref().map(|s| s.access_count).unwrap_or(0),
        last_boosted_at: score.as_ref().map(|s| s.last_boosted_at.clone()),
        content_hash: Some(content_hash(&note.body)),
        updated_at: score.as_ref().map(|s| s.updated_at.clone()),
    };
    let tags = frontmatter::extract_tags(&note.frontmatter);
    let links = frontmatter::extract_refs(&note.body);
    Ok(Some(ParsedEntity {
        row,
        body: note.body,
        tags,
        links,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn md_with_id(id: &str, title: &str, body: &str) -> String {
        format!(
            "---\n\
             id: {id}\n\
             title: {title}\n\
             layer: knowledge\n\
             status: active\n\
             score:\n  interest: 85.0\n  strategy: 92.0\n  consensus: 78.0\n  composite: 85.3\n  updated_at: '2026-07-05T10:00:00+08:00'\n  last_boosted_at: '2026-07-05T10:00:00+08:00'\n  access_count: 12\n\
             ---\n\
             {body}\n"
        )
    }

    fn md_without_id(title: &str) -> String {
        format!("---\ntitle: {title}\n---\nbody text\n")
    }

    fn sample_row(id: &str) -> EntityRow {
        EntityRow {
            id: id.into(),
            file_path: format!("{id}.md"),
            title: Some("Game Theory".into()),
            layer: Some("knowledge".into()),
            status: Some("active".into()),
            interest: Some(85.0),
            strategy: Some(92.0),
            consensus: Some(78.0),
            composite: Some(85.3),
            access_count: 12,
            last_boosted_at: Some("2026-07-05T10:00:00+08:00".into()),
            content_hash: Some("abc".into()),
            updated_at: Some("2026-07-05T10:00:00+08:00".into()),
        }
    }

    #[test]
    fn schema_migrations_are_repeatable() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        db.init_schema().unwrap();
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);

        let suggestion_tables: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'suggestions'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(suggestion_tables, 1);
    }

    #[test]
    fn entity_relationship_indexes_are_replaceable_and_deletable() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "body").unwrap();
        db.replace_entity_relationships(
            "know-1",
            &["Rust".to_string(), "rust".to_string(), "SQLite".to_string()],
            &["know-2".to_string(), "know-2".to_string()],
        )
        .unwrap();
        assert_eq!(db.entity_tags("know-1").unwrap(), vec!["Rust", "SQLite"]);
        assert_eq!(db.entity_links("know-1").unwrap(), vec!["know-2"]);

        db.replace_entity_relationships("know-1", &["New".to_string()], &[])
            .unwrap();
        assert_eq!(db.entity_tags("know-1").unwrap(), vec!["New"]);
        assert!(db.entity_links("know-1").unwrap().is_empty());

        db.delete_entity("know-1").unwrap();
        assert!(db.entity_tags("know-1").unwrap().is_empty());
    }

    #[test]
    fn legacy_database_is_backed_up_before_migration() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("index.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                "CREATE TABLE entities (
                   id TEXT PRIMARY KEY, file_path TEXT UNIQUE NOT NULL, title TEXT,
                   layer TEXT, status TEXT, interest REAL, strategy REAL,
                   consensus REAL, composite REAL, access_count INTEGER NOT NULL DEFAULT 0,
                   last_boosted_at TEXT, content_hash TEXT, updated_at TEXT
                 );
                 CREATE TABLE score_history (
                   id INTEGER PRIMARY KEY AUTOINCREMENT, entity_id TEXT NOT NULL,
                   dimension TEXT, old REAL, new REAL, reason TEXT, trigger TEXT,
                   created_at TEXT NOT NULL
                 );
                 CREATE TABLE timeline (
                   id INTEGER PRIMARY KEY AUTOINCREMENT, entity_id TEXT NOT NULL,
                   event_type TEXT NOT NULL, intensity REAL, source TEXT,
                   created_at TEXT NOT NULL
                 );
                 CREATE INDEX idx_score_history_entity ON score_history(entity_id);
                 CREATE INDEX idx_score_history_trigger ON score_history(entity_id, trigger);
                 CREATE INDEX idx_timeline_entity ON timeline(entity_id);
                 CREATE VIRTUAL TABLE entities_fts USING fts5(title, content);
                 INSERT INTO entities (id, file_path, title, access_count)
                   VALUES ('know-legacy', 'legacy.md', 'Legacy', 0);",
            )
            .unwrap();
        }
        Connection::open(&path)
            .unwrap()
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .unwrap();
        let before = fs::read(&path).unwrap();

        let db = Db::open(&path).unwrap();
        let backup = path.with_extension("db.pre-migration.bak");
        assert!(backup.exists());
        assert_eq!(fs::read(&backup).unwrap(), before);
        assert_eq!(db.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
        let legacy_entity: String = db
            .conn
            .query_row(
                "SELECT id FROM entities WHERE id = 'know-legacy'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(legacy_entity, "know-legacy");
    }

    #[test]
    fn failed_migration_rolls_back_schema_changes() {
        let conn = Connection::open_in_memory().unwrap();
        let db = Db { conn };
        let tx = db.conn.unchecked_transaction().unwrap();

        assert!(apply_migration(&tx, 99).is_err());
        drop(tx);
        let probe_table_count: i64 = db
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'migration_probe'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(probe_table_count, 0);
    }

    #[test]
    fn test_open_in_memory_creates_schema() {
        let db = Db::open_in_memory().unwrap();
        // 表存在：插入查询不报错即证明
        let row = sample_row("know-000001");
        db.upsert_entity(&row, "body").unwrap();
        let got = db.get_entity("know-000001").unwrap().unwrap();
        assert_eq!(got.id, "know-000001");
        assert_eq!(got.title.as_deref(), Some("Game Theory"));
        assert!((got.composite.unwrap() - 85.3).abs() < 1e-9);
    }

    #[test]
    fn test_open_creates_db_file_and_parent() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("nested").join("index.db");
        {
            let db = Db::open(&db_path).unwrap();
            db.upsert_entity(&sample_row("know-1"), "b").unwrap();
        }
        assert!(db_path.exists(), "db 文件应被创建");
    }

    #[test]
    fn test_upsert_then_get_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let row = sample_row("know-000001");
        db.upsert_entity(&row, "Nash equilibrium is a core concept")
            .unwrap();
        let got = db.get_entity("know-000001").unwrap().unwrap();
        assert_eq!(got, row);
    }

    #[test]
    fn test_upsert_updates_existing() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "old body").unwrap();
        let mut row = sample_row("know-1");
        row.title = Some("Updated Title".into());
        row.composite = Some(50.0);
        db.upsert_entity(&row, "new body").unwrap();
        let got = db.get_entity("know-1").unwrap().unwrap();
        assert_eq!(got.title.as_deref(), Some("Updated Title"));
        assert!((got.composite.unwrap() - 50.0).abs() < 1e-9);
        // 实体不重复
        assert_eq!(db.list_entities().unwrap().len(), 1);
    }

    #[test]
    fn test_list_entities_ordered_by_composite_desc() {
        let db = Db::open_in_memory().unwrap();
        let mut low = sample_row("know-low");
        low.composite = Some(10.0);
        let mut high = sample_row("know-high");
        high.composite = Some(99.0);
        db.upsert_entity(&low, "b").unwrap();
        db.upsert_entity(&high, "b").unwrap();
        let list = db.list_entities().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "know-high");
        assert_eq!(list[1].id, "know-low");
    }

    #[test]
    fn test_list_entities_null_composite_last() {
        let db = Db::open_in_memory().unwrap();
        let mut with_score = sample_row("know-1");
        with_score.composite = Some(5.0);
        let mut no_score = sample_row("know-2");
        no_score.composite = None;
        db.upsert_entity(&with_score, "b").unwrap();
        db.upsert_entity(&no_score, "b").unwrap();
        let list = db.list_entities().unwrap();
        assert_eq!(list[0].id, "know-1");
        assert_eq!(list[1].id, "know-2");
    }

    #[test]
    fn test_fts_search_hit() {
        let db = Db::open_in_memory().unwrap();
        let row = sample_row("know-000001");
        db.upsert_entity(&row, "Nash equilibrium is a core concept in game theory")
            .unwrap();
        let hits = db.fts_search("Nash", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "know-000001");
        assert_eq!(hits[0].title.as_deref(), Some("Game Theory"));
        assert!(hits[0].snippet.as_deref().unwrap_or("").contains("Nash"));
    }

    #[test]
    fn test_fts_search_multi_word_and() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "Nash equilibrium game")
            .unwrap();
        db.upsert_entity(&sample_row("know-2"), "Nash only")
            .unwrap();
        let hits = db.fts_search("Nash equilibrium", 10).unwrap();
        assert_eq!(hits.len(), 1, "多词应 AND，只命中含两词的");
        assert_eq!(hits[0].id, "know-1");
    }

    #[test]
    fn test_fts_search_no_match() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "hello world")
            .unwrap();
        let hits = db.fts_search("nonexistent", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_fts_search_empty_query_returns_empty() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "hello").unwrap();
        assert!(db.fts_search("", 10).unwrap().is_empty());
        assert!(db.fts_search("   ", 10).unwrap().is_empty());
    }

    #[test]
    fn test_fts_search_limit() {
        let db = Db::open_in_memory().unwrap();
        for i in 0..5 {
            db.upsert_entity(&sample_row(&format!("know-{i}")), "common keyword")
                .unwrap();
        }
        let hits = db.fts_search("common", 2).unwrap();
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn test_fts_update_reflects_new_content() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "old keyword alpha")
            .unwrap();
        assert_eq!(db.fts_search("alpha", 10).unwrap().len(), 1);
        db.upsert_entity(&sample_row("know-1"), "new keyword beta")
            .unwrap();
        assert!(
            db.fts_search("alpha", 10).unwrap().is_empty(),
            "旧内容应被替换"
        );
        assert_eq!(db.fts_search("beta", 10).unwrap().len(), 1);
    }

    #[test]
    fn test_delete_entity_removes_fts() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-1"), "unique searchable text")
            .unwrap();
        assert_eq!(db.fts_search("unique", 10).unwrap().len(), 1);
        db.delete_entity("know-1").unwrap();
        assert!(db.get_entity("know-1").unwrap().is_none());
        assert!(db.fts_search("unique", 10).unwrap().is_empty());
    }

    #[test]
    fn test_delete_nonexistent_is_noop() {
        let db = Db::open_in_memory().unwrap();
        db.delete_entity("ghost").unwrap();
    }

    #[test]
    fn test_insert_and_get_score_history() {
        let db = Db::open_in_memory().unwrap();
        db.insert_score_history(&ScoreHistoryRow {
            entity_id: "know-1".into(),
            dimension: Some("interest".into()),
            old: Some(80.0),
            new: Some(82.0),
            reason: Some("cited".into()),
            trigger: Some("Cited".into()),
            created_at: "2026-07-05T10:00:00Z".into(),
        })
        .unwrap();
        let t = db.last_trigger_time("know-1", "Cited").unwrap();
        assert_eq!(t.as_deref(), Some("2026-07-05T10:00:00Z"));
    }

    #[test]
    fn test_last_trigger_time_returns_latest_by_insert_order() {
        let db = Db::open_in_memory().unwrap();
        // 故意让 created_at 字典序与插入序相反，验证按 id DESC（插入序）取最新
        db.insert_score_history(&ScoreHistoryRow {
            entity_id: "know-1".into(),
            dimension: None,
            old: None,
            new: None,
            reason: None,
            trigger: Some("Cited".into()),
            created_at: "2026-07-10T00:00:00Z".into(),
        })
        .unwrap();
        db.insert_score_history(&ScoreHistoryRow {
            entity_id: "know-1".into(),
            dimension: None,
            old: None,
            new: None,
            reason: None,
            trigger: Some("Cited".into()),
            created_at: "2026-07-01T00:00:00Z".into(),
        })
        .unwrap();
        let t = db.last_trigger_time("know-1", "Cited").unwrap();
        // 插入序最新 = 第二条（id 更大），即使 created_at 更早
        assert_eq!(t.as_deref(), Some("2026-07-01T00:00:00Z"));
    }

    #[test]
    fn test_last_trigger_time_none_when_no_record() {
        let db = Db::open_in_memory().unwrap();
        assert!(db.last_trigger_time("know-1", "Cited").unwrap().is_none());
    }

    #[test]
    fn test_last_trigger_time_filters_by_trigger_type() {
        let db = Db::open_in_memory().unwrap();
        db.insert_score_history(&ScoreHistoryRow {
            entity_id: "know-1".into(),
            dimension: None,
            old: None,
            new: None,
            reason: None,
            trigger: Some("Linked".into()),
            created_at: "2026-07-09T00:00:00Z".into(),
        })
        .unwrap();
        db.insert_score_history(&ScoreHistoryRow {
            entity_id: "know-1".into(),
            dimension: None,
            old: None,
            new: None,
            reason: None,
            trigger: Some("Cited".into()),
            created_at: "2026-07-05T00:00:00Z".into(),
        })
        .unwrap();
        let cited = db.last_trigger_time("know-1", "Cited").unwrap();
        assert_eq!(cited.as_deref(), Some("2026-07-05T00:00:00Z"));
        let linked = db.last_trigger_time("know-1", "Linked").unwrap();
        assert_eq!(linked.as_deref(), Some("2026-07-09T00:00:00Z"));
    }

    #[test]
    fn test_insert_timeline() {
        let db = Db::open_in_memory().unwrap();
        db.insert_timeline(&TimelineRow {
            entity_id: "know-1".into(),
            event_type: "access".into(),
            intensity: Some(2.0),
            source: Some("obsidian".into()),
            created_at: "2026-07-05T10:00:00Z".into(),
        })
        .unwrap();
        // 无直接查询接口，验证不报错即可（T1.5/T1.6 扩展查询）
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM timeline", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_rebuild_indexes_vault() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let note = md_with_id("know-000001", "Game Theory", "Nash equilibrium is core.")
            .replace(
                "title: Game Theory\n",
                "title: Game Theory\ntags:\n  - Rust\n  - '#rust'\n",
            )
            .replace(
                "Nash equilibrium is core.",
                "Nash equilibrium is core. [[know-000002]]",
            );
        fs::write(vault.join("know-000001.md"), note).unwrap();
        fs::write(vault.join("noid.md"), md_without_id("No ID")).unwrap();

        let db = Db::open_in_memory().unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1, "有 id 的应被索引");
        assert_eq!(stats.skipped, 1, "无 id 的应跳过");

        let got = db.get_entity("know-000001").unwrap().unwrap();
        assert_eq!(got.title.as_deref(), Some("Game Theory"));
        assert_eq!(got.file_path, "know-000001.md");
        assert!((got.composite.unwrap() - 85.3).abs() < 1e-9);
        assert_eq!(got.access_count, 12);
        assert!(got.content_hash.is_some());
        assert_eq!(db.entity_tags("know-000001").unwrap(), vec!["Rust"]);
        assert_eq!(db.entity_links("know-000001").unwrap(), vec!["know-000002"]);

        // FTS 可查
        let hits = db.fts_search("Nash", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "know-000001");
    }

    #[test]
    fn test_rebuild_is_idempotent() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("know-1.md"),
            md_with_id("know-1", "A", "content one"),
        )
        .unwrap();
        fs::write(
            vault.join("know-2.md"),
            md_with_id("know-2", "B", "content two"),
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        db.rebuild_from_vault(&vault).unwrap();
        db.rebuild_from_vault(&vault).unwrap();
        let list = db.list_entities().unwrap();
        assert_eq!(list.len(), 2, "rebuild 两次不应重复");
    }

    #[test]
    fn test_rebuild_duplicate_id_skipped() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("a.md"),
            md_with_id("know-dup", "First", "first body alpha"),
        )
        .unwrap();
        fs::write(
            vault.join("b.md"),
            md_with_id("know-dup", "Second", "second body beta"),
        )
        .unwrap();
        let db = Db::open_in_memory().unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1, "首个索引，重复的跳过");
        assert_eq!(stats.duplicates, 1, "重复 id 计数");
        // 恰好一个被索引（不依赖 read_dir 顺序）
        let hf = db.fts_search("alpha", 10).unwrap().len();
        let hs = db.fts_search("beta", 10).unwrap().len();
        assert_eq!(hf + hs, 1, "两个同 id 文件只索引一个");
    }
    #[test]
    fn test_rebuild_skips_hidden_dirs() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        let obsidian = vault.join(".obsidian");
        fs::create_dir_all(&obsidian).unwrap();
        fs::write(
            vault.join("know-1.md"),
            md_with_id("know-1", "Visible", "visible body"),
        )
        .unwrap();
        fs::write(
            obsidian.join("config.md"),
            md_with_id("hidden-1", "Hidden", "hidden body"),
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1);
        assert!(db.get_entity("hidden-1").unwrap().is_none());
        assert!(db.fts_search("hidden", 10).unwrap().is_empty());
        assert!(db.fts_search("visible", 10).unwrap().len() == 1);
    }

    #[test]
    fn test_rebuild_nested_subdirs() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        let nested = vault.join("Knowledge").join("Math");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            nested.join("know-1.md"),
            md_with_id("know-1", "Nested", "nested body"),
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        db.rebuild_from_vault(&vault).unwrap();
        let got = db.get_entity("know-1").unwrap().unwrap();
        assert_eq!(got.file_path, "Knowledge/Math/know-1.md");
    }

    #[test]
    fn test_rebuild_vault_sample() {
        // 使用固定 fixture，避免运行时评分改写跟踪 vault 后污染测试断言。
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(vault.join("Projects")).unwrap();
        fs::write(
            vault.join("Projects").join("compass-v2.md"),
            "---\n\
             id: proj-compass-v3\n\
             title: Compass V3\n\
             layer: direction\n\
             status: active\n\
             score:\n  interest: 9.5\n  strategy: 9.5\n  consensus: 7.0\n  composite: 8.9\n  updated_at: '2026-07-05T10:00:00+08:00'\n  last_boosted_at: '2026-07-05T10:00:00+08:00'\n  access_count: 1\n\
             ---\n\
             # Compass V3\n",
        )
        .unwrap();
        let db = Db::open_in_memory().unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1, "应索引 fixture compass-v2.md");
        let got = db.get_entity("proj-compass-v3").unwrap();
        assert!(
            got.is_some(),
            "fixture compass-v2.md 的 id=proj-compass-v3 应被索引"
        );
        let got = got.unwrap();
        assert_eq!(got.file_path, "Projects/compass-v2.md");
        assert!((got.composite.unwrap() - 8.9).abs() < 1e-9);
        assert_eq!(got.layer.as_deref(), Some("direction"));
    }

    #[test]
    fn test_content_hash_stable() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
    }

    #[test]
    fn test_fts_query_sanitizes() {
        assert_eq!(fts_query("Nash equilibrium"), "\"Nash\" \"equilibrium\"");
        assert_eq!(fts_query(""), "");
        assert_eq!(fts_query("   "), "");
        // 引号被剥离，避免注入 FTS 语法
        assert_eq!(fts_query("\"inject"), "\"inject\"");
    }
}
