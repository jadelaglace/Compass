//! SQLite �����㣨T1.4����entities / score_history / timeline / entities_fts��
//!
//! ��ƣ�PRD_v3.0 ��4.2����
//! - frontmatter ��Ȩ����SQLite ��������/����/��ʷ��ɾ��ɴ� vault �ؽ���
//! - entities �������ά���֣�interest/strategy/consensus + composite�����б��ѯ����ļ���
//! - score_history �����ֱ����ʷ��frontmatter ���棩������ T1.3 ��ȴ per-type ��ѯ
//!   ��`last_trigger_time` �� `id DESC` ȡ���¡���������ʱ���򣬹�� RFC3339 ʱ���������⣩��
//! - entities_fts ����ͨ FTS5 �ڲ����title, content����rowid �� entities ��ʽ rowid��
//!   ֧�� snippet���� Agent/Skill �ã�Obsidian ��������������
//! - rebuild ����� score_history/timeline����ʷ��־������¶���¼�ɽ��ܣ�T1.4 ��Χ�⣩��

#[cfg(test)]
use std::collections::hash_map::DefaultHasher;
#[cfg(test)]
use std::hash::{Hash, Hasher};
use std::path::Path;
#[cfg(test)]
use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension, Row, Transaction};

use crate::application::ports::{
    CachedSuggestion, IndexProjection, IndexSearchHit, IndexedEntity, RepositoryPort,
    ScoreHistoryEntry, SuggestionStats, TimelineEntry,
};
use crate::infrastructure::database_files::prepare_database_path;
#[cfg(test)]
use crate::infrastructure::vault_adapter as frontmatter;

/// entities ����о���vault �ļ����������棩��
#[derive(Debug, Clone, PartialEq)]
struct EntityRow {
    pub id: String,
    /// ��� vault ����·������б�ָܷ��
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
    /// ����д��ʱ�䣨ȡ�� frontmatter `score.updated_at`����
    pub updated_at: Option<String>,
}

/// score_history �У����ֱ����ʷ��frontmatter ���棩��
#[derive(Debug, Clone)]
struct ScoreHistoryRow {
    pub entity_id: String,
    pub dimension: Option<String>,
    pub old: Option<f64>,
    pub new: Option<f64>,
    pub reason: Option<String>,
    /// ��������������Cited/Linked/CaseAdded/ManualMark/ReviewCompleted/Decay/Access...����
    pub trigger: Option<String>,
    pub created_at: String,
}

/// timeline �У�����/����/�����¼�������
#[derive(Debug, Clone)]
struct TimelineRow {
    pub entity_id: String,
    pub event_type: String,
    pub intensity: Option<f64>,
    pub source: Option<String>,
    pub created_at: String,
}

/// FTS �������С�
#[derive(Debug, Clone, PartialEq)]
struct FtsHit {
    pub id: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
}

/// Persisted Phase 4 suggestion. Suggestions are rebuildable cache rows; Vault remains authoritative.
#[derive(Debug, Clone, PartialEq)]
struct SuggestionRow {
    pub suggestion_id: String,
    pub kind: String,
    pub entity_id: String,
    pub candidate: String,
    pub candidate_key: String,
    pub confidence: Option<f64>,
    pub reason: String,
    pub source: String,
    pub algorithm_version: String,
    pub content_hash: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SuggestionStatsRow {
    pub accepted: u64,
    pub rejected: u64,
    pub expired: u64,
}

/// ȫ���ؽ�ͳ�ơ�
#[derive(Debug, Default, Clone)]
#[cfg(test)]
struct RebuildStats {
    pub indexed: u32,
    pub skipped: u32,
    pub duplicates: u32,
}

pub(crate) struct SqliteRepository {
    conn: Connection,
}

#[cfg(test)]
type Db = SqliteRepository;

fn indexed_entity(row: EntityRow) -> IndexedEntity {
    IndexedEntity {
        id: row.id,
        file_path: row.file_path,
        title: row.title,
        layer: row.layer,
        status: row.status,
        interest: row.interest,
        strategy: row.strategy,
        consensus: row.consensus,
        composite: row.composite,
        access_count: row.access_count,
        last_boosted_at: row.last_boosted_at,
        content_hash: row.content_hash,
        updated_at: row.updated_at,
    }
}

fn entity_row(entity: &IndexedEntity) -> EntityRow {
    EntityRow {
        id: entity.id.clone(),
        file_path: entity.file_path.clone(),
        title: entity.title.clone(),
        layer: entity.layer.clone(),
        status: entity.status.clone(),
        interest: entity.interest,
        strategy: entity.strategy,
        consensus: entity.consensus,
        composite: entity.composite,
        access_count: entity.access_count,
        last_boosted_at: entity.last_boosted_at.clone(),
        content_hash: entity.content_hash.clone(),
        updated_at: entity.updated_at.clone(),
    }
}

fn projection_row(projection: &IndexProjection) -> EntityRow {
    EntityRow {
        id: projection.id.clone(),
        file_path: projection.file_path.clone(),
        title: projection.title.clone(),
        layer: projection.layer.clone(),
        status: projection.status.clone(),
        interest: projection.score.as_ref().map(|score| score.interest),
        strategy: projection.score.as_ref().map(|score| score.strategy),
        consensus: projection.score.as_ref().map(|score| score.consensus),
        composite: projection.score.as_ref().map(|score| score.composite),
        access_count: projection
            .score
            .as_ref()
            .map(|score| score.access_count)
            .unwrap_or_default(),
        last_boosted_at: projection
            .score
            .as_ref()
            .map(|score| score.last_boosted_at.clone()),
        content_hash: projection.content_hash.clone(),
        updated_at: projection
            .score
            .as_ref()
            .map(|score| score.updated_at.clone()),
    }
}

const CURRENT_SCHEMA_VERSION: i64 = 2;

impl SqliteRepository {
    /// �򿪣��򴴽������ݿ��ļ�����ʼ�� schema����Ŀ¼�Զ�������
    pub(crate) fn open(path: &Path) -> Result<Self> {
        prepare_database_path(path)?;
        let conn =
            Connection::open(path).with_context(|| format!("�����ݿ�ʧ�� {}", path.display()))?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    /// �ڴ����ݿ⣨�����ã���
    #[cfg(test)]
    pub(crate) fn open_in_memory() -> Result<Self> {
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

    fn schema_version(&self) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT version FROM schema_version WHERE id = 1",
            [],
            |row| row.get(0),
        )?)
    }

    /// upsert entity ��ͬ�� FTS��`fts_content` = Markdown ���ģ��� FTS ������ snippet����
    fn upsert_entity(&self, e: &EntityRow, fts_content: &str) -> Result<()> {
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
        // FTS5 �ڲ�����Ȱ� rowid ɾ�ɣ�������Ҳ��ȫ�����ٲ��¡�
        tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rowid])?;
        tx.execute(
            "INSERT INTO entities_fts (rowid, title, content) VALUES (?1, ?2, ?3)",
            params![rowid, e.title.as_deref().unwrap_or(""), fts_content],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn upsert_entity_with_relationships(
        &self,
        e: &EntityRow,
        fts_content: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        let previous_id: Option<String> = tx
            .query_row(
                "SELECT id FROM entities WHERE file_path = ?1",
                params![e.file_path],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(previous_id) = previous_id.filter(|id| id != &e.id) {
            let previous_rowid: i64 = tx.query_row(
                "SELECT rowid FROM entities WHERE id = ?1",
                params![previous_id],
                |row| row.get(0),
            )?;
            tx.execute(
                "DELETE FROM entities_fts WHERE rowid = ?1",
                params![previous_rowid],
            )?;
            tx.execute(
                "DELETE FROM entity_tags WHERE entity_id = ?1",
                params![previous_id],
            )?;
            tx.execute(
                "DELETE FROM entity_links WHERE source_id = ?1",
                params![previous_id],
            )?;
            tx.execute("DELETE FROM entities WHERE id = ?1", params![previous_id])?;
        }
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
            |row| row.get(0),
        )?;
        tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rowid])?;
        tx.execute(
            "INSERT INTO entities_fts (rowid, title, content) VALUES (?1, ?2, ?3)",
            params![rowid, e.title.as_deref().unwrap_or(""), fts_content],
        )?;
        tx.execute(
            "DELETE FROM entity_tags WHERE entity_id = ?1",
            params![e.id],
        )?;
        tx.execute(
            "DELETE FROM entity_links WHERE source_id = ?1",
            params![e.id],
        )?;
        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO entity_tags (entity_id, tag, tag_key)
                 VALUES (?1, ?2, ?3)",
                params![e.id, tag, tag.to_lowercase()],
            )?;
        }
        for target_id in links {
            tx.execute(
                "INSERT OR IGNORE INTO entity_links (source_id, target_id)
                 VALUES (?1, ?2)",
                params![e.id, target_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_entity(&self, id: &str) -> Result<Option<EntityRow>> {
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

    fn delete_entities_under_path(&self, path: &str) -> Result<()> {
        let prefix = path.trim_end_matches('/');
        let path_prefix = format!("{prefix}/");
        let entities = {
            let mut stmt = self
                .conn
                .prepare("SELECT id, rowid, file_path FROM entities")?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let tx = self.conn.unchecked_transaction()?;
        for (id, rowid, _) in entities
            .into_iter()
            .filter(|(_, _, file_path)| file_path == prefix || file_path.starts_with(&path_prefix))
        {
            tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rowid])?;
            tx.execute("DELETE FROM entity_tags WHERE entity_id = ?1", params![id])?;
            tx.execute("DELETE FROM entity_links WHERE source_id = ?1", params![id])?;
            tx.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// �� composite ���򷵻أ�NULL ��Ȼ����󣩡�
    fn list_entities(&self) -> Result<Vec<EntityRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_path, title, layer, status, interest, strategy, consensus,
                    composite, access_count, last_boosted_at, content_hash, updated_at
             FROM entities ORDER BY composite DESC, id",
        )?;
        let rows = stmt.query_map([], row_to_entity)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// ɾ��ʵ�岢������ FTS ��¼��
    #[cfg(test)]
    fn delete_entity(&self, id: &str) -> Result<()> {
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
        tx.execute("DELETE FROM entity_links WHERE source_id = ?1", params![id])?;
        tx.execute("DELETE FROM entities WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(())
    }

    #[allow(dead_code)]
    fn replace_entity_relationships(
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
    fn entity_tags(&self, entity_id: &str) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM entity_tags WHERE entity_id = ?1 ORDER BY tag_key, tag")?;
        let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    #[allow(dead_code)]
    fn entity_links(&self, entity_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT target_id FROM entity_links WHERE source_id = ?1 ORDER BY target_id",
        )?;
        let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    #[allow(dead_code)]
    fn directly_linked_entities(&self, entity_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT target_id FROM entity_links WHERE source_id = ?1
             UNION
             SELECT source_id FROM entity_links WHERE target_id = ?1
             ORDER BY 1",
        )?;
        let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn upsert_suggestion(&self, suggestion: &SuggestionRow) -> Result<()> {
        self.conn.execute(
            "INSERT INTO suggestions
               (suggestion_id, kind, entity_id, candidate, candidate_key, confidence,
                reason, source, algorithm_version, content_hash, status, created_at, updated_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(suggestion_id) DO UPDATE SET
               confidence=excluded.confidence, reason=excluded.reason,
               content_hash=excluded.content_hash, updated_at=excluded.updated_at",
            params![
                suggestion.suggestion_id,
                suggestion.kind,
                suggestion.entity_id,
                suggestion.candidate,
                suggestion.candidate_key,
                suggestion.confidence,
                suggestion.reason,
                suggestion.source,
                suggestion.algorithm_version,
                suggestion.content_hash,
                suggestion.status,
                suggestion.created_at,
                suggestion.updated_at,
            ],
        )?;
        Ok(())
    }

    fn get_suggestion(&self, suggestion_id: &str) -> Result<Option<SuggestionRow>> {
        self.conn
            .query_row(
                "SELECT suggestion_id, kind, entity_id, candidate, candidate_key, confidence,
                        reason, source, algorithm_version, content_hash, status, created_at, updated_at
                 FROM suggestions WHERE suggestion_id = ?1",
                params![suggestion_id],
                suggestion_from_row,
            )
            .optional()
            .map_err(Into::into)
    }

    fn update_suggestion_status(
        &self,
        suggestion_id: &str,
        status: &str,
        updated_at: &str,
    ) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE suggestions SET status = ?1, updated_at = ?2 WHERE suggestion_id = ?3",
            params![status, updated_at, suggestion_id],
        )? > 0)
    }

    /// FTS5 ������query ���հײ�ʡ����Լ����ź� AND ���ӣ����� `-`/`*` �ȱ����� FTS �﷨����
    fn fts_search(&self, query: &str, limit: u32) -> Result<Vec<FtsHit>> {
        if contains_cjk(query) {
            return self.cjk_substring_search(query, limit);
        }
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

    fn cjk_substring_search(&self, query: &str, limit: u32) -> Result<Vec<FtsHit>> {
        if limit == 0 {
            return Ok(vec![]);
        }
        let terms = query
            .split_whitespace()
            .filter(|term| !term.is_empty())
            .map(str::to_lowercase)
            .collect::<Vec<_>>();
        if terms.is_empty() {
            return Ok(vec![]);
        }

        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.title, f.content
             FROM entities_fts f
             JOIN entities e ON e.rowid = f.rowid
             ORDER BY e.composite IS NULL, e.composite DESC, e.id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut hits = Vec::new();
        for row in rows {
            let (id, title, content) = row?;
            let title_lowercase = title.as_deref().unwrap_or("").to_lowercase();
            let content_lowercase = content.to_lowercase();
            let searchable = format!("{title_lowercase}\n{content_lowercase}");
            if terms.iter().all(|term| searchable.contains(term)) {
                let snippet_source = if content_lowercase.contains(&terms[0]) {
                    content.as_str()
                } else {
                    title.as_deref().unwrap_or(content.as_str())
                };
                let snippet = search_snippet(snippet_source, &terms[0]);
                hits.push(FtsHit {
                    id,
                    title,
                    snippet: Some(snippet),
                });
                if hits.len() >= limit as usize {
                    break;
                }
            }
        }
        Ok(hits)
    }

    /// ��ʵ��ĳ�������������һ�δ���ʱ�䣨�� T1.3 ��ȴ�жϣ���
    /// �� score_history ���� `id DESC` ȡ���¡���������ʱ���򣬹�� RFC3339 ʱ���������塣
    fn last_trigger_time(&self, entity_id: &str, trigger: &str) -> Result<Option<String>> {
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

    fn insert_score_history(&self, h: &ScoreHistoryRow) -> Result<()> {
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

    fn insert_timeline(&self, t: &TimelineRow) -> Result<()> {
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

    fn score_history_between(&self, from: &str, to: &str) -> Result<Vec<ScoreHistoryRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_id, dimension, old, new, reason, trigger, created_at
             FROM score_history
             WHERE created_at >= ?1 AND created_at < ?2
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(ScoreHistoryRow {
                entity_id: row.get(0)?,
                dimension: row.get(1)?,
                old: row.get(2)?,
                new: row.get(3)?,
                reason: row.get(4)?,
                trigger: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn timeline_between(&self, from: &str, to: &str) -> Result<Vec<TimelineRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT entity_id, event_type, intensity, source, created_at
             FROM timeline
             WHERE created_at >= ?1 AND created_at < ?2
             ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok(TimelineRow {
                entity_id: row.get(0)?,
                event_type: row.get(1)?,
                intensity: row.get(2)?,
                source: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn suggestion_stats_between(&self, from: &str, to: &str) -> Result<SuggestionStatsRow> {
        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*)
             FROM suggestions
             WHERE updated_at >= ?1 AND updated_at < ?2
               AND status IN ('accepted', 'rejected', 'expired')
             GROUP BY status",
        )?;
        let rows = stmt.query_map(params![from, to], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, u64>(1)?))
        })?;
        let mut stats = SuggestionStatsRow::default();
        for row in rows {
            let (status, count) = row?;
            match status.as_str() {
                "accepted" => stats.accepted = count,
                "rejected" => stats.rejected = count,
                "expired" => stats.expired = count,
                _ => {}
            }
        }
        Ok(stats)
    }

    fn has_report_history(&self) -> Result<bool> {
        self.conn
            .query_row(
                "SELECT EXISTS(
                SELECT 1 FROM score_history
                UNION ALL SELECT 1 FROM timeline
                UNION ALL SELECT 1 FROM suggestions
             )",
                [],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    /// �� vault ȫ���ؽ���������� entities + FTS������ɨ��д�룩��
    /// score_history/timeline ����գ���ʷ��������� `id` frontmatter �ıʼ�������
    #[cfg(test)]
    fn rebuild_from_vault(&self, vault: &Path) -> Result<RebuildStats> {
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
                        tracing::warn!(id = %parsed.row.id, path = %path.display(), "�ظ� id�����������ļ�");
                        stats.duplicates += 1;
                        continue;
                    }
                    if let Err(e) = self.upsert_entity_with_relationships(
                        &parsed.row,
                        &parsed.body,
                        &parsed.tags,
                        &parsed.links,
                    ) {
                        tracing::warn!(path = %path.display(), err = %e, "����д��ʧ�ܣ�����");
                        stats.skipped += 1;
                    } else {
                        stats.indexed += 1;
                    }
                }
                Ok(None) => stats.skipped += 1,
                Err(e) => {
                    tracing::warn!(path = %path.display(), err = %e, "����ʧ�ܣ�����");
                    stats.skipped += 1;
                }
            }
        }
        Ok(stats)
    }
}

fn upsert_projection_tx(
    tx: &Transaction<'_>,
    entity: &EntityRow,
    fts_content: &str,
    tags: &[String],
    links: &[String],
) -> Result<()> {
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
            entity.id,
            entity.file_path,
            entity.title,
            entity.layer,
            entity.status,
            entity.interest,
            entity.strategy,
            entity.consensus,
            entity.composite,
            entity.access_count,
            entity.last_boosted_at,
            entity.content_hash,
            entity.updated_at,
        ],
    )?;
    let rowid: i64 = tx.query_row(
        "SELECT rowid FROM entities WHERE id = ?1",
        params![entity.id],
        |row| row.get(0),
    )?;
    tx.execute("DELETE FROM entities_fts WHERE rowid = ?1", params![rowid])?;
    tx.execute(
        "INSERT INTO entities_fts (rowid, title, content) VALUES (?1, ?2, ?3)",
        params![rowid, entity.title.as_deref().unwrap_or(""), fts_content],
    )?;
    tx.execute(
        "DELETE FROM entity_tags WHERE entity_id = ?1",
        params![entity.id],
    )?;
    tx.execute(
        "DELETE FROM entity_links WHERE source_id = ?1",
        params![entity.id],
    )?;
    for tag in tags {
        tx.execute(
            "INSERT OR IGNORE INTO entity_tags (entity_id, tag, tag_key) VALUES (?1, ?2, ?3)",
            params![entity.id, tag, tag.to_lowercase()],
        )?;
    }
    for target_id in links {
        tx.execute(
            "INSERT OR IGNORE INTO entity_links (source_id, target_id) VALUES (?1, ?2)",
            params![entity.id, target_id],
        )?;
    }
    Ok(())
}

impl RepositoryPort for SqliteRepository {
    fn schema_version(&self) -> Result<i64> {
        SqliteRepository::schema_version(self)
    }

    fn replace_index_projections(&self, projections: &[IndexProjection]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM entities", [])?;
        tx.execute("DELETE FROM entities_fts", [])?;
        tx.execute("DELETE FROM entity_tags", [])?;
        tx.execute("DELETE FROM entity_links", [])?;
        for projection in projections {
            let row = projection_row(projection);
            upsert_projection_tx(
                &tx,
                &row,
                &projection.body,
                &projection.tags,
                &projection.links,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    fn upsert_index_projection(&self, projection: &IndexProjection) -> Result<()> {
        let row = projection_row(projection);
        self.upsert_entity_with_relationships(
            &row,
            &projection.body,
            &projection.tags,
            &projection.links,
        )
    }

    fn upsert_indexed_entity(&self, entity: &IndexedEntity, body: &str) -> Result<()> {
        self.upsert_entity(&entity_row(entity), body)
    }

    fn upsert_indexed_entity_with_relationships(
        &self,
        entity: &IndexedEntity,
        body: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()> {
        self.upsert_entity_with_relationships(&entity_row(entity), body, tags, links)
    }

    fn entity_exists(&self, id: &str) -> Result<bool> {
        Ok(self.get_entity(id)?.is_some())
    }

    fn entity_file_path(&self, id: &str) -> Result<Option<String>> {
        Ok(self.get_entity(id)?.map(|row| row.file_path))
    }

    fn get_entity(&self, id: &str) -> Result<Option<IndexedEntity>> {
        SqliteRepository::get_entity(self, id).map(|row| row.map(indexed_entity))
    }

    fn list_entities(&self) -> Result<Vec<IndexedEntity>> {
        SqliteRepository::list_entities(self)
            .map(|rows| rows.into_iter().map(indexed_entity).collect())
    }

    fn delete_entities_under_path(&self, path: &str) -> Result<()> {
        SqliteRepository::delete_entities_under_path(self, path)
    }

    fn replace_entity_relationships(
        &self,
        entity_id: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()> {
        SqliteRepository::replace_entity_relationships(self, entity_id, tags, links)
    }

    fn directly_linked_entities(&self, entity_id: &str) -> Result<Vec<String>> {
        SqliteRepository::directly_linked_entities(self, entity_id)
    }

    fn entity_tags(&self, entity_id: &str) -> Result<Vec<String>> {
        SqliteRepository::entity_tags(self, entity_id)
    }

    fn entity_links(&self, entity_id: &str) -> Result<Vec<String>> {
        SqliteRepository::entity_links(self, entity_id)
    }

    fn search(&self, query: &str, limit: u32) -> Result<Vec<IndexSearchHit>> {
        SqliteRepository::fts_search(self, query, limit).map(|hits| {
            hits.into_iter()
                .map(|hit| IndexSearchHit {
                    id: hit.id,
                    title: hit.title,
                    snippet: hit.snippet,
                })
                .collect()
        })
    }

    fn upsert_suggestion(&self, suggestion: &CachedSuggestion) -> Result<()> {
        SqliteRepository::upsert_suggestion(
            self,
            &SuggestionRow {
                suggestion_id: suggestion.suggestion_id.clone(),
                kind: suggestion.kind.clone(),
                entity_id: suggestion.entity_id.clone(),
                candidate: suggestion.candidate.clone(),
                candidate_key: suggestion.candidate_key.clone(),
                confidence: suggestion.confidence,
                reason: suggestion.reason.clone(),
                source: suggestion.source.clone(),
                algorithm_version: suggestion.algorithm_version.clone(),
                content_hash: suggestion.content_hash.clone(),
                status: suggestion.status.clone(),
                created_at: suggestion.created_at.clone(),
                updated_at: suggestion.updated_at.clone(),
            },
        )
    }

    fn get_suggestion(&self, suggestion_id: &str) -> Result<Option<CachedSuggestion>> {
        SqliteRepository::get_suggestion(self, suggestion_id).map(|row| {
            row.map(|row| CachedSuggestion {
                suggestion_id: row.suggestion_id,
                kind: row.kind,
                entity_id: row.entity_id,
                candidate: row.candidate,
                candidate_key: row.candidate_key,
                confidence: row.confidence,
                reason: row.reason,
                source: row.source,
                algorithm_version: row.algorithm_version,
                content_hash: row.content_hash,
                status: row.status,
                created_at: row.created_at,
                updated_at: row.updated_at,
            })
        })
    }

    fn update_suggestion_status(
        &self,
        suggestion_id: &str,
        status: &str,
        updated_at: &str,
    ) -> Result<bool> {
        SqliteRepository::update_suggestion_status(self, suggestion_id, status, updated_at)
    }

    fn last_trigger_time(&self, entity_id: &str, trigger: &str) -> Result<Option<String>> {
        SqliteRepository::last_trigger_time(self, entity_id, trigger)
    }

    fn update_index_score(&self, id: &str, score: &crate::domain::entity::Score) -> Result<()> {
        self.conn.execute(
            "UPDATE entities SET interest = ?2, strategy = ?3, consensus = ?4, composite = ?5, access_count = ?6, last_boosted_at = ?7, updated_at = ?8 WHERE id = ?1",
            params![id, score.interest, score.strategy, score.consensus, score.composite, score.access_count, score.last_boosted_at, score.updated_at],
        )?;
        Ok(())
    }

    fn record_score_history(&self, entry: &ScoreHistoryEntry) -> Result<()> {
        SqliteRepository::insert_score_history(
            self,
            &ScoreHistoryRow {
                entity_id: entry.entity_id.clone(),
                dimension: entry.dimension.clone(),
                old: entry.old,
                new: entry.new,
                reason: entry.reason.clone(),
                trigger: entry.trigger.clone(),
                created_at: entry.created_at.clone(),
            },
        )
    }

    fn record_timeline(&self, entry: &TimelineEntry) -> Result<()> {
        SqliteRepository::insert_timeline(
            self,
            &TimelineRow {
                entity_id: entry.entity_id.clone(),
                event_type: entry.event_type.clone(),
                intensity: entry.intensity,
                source: entry.source.clone(),
                created_at: entry.created_at.clone(),
            },
        )
    }

    fn score_history_between(&self, from: &str, to: &str) -> Result<Vec<ScoreHistoryEntry>> {
        SqliteRepository::score_history_between(self, from, to).map(|rows| {
            rows.into_iter()
                .map(|row| ScoreHistoryEntry {
                    entity_id: row.entity_id,
                    dimension: row.dimension,
                    old: row.old,
                    new: row.new,
                    reason: row.reason,
                    trigger: row.trigger,
                    created_at: row.created_at,
                })
                .collect()
        })
    }

    fn timeline_between(&self, from: &str, to: &str) -> Result<Vec<TimelineEntry>> {
        SqliteRepository::timeline_between(self, from, to).map(|rows| {
            rows.into_iter()
                .map(|row| TimelineEntry {
                    entity_id: row.entity_id,
                    event_type: row.event_type,
                    intensity: row.intensity,
                    source: row.source,
                    created_at: row.created_at,
                })
                .collect()
        })
    }

    fn suggestion_stats_between(&self, from: &str, to: &str) -> Result<SuggestionStats> {
        SqliteRepository::suggestion_stats_between(self, from, to).map(|stats| SuggestionStats {
            accepted: stats.accepted,
            rejected: stats.rejected,
            expired: stats.expired,
        })
    }

    fn has_report_history(&self) -> Result<bool> {
        SqliteRepository::has_report_history(self)
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

fn suggestion_from_row(r: &Row) -> rusqlite::Result<SuggestionRow> {
    Ok(SuggestionRow {
        suggestion_id: r.get(0)?,
        kind: r.get(1)?,
        entity_id: r.get(2)?,
        candidate: r.get(3)?,
        candidate_key: r.get(4)?,
        confidence: r.get(5)?,
        reason: r.get(6)?,
        source: r.get(7)?,
        algorithm_version: r.get(8)?,
        content_hash: r.get(9)?,
        status: r.get(10)?,
        created_at: r.get(11)?,
        updated_at: r.get(12)?,
    })
}

/// ����ָ�ƣ�DefaultHasher���̶����ӣ��������ȶ����Ǽ��ܣ����������⣩��
#[cfg(test)]
fn content_hash(s: &str) -> String {
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

/// ��� vault ����·����ͳһ��б�ܡ�
#[cfg(test)]
fn rel_path(vault: &Path, p: &Path) -> String {
    match p.strip_prefix(vault) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        Err(_) => p.to_string_lossy().replace('\\', "/"),
    }
}

/// FTS MATCH ��ѯ��ϴ����ʡ�ȥ���š�ÿ�ʼ�˫���š�AND ���ӡ�
fn fts_query(q: &str) -> String {
    q.split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .filter(|w| w != "\"\"")
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(|character| {
        matches!(
            character as u32,
            0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xF900..=0xFAFF
                | 0x3040..=0x30FF
                | 0xAC00..=0xD7AF
        )
    })
}

fn search_snippet(content: &str, term: &str) -> String {
    const CONTEXT_BEFORE: usize = 40;
    const MAX_CHARS: usize = 160;

    let lowercase = content.to_lowercase();
    let match_byte = lowercase.find(term).unwrap_or(0);
    let match_char = lowercase[..match_byte].chars().count();
    let start = match_char.saturating_sub(CONTEXT_BEFORE);
    let snippet = content
        .chars()
        .skip(start)
        .take(MAX_CHARS)
        .collect::<String>();
    let prefix = if start > 0 { "..." } else { "" };
    let suffix = if content.chars().count() > start + MAX_CHARS {
        "..."
    } else {
        ""
    };
    format!("{prefix}{snippet}{suffix}")
}

/// �ݹ���� .md �ļ�����������Ŀ¼/�ļ���.obsidian/.compass/.git �ȣ���
#[cfg(test)]
fn walk_md(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk_md_inner(root, &mut out)?;
    Ok(out)
}

#[cfg(test)]
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
            if name == "Templates" {
                continue;
            }
            walk_md_inner(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

/// ���������ʼ� �� EntityRow��+ ���ģ����� `id` �ֶη��� None����������
#[cfg(test)]
struct ParsedEntity {
    row: EntityRow,
    body: String,
    tags: Vec<String>,
    links: Vec<String>,
}

#[cfg(test)]
fn parse_entity(vault: &Path, path: &Path) -> Result<Option<ParsedEntity>> {
    let note = frontmatter::read_note(path)?;
    if frontmatter::has_unrendered_templater_marker(&note.frontmatter) {
        return Ok(None);
    }
    let fm: serde_yaml::Value =
        serde_yaml::from_str(&note.frontmatter).context("���� frontmatter ʧ��")?;
    let m = fm
        .as_mapping()
        .ok_or_else(|| anyhow::anyhow!("frontmatter ���� mapping"))?;

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
        db.upsert_entity(&sample_row("know-2"), "body").unwrap();
        assert_eq!(db.entity_tags("know-1").unwrap(), vec!["Rust", "SQLite"]);
        assert_eq!(db.entity_links("know-1").unwrap(), vec!["know-2"]);

        db.replace_entity_relationships("know-1", &["New".to_string()], &[])
            .unwrap();
        assert_eq!(db.entity_tags("know-1").unwrap(), vec!["New"]);
        assert!(db.entity_links("know-1").unwrap().is_empty());

        db.replace_entity_relationships("know-1", &[], &["know-2".to_string()])
            .unwrap();
        db.delete_entity("know-2").unwrap();
        assert_eq!(db.entity_links("know-1").unwrap(), vec!["know-2"]);

        db.delete_entity("know-1").unwrap();
        assert!(db.entity_tags("know-1").unwrap().is_empty());
    }

    #[test]
    fn deleting_a_literal_directory_prefix_does_not_use_sql_wildcards() {
        let db = Db::open_in_memory().unwrap();
        let mut literal = sample_row("know-literal");
        literal.file_path = "folder%/note.md".to_string();
        let mut other = sample_row("know-other");
        other.file_path = "folderX/note.md".to_string();
        db.upsert_entity(&literal, "body").unwrap();
        db.upsert_entity(&other, "body").unwrap();

        db.delete_entities_under_path("folder%").unwrap();
        assert!(db.get_entity("know-literal").unwrap().is_none());
        assert!(db.get_entity("know-other").unwrap().is_some());
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
        // ����ڣ������ѯ�������֤��
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
        assert!(db_path.exists(), "db �ļ�Ӧ������");
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
        // ʵ�岻�ظ�
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
        assert_eq!(hits.len(), 1, "���Ӧ AND��ֻ���к����ʵ�");
        assert_eq!(hits[0].id, "know-1");
    }

    #[test]
    fn test_fts_search_cjk_substring() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(
            &sample_row("know-cn"),
            "Compass uses three dimensions to organize knowledge and 知识.",
        )
        .unwrap();

        let hits = db.fts_search("知识", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "know-cn");
        assert!(hits[0].snippet.as_deref().unwrap().contains("知识"));
    }

    #[test]
    fn test_fts_search_cjk_terms_use_and_semantics() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("know-both"), "������������֪ʶ����")
            .unwrap();
        db.upsert_entity(&sample_row("know-one"), "ֻ����������")
            .unwrap();

        let hits = db.fts_search("���� ֪ʶ", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "know-both");
    }

    #[test]
    fn test_fts_search_cjk_title_hit_and_zero_limit() {
        let db = Db::open_in_memory().unwrap();
        let mut row = sample_row("know-title");
        row.title = Some("认知 science notes".to_string());
        db.upsert_entity(&row, "unrelated body").unwrap();

        let hits = db.fts_search("认知", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.as_deref().unwrap().contains("认知"));
        assert!(db.fts_search("认知", 0).unwrap().is_empty());
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
            "������Ӧ���滻"
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
        // ������ created_at �ֵ�����������෴����֤�� id DESC��������ȡ����
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
        // ���������� = �ڶ�����id ���󣩣���ʹ created_at ����
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
        // ��ֱ�Ӳ�ѯ�ӿڣ���֤��������ɣ�T1.5/T1.6 ��չ��ѯ��
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
        assert_eq!(stats.indexed, 1, "�� id ��Ӧ������");
        assert_eq!(stats.skipped, 1, "�� id ��Ӧ����");

        let got = db.get_entity("know-000001").unwrap().unwrap();
        assert_eq!(got.title.as_deref(), Some("Game Theory"));
        assert_eq!(got.file_path, "know-000001.md");
        assert!((got.composite.unwrap() - 85.3).abs() < 1e-9);
        assert_eq!(got.access_count, 12);
        assert!(got.content_hash.is_some());
        assert_eq!(db.entity_tags("know-000001").unwrap(), vec!["Rust"]);
        assert_eq!(db.entity_links("know-000001").unwrap(), vec!["know-000002"]);

        // FTS �ɲ�
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
        assert_eq!(list.len(), 2, "rebuild ���β�Ӧ�ظ�");
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
        assert_eq!(stats.indexed, 1, "�׸��������ظ�������");
        assert_eq!(stats.duplicates, 1, "�ظ� id ����");
        // ǡ��һ���������������� read_dir ˳��
        let hf = db.fts_search("alpha", 10).unwrap().len();
        let hs = db.fts_search("beta", 10).unwrap().len();
        assert_eq!(hf + hs, 1, "����ͬ id �ļ�ֻ����һ��");
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
    fn test_rebuild_skips_templates_at_any_depth() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(vault.join("Templates")).unwrap();
        fs::create_dir_all(vault.join("Knowledge").join("Templates")).unwrap();
        fs::write(
            vault.join("Templates").join("knowledge.md"),
            md_with_id("template-root", "Root template", "template root body"),
        )
        .unwrap();
        fs::write(
            vault.join("Knowledge").join("Templates").join("case.md"),
            md_with_id("template-nested", "Nested template", "template nested body"),
        )
        .unwrap();
        fs::create_dir_all(vault.join("Knowledge").join("Math")).unwrap();
        fs::write(
            vault.join("Knowledge").join("Math").join("know-1.md"),
            md_with_id("know-1", "Indexed", "normal recursive body"),
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1);
        assert!(db.get_entity("template-root").unwrap().is_none());
        assert!(db.get_entity("template-nested").unwrap().is_none());
        assert!(db.fts_search("template", 10).unwrap().is_empty());
        assert!(db.get_entity("know-1").unwrap().is_some());
    }

    #[test]
    fn test_rebuild_skips_unrendered_templater_frontmatter_and_cleans_dirty_index() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("unrendered.md"),
            md_with_id(
                "case-<% tp.date.now(\"YYMMDD\") %>",
                "<% tp.file.title %>",
                "template source body",
            ),
        )
        .unwrap();
        fs::write(
            vault.join("know-1.md"),
            md_with_id("know-1", "Indexed", "normal body"),
        )
        .unwrap();

        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(&sample_row("dirty-template"), "stale template entry")
            .unwrap();
        let stats = db.rebuild_from_vault(&vault).unwrap();
        assert_eq!(stats.indexed, 1);
        assert_eq!(stats.skipped, 1);
        assert!(db.get_entity("dirty-template").unwrap().is_none());
        assert!(db
            .get_entity("case-<% tp.date.now(\"YYMMDD\") %>")
            .unwrap()
            .is_none());
        assert!(db.fts_search("template", 10).unwrap().is_empty());
        assert!(db.get_entity("know-1").unwrap().is_some());
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
        // ʹ�ù̶� fixture����������ʱ���ָ�д���� vault ����Ⱦ���Զ��ԡ�
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
        assert_eq!(stats.indexed, 1, "Ӧ���� fixture compass-v2.md");
        let got = db.get_entity("proj-compass-v3").unwrap();
        assert!(
            got.is_some(),
            "fixture compass-v2.md �� id=proj-compass-v3 Ӧ������"
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
        // ���ű����룬����ע�� FTS �﷨
        assert_eq!(fts_query("\"inject"), "\"inject\"");
    }

    /// TC-V03: Vault ��Ȩ������ʹ SQLite ������ʱ��һ�£�rebuild Ҳ�ܴ� Vault �ָ���
    #[test]
    fn rebuild_restores_index_from_authoritative_vault() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        let empty_vault = dir.path().join("empty");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&empty_vault).unwrap();
        let original = md_with_id("know-recover", "Recover", "original body");
        let path = vault.join("know-recover.md");
        fs::write(&path, &original).unwrap();

        let db = Db::open_in_memory().unwrap();
        db.rebuild_from_vault(&vault).unwrap();
        assert!(db.get_entity("know-recover").unwrap().is_some());

        // ģ�� SQLite ������ʧ��rebuild һ���� vault ����� entities/fts
        db.rebuild_from_vault(&empty_vault).unwrap();
        assert!(db.get_entity("know-recover").unwrap().is_none());

        // Vault ���ݲ��䣬rebuild �ָ�����
        db.rebuild_from_vault(&vault).unwrap();
        let recovered = db.get_entity("know-recover").unwrap().unwrap();
        assert_eq!(recovered.title.as_deref(), Some("Recover"));
        assert!((recovered.composite.unwrap() - 85.3).abs() < 1e-9);
    }

    /// TC-I02: rebuild �� watcher ��ͬһ�ݱʼǲ����ȼ۵�����ͶӰ��
    /// ʹ���� outgoing refs �ıʼǣ����� watcher ���� Linked/Cited �����á�
    #[tokio::test]
    async fn rebuild_and_watcher_produce_equivalent_index_projections() {
        use std::sync::Arc;
        use tempfile::tempdir;
        use tokio::sync::Mutex;

        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let note = "---\nid: know-shared\ntitle: Shared Parse\nlayer: knowledge\nstatus: active\ntags:\n  - Rust\nscore:\n  interest: 85.0\n  strategy: 90.0\n  consensus: 80.0\n  composite: 85.5\n  updated_at: '2026-07-06T00:00:00Z'\n  last_boosted_at: '2026-07-06T00:00:00Z'\n  access_count: 5\n---\nBody with no refs.\n";
        let path = vault.join("know-shared.md");
        fs::write(&path, note).unwrap();

        // ·�� 1��rebuild_from_vault
        let db_rebuild = Db::open_in_memory().unwrap();
        db_rebuild.rebuild_from_vault(&vault).unwrap();
        let rebuilt = db_rebuild.get_entity("know-shared").unwrap().unwrap();

        // ·�� 2��watcher process_single_file
        let db_watcher: crate::application::ports::RepositoryHandle =
            Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let weights = crate::domain::entity::Weights::default();
        crate::watcher::process_single_file(&vault, &db_watcher, &weights, &path)
            .await
            .unwrap();
        let watched = db_watcher
            .lock()
            .await
            .get_entity("know-shared")
            .unwrap()
            .unwrap();

        assert_eq!(indexed_entity(rebuilt), watched);
        assert_eq!(
            db_rebuild.entity_tags("know-shared").unwrap(),
            db_watcher.lock().await.entity_tags("know-shared").unwrap()
        );
    }
}
