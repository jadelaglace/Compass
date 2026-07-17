//! Application-facing capabilities supplied by infrastructure adapters.

use std::sync::Arc;

use anyhow::Result;
use tokio::sync::Mutex;

use crate::domain::entity::{Freshness, Score};
use crate::domain::vault::{MetadataPatch, MetadataPatchResult, VaultNote, VaultScore};

/// Storage-neutral entity snapshot. This is the only entity-shaped value that
/// application and transport code may receive from an index repository.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IndexedEntity {
    pub(crate) id: String,
    pub(crate) file_path: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) interest: Option<f64>,
    pub(crate) strategy: Option<f64>,
    pub(crate) consensus: Option<f64>,
    pub(crate) composite: Option<f64>,
    pub(crate) access_count: i64,
    pub(crate) last_boosted_at: Option<String>,
    pub(crate) content_hash: Option<String>,
    pub(crate) updated_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct ScoreHistoryEntry {
    pub(crate) entity_id: String,
    pub(crate) dimension: Option<String>,
    pub(crate) old: Option<f64>,
    pub(crate) new: Option<f64>,
    pub(crate) reason: Option<String>,
    pub(crate) trigger: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineEntry {
    pub(crate) entity_id: String,
    pub(crate) event_type: String,
    pub(crate) intensity: Option<f64>,
    pub(crate) source: Option<String>,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IndexSearchHit {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CachedSuggestion {
    pub(crate) suggestion_id: String,
    pub(crate) kind: String,
    pub(crate) entity_id: String,
    pub(crate) candidate: String,
    pub(crate) candidate_key: String,
    pub(crate) confidence: Option<f64>,
    pub(crate) reason: String,
    pub(crate) source: String,
    pub(crate) algorithm_version: String,
    pub(crate) content_hash: String,
    pub(crate) status: String,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SuggestionStats {
    pub(crate) accepted: u64,
    pub(crate) rejected: u64,
    pub(crate) expired: u64,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct RebuildStats {
    pub(crate) indexed: u32,
    pub(crate) skipped: u32,
    pub(crate) duplicates: u32,
}

/// A parsed Vault note ready to be projected into the rebuildable SQLite index.
#[derive(Debug, Clone)]
pub(crate) struct VaultIndexEntry {
    pub(crate) id: String,
    pub(crate) file_path: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) score: Option<VaultScore>,
    pub(crate) content_hash: Option<String>,
    pub(crate) body: String,
    pub(crate) tags: Vec<String>,
    pub(crate) links: Vec<String>,
}

#[derive(Debug, Default)]
pub(crate) struct VaultScan {
    pub(crate) entries: Vec<VaultIndexEntry>,
    pub(crate) skipped: u32,
}

/// A storage-neutral projection of a Vault note used to maintain the
/// rebuildable index. SQL row mappings remain inside the database adapter.
#[derive(Debug, Clone)]
pub(crate) struct IndexProjection {
    pub(crate) id: String,
    pub(crate) file_path: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) score: Option<VaultScore>,
    pub(crate) content_hash: Option<String>,
    pub(crate) body: String,
    pub(crate) tags: Vec<String>,
    pub(crate) links: Vec<String>,
}

/// Authoritative Markdown and frontmatter operations.
#[allow(dead_code)]
pub(crate) trait VaultPort: Send + Sync {
    fn load(&self, file_path: &str) -> Result<VaultNote>;
    fn read_raw(&self, file_path: &str) -> Result<String>;
    fn score(&self, note: &VaultNote) -> Result<Option<Score>>;
    fn freshness(&self, note: &VaultNote) -> Result<Freshness>;
    fn content_updated_at(&self, note: &VaultNote) -> Result<Option<String>>;
    fn tags(&self, note: &VaultNote) -> Vec<String>;
    fn refs(&self, note: &VaultNote) -> Vec<String>;
    fn write_score(&self, file_path: &str, score: &VaultScore) -> Result<()>;
    fn patch_metadata(
        &self,
        file_path: &str,
        expected_hash: &str,
        patches: &[MetadataPatch],
    ) -> Result<MetadataPatchResult>;
    fn create(&self, file_path: &str, content: &str) -> Result<()>;
    fn index_entry(&self, file_path: &str) -> Result<Option<VaultIndexEntry>>;
    fn scan(&self) -> Result<VaultScan>;
}

/// Rebuildable index and history operations. Implementations keep SQL, FTS
/// syntax, transactions, and SQLite row mappings private.
#[allow(dead_code)]
pub(crate) trait RepositoryPort: Send {
    fn schema_version(&self) -> Result<i64>;
    fn replace_index_projections(&self, projections: &[IndexProjection]) -> Result<()>;
    fn upsert_index_projection(&self, projection: &IndexProjection) -> Result<()>;
    fn upsert_indexed_entity(&self, entity: &IndexedEntity, body: &str) -> Result<()>;
    fn upsert_indexed_entity_with_relationships(
        &self,
        entity: &IndexedEntity,
        body: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()>;
    fn entity_exists(&self, id: &str) -> Result<bool>;
    fn entity_file_path(&self, id: &str) -> Result<Option<String>>;
    fn get_entity(&self, id: &str) -> Result<Option<IndexedEntity>>;
    fn list_entities(&self) -> Result<Vec<IndexedEntity>>;
    fn delete_entities_under_path(&self, path: &str) -> Result<()>;
    fn replace_entity_relationships(
        &self,
        entity_id: &str,
        tags: &[String],
        links: &[String],
    ) -> Result<()>;
    fn directly_linked_entities(&self, entity_id: &str) -> Result<Vec<String>>;
    fn entity_tags(&self, entity_id: &str) -> Result<Vec<String>>;
    fn entity_links(&self, entity_id: &str) -> Result<Vec<String>>;
    fn search(&self, query: &str, limit: u32) -> Result<Vec<IndexSearchHit>>;
    fn upsert_suggestion(&self, suggestion: &CachedSuggestion) -> Result<()>;
    fn get_suggestion(&self, suggestion_id: &str) -> Result<Option<CachedSuggestion>>;
    fn update_suggestion_status(
        &self,
        suggestion_id: &str,
        status: &str,
        updated_at: &str,
    ) -> Result<bool>;
    fn last_trigger_time(&self, entity_id: &str, trigger: &str) -> Result<Option<String>>;
    fn update_index_score(&self, id: &str, score: &Score) -> Result<()>;
    fn record_score_history(&self, entry: &ScoreHistoryEntry) -> Result<()>;
    fn record_timeline(&self, entry: &TimelineEntry) -> Result<()>;
    fn score_history_between(&self, from: &str, to: &str) -> Result<Vec<ScoreHistoryEntry>>;
    fn timeline_between(&self, from: &str, to: &str) -> Result<Vec<TimelineEntry>>;
    fn suggestion_stats_between(&self, from: &str, to: &str) -> Result<SuggestionStats>;
    fn has_report_history(&self) -> Result<bool>;
}

/// A mutex protects only one synchronous repository call at a time. Callers
/// must drop the guard before any file operation, ordering work, or await.
pub(crate) type RepositoryHandle = Arc<Mutex<dyn RepositoryPort>>;
