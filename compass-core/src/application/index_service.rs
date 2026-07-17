//! Shared Vault-to-index orchestration for startup rebuilds and file changes.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use crate::application::ports::{
    IndexProjection, RebuildStats, RepositoryHandle, ScoreHistoryEntry, TimelineEntry,
    VaultIndexEntry, VaultPort,
};
use crate::domain::entity::{Score, Weights};
use crate::domain::scoring;
use crate::domain::vault::VaultScore;

const DEFAULT_SCORE: f64 = 5.0;

/// Owns the common parse-to-projection path. The watcher is an event adapter;
/// it never performs indexing or SQLite work itself.
pub(crate) struct IndexService {
    vault: Arc<dyn VaultPort>,
    repository: RepositoryHandle,
    weights: Weights,
}

impl IndexService {
    pub(crate) fn new(
        vault: Arc<dyn VaultPort>,
        repository: RepositoryHandle,
        weights: Weights,
    ) -> Self {
        Self {
            vault,
            repository,
            weights,
        }
    }

    pub(crate) async fn rebuild(&self) -> Result<RebuildStats> {
        // Scanning and projection happen before acquiring the SQLite mutex.
        let scan = self.vault.scan()?;
        let mut stats = RebuildStats {
            skipped: scan.skipped,
            ..RebuildStats::default()
        };
        let mut ids = HashSet::new();
        let mut projections = Vec::new();
        for entry in scan.entries {
            if !ids.insert(entry.id.clone()) {
                tracing::warn!(id = %entry.id, path = %entry.file_path, "duplicate id skipped during rebuild");
                stats.duplicates += 1;
                continue;
            }
            projections.push(self.project(entry));
        }
        stats.indexed = projections.len() as u32;
        self.repository
            .lock()
            .await
            .replace_index_projections(&projections)?;
        Ok(stats)
    }

    /// Reconcile one changed Markdown path. An absent or ineligible note removes
    /// the stale projection at that path.
    pub(crate) async fn process_changed_path(&self, file_path: &str) -> Result<()> {
        let Some(mut entry) = self.vault.index_entry(file_path)? else {
            self.repository
                .lock()
                .await
                .delete_entities_under_path(file_path)?;
            return Ok(());
        };

        if entry.score.is_none() {
            let score = default_score(self.weights);
            self.vault.write_score(&entry.file_path, &score)?;
            entry.score = Some(score);
        }

        let projection = self.project(entry);
        let source_score = projection.score.clone();
        let existed = {
            let repository = self.repository.lock().await;
            let existed = repository.entity_exists(&projection.id)?;
            repository.upsert_index_projection(&projection)?;
            if !existed {
                repository.record_timeline(&TimelineEntry {
                    entity_id: projection.id.clone(),
                    event_type: "create".to_string(),
                    intensity: None,
                    source: Some("watcher".to_string()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                })?;
            }
            existed
        };
        let _ = existed;

        if let Some(score) = source_score {
            self.fire_link_and_cited_triggers(&projection, &score)
                .await?;
        }
        Ok(())
    }

    fn project(&self, mut entry: VaultIndexEntry) -> IndexProjection {
        if let Some(score) = entry.score.as_mut() {
            score.composite = scoring::composite(
                score.interest,
                score.strategy,
                score.consensus,
                &self.weights,
            );
        }
        let fallback_title = entry
            .file_path
            .rsplit('/')
            .next()
            .and_then(|name| name.strip_suffix(".md"))
            .unwrap_or("unknown")
            .to_string();
        IndexProjection {
            id: entry.id,
            file_path: entry.file_path,
            title: entry.title.or(Some(fallback_title)),
            layer: entry.layer,
            status: entry.status.or(Some("active".to_string())),
            score: entry.score,
            content_hash: entry.content_hash,
            body: entry.body,
            tags: entry.tags,
            links: entry.links,
        }
    }

    async fn fire_link_and_cited_triggers(
        &self,
        source: &IndexProjection,
        source_score: &VaultScore,
    ) -> Result<()> {
        if source.links.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();

        let last_linked = self
            .repository
            .lock()
            .await
            .last_trigger_time(&source.id, "Linked")?;
        if let Some(updated) = scoring::apply_trigger_if_eligible(
            source_score,
            scoring::Trigger::Linked,
            &now,
            last_linked.as_deref(),
        )? {
            self.vault.write_score(&source.file_path, &updated)?;
            let repository = self.repository.lock().await;
            repository.update_index_score(&source.id, &updated)?;
            repository.record_score_history(&score_history(
                &source.id,
                "Linked",
                "interest",
                source_score.interest,
                updated.interest,
                &now,
            ))?;
        }

        for target_id in &source.links {
            let (Some(file_path), last_cited) = ({
                let repository = self.repository.lock().await;
                (
                    repository.entity_file_path(target_id)?,
                    repository.last_trigger_time(target_id, "Cited")?,
                )
            }) else {
                continue;
            };
            let note = self.vault.load(&file_path)?;
            let Some(score) = self.vault.score(&note)? else {
                continue;
            };
            let Some(updated) = scoring::apply_trigger_if_eligible(
                &score,
                scoring::Trigger::Cited,
                &now,
                last_cited.as_deref(),
            )?
            else {
                continue;
            };
            self.vault.write_score(&file_path, &updated)?;
            let repository = self.repository.lock().await;
            repository.update_index_score(target_id, &updated)?;
            repository.record_score_history(&score_history(
                target_id,
                "Cited",
                "consensus",
                score.consensus,
                updated.consensus,
                &now,
            ))?;
        }
        Ok(())
    }
}

fn default_score(weights: Weights) -> Score {
    let now = chrono::Utc::now().to_rfc3339();
    Score {
        interest: DEFAULT_SCORE,
        strategy: DEFAULT_SCORE,
        consensus: DEFAULT_SCORE,
        composite: scoring::composite(DEFAULT_SCORE, DEFAULT_SCORE, DEFAULT_SCORE, &weights),
        weights: Some(weights),
        updated_at: now.clone(),
        last_boosted_at: now,
        access_count: 0,
    }
}

fn score_history(
    entity_id: &str,
    trigger: &str,
    dimension: &str,
    old: f64,
    new: f64,
    now: &str,
) -> ScoreHistoryEntry {
    ScoreHistoryEntry {
        entity_id: entity_id.to_string(),
        dimension: Some(dimension.to_string()),
        old: Some(old),
        new: Some(new),
        reason: Some(format!("trigger:{trigger}")),
        trigger: Some(trigger.to_string()),
        created_at: now.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::{IndexedEntity, RepositoryHandle, RepositoryPort};
    use crate::infrastructure::sqlite_repository::SqliteRepository;
    use crate::infrastructure::vault_adapter::VaultAdapter;
    use std::fs;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    fn service(vault: std::path::PathBuf, repository: RepositoryHandle) -> IndexService {
        IndexService::new(
            Arc::new(VaultAdapter::new(vault)),
            repository,
            Weights::default(),
        )
    }

    #[tokio::test]
    async fn changed_file_writes_default_score_and_replaces_relationships() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let path = vault.join("note.md");
        fs::write(
            &path,
            "---\nid: know-one\ntitle: One\ntags:\n  - Rust\n---\nLinks [[know-two]].\n",
        )
        .unwrap();
        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let indexer = service(vault.clone(), db.clone());

        indexer.process_changed_path("note.md").await.unwrap();
        let first = db.lock().await.get_entity("know-one").unwrap().unwrap();
        // The default score is written first, then the Linked trigger applies.
        assert_eq!(first.composite, Some(5.4));
        assert_eq!(db.lock().await.entity_tags("know-one").unwrap(), ["Rust"]);
        assert_eq!(
            db.lock().await.entity_links("know-one").unwrap(),
            ["know-two"]
        );
        assert!(fs::read_to_string(&path).unwrap().contains("composite:"));

        fs::write(
            &path,
            "---\nid: know-three\ntitle: Three\ntags:\n  - SQLite\nscore:\n  interest: 20.0\n  strategy: 30.0\n  consensus: 40.0\n  composite: 0.0\n  updated_at: '2026-07-06T00:00:00Z'\n  last_boosted_at: '2026-07-06T00:00:00Z'\n  access_count: 1\n---\nLinks [[know-four]].\n",
        )
        .unwrap();
        indexer.process_changed_path("note.md").await.unwrap();
        assert!(db.lock().await.get_entity("know-one").unwrap().is_none());
        assert_eq!(
            db.lock().await.entity_tags("know-three").unwrap(),
            ["SQLite"]
        );
        assert_eq!(
            db.lock().await.entity_links("know-three").unwrap(),
            ["know-four"]
        );
    }

    #[tokio::test]
    async fn changed_file_cleans_template_unrendered_and_deleted_projections() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        fs::create_dir_all(vault.join("Knowledge/Templates")).unwrap();
        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let indexer = service(vault.clone(), db.clone());
        for (id, file_path) in [
            ("template", "Knowledge/Templates/template.md"),
            ("unrendered", "unrendered.md"),
            ("deleted", "deleted.md"),
        ] {
            db.lock()
                .await
                .upsert_indexed_entity(
                    &IndexedEntity {
                        id: id.to_string(),
                        file_path: file_path.to_string(),
                        title: None,
                        layer: None,
                        status: None,
                        interest: None,
                        strategy: None,
                        consensus: None,
                        composite: None,
                        access_count: 0,
                        last_boosted_at: None,
                        content_hash: None,
                        updated_at: None,
                    },
                    "stale",
                )
                .unwrap();
        }
        fs::write(
            vault.join("Knowledge/Templates/template.md"),
            "---\nid: template\n---\ntemplate\n",
        )
        .unwrap();
        fs::write(
            vault.join("unrendered.md"),
            "---\nid: '<% tp.file.title %>'\n---\ntemplate\n",
        )
        .unwrap();

        for file_path in [
            "Knowledge/Templates/template.md",
            "unrendered.md",
            "deleted.md",
        ] {
            indexer.process_changed_path(file_path).await.unwrap();
        }
        for id in ["template", "unrendered", "deleted"] {
            assert!(db.lock().await.get_entity(id).unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn rebuild_excludes_sync_conflict_copies_and_keeps_the_primary_note() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        fs::create_dir_all(vault.join("Knowledge")).unwrap();
        let primary = vault.join("Knowledge").join("note.md");
        let conflict = vault
            .join("Knowledge")
            .join("note.sync-conflict-20260712-120000-DEVICE.md");
        let note = "---\nid: know-sync\ntitle: Primary\n---\nPrimary content.\n";
        fs::write(&primary, note).unwrap();
        fs::write(&conflict, note.replace("Primary", "Conflicting")).unwrap();

        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let indexer = service(vault, db.clone());
        let stats = indexer.rebuild().await.unwrap();

        assert_eq!(stats.indexed, 1);
        assert_eq!(stats.duplicates, 0);
        let entity = db.lock().await.get_entity("know-sync").unwrap().unwrap();
        assert_eq!(entity.file_path, "Knowledge/note.md");

        indexer
            .process_changed_path("Knowledge/note.sync-conflict-20260712-120000-DEVICE.md")
            .await
            .unwrap();
        assert!(db.lock().await.get_entity("know-sync").unwrap().is_some());
    }

    #[tokio::test]
    async fn sync_conflict_round_trip_rebuilds_from_the_resolved_primary_note() {
        let directory = tempdir().unwrap();
        let device_a = directory.path().join("device-a");
        let device_b = directory.path().join("device-b");
        for vault in [&device_a, &device_b] {
            fs::create_dir_all(vault.join("Knowledge")).unwrap();
        }

        let source_primary = device_a.join("Knowledge").join("note.md");
        let target_primary = device_b.join("Knowledge").join("note.md");
        let target_conflict = device_b
            .join("Knowledge")
            .join("note.sync-conflict-20260712-120000-DEVICEA.md");
        fs::write(
            &source_primary,
            "---\nid: know-sync-roundtrip\ntitle: Shared\n---\nInitial version.\n",
        )
        .unwrap();
        fs::copy(&source_primary, &target_primary).unwrap();

        // Both devices edit offline. The transport retains A's concurrent edit
        // as a conflict copy on B rather than replacing B's primary file.
        fs::write(
            &source_primary,
            "---\nid: know-sync-roundtrip\ntitle: Shared\n---\nDeviceA version.\n",
        )
        .unwrap();
        fs::write(
            &target_primary,
            "---\nid: know-sync-roundtrip\ntitle: Shared\n---\nDeviceB version.\n",
        )
        .unwrap();
        fs::copy(&source_primary, &target_conflict).unwrap();

        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let indexer = service(device_b.clone(), db.clone());
        let stats = indexer.rebuild().await.unwrap();
        assert_eq!(stats.indexed, 1);
        assert_eq!(stats.duplicates, 0);
        assert!(
            target_conflict.exists(),
            "conflict evidence must be retained"
        );
        assert_eq!(
            db.lock().await.search("DeviceB", 10).unwrap().len(),
            1,
            "only B's primary version should be indexed before resolution"
        );
        assert!(db.lock().await.search("DeviceA", 10).unwrap().is_empty());

        fs::write(
            &target_primary,
            "---\nid: know-sync-roundtrip\ntitle: Shared\n---\nMerged resolution.\n",
        )
        .unwrap();
        fs::remove_file(&target_conflict).unwrap();
        let resolved = indexer.rebuild().await.unwrap();
        assert_eq!(resolved.indexed, 1);
        assert_eq!(resolved.duplicates, 0);
        assert_eq!(
            db.lock().await.search("Merged", 10).unwrap().len(),
            1,
            "rebuild should replace the target projection after manual resolution"
        );
        assert!(db.lock().await.search("DeviceB", 10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn changed_file_fires_linked_and_cited_triggers_once() {
        let directory = tempdir().unwrap();
        let vault = directory.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let score = "score:\n  interest: 50.0\n  strategy: 50.0\n  consensus: 50.0\n  composite: 50.0\n  updated_at: '2026-07-01T00:00:00Z'\n  last_boosted_at: '2026-07-01T00:00:00Z'\n  access_count: 0";
        fs::write(
            vault.join("b.md"),
            format!("---\nid: know-b\ntitle: B\n{score}\n---\nBody.\n"),
        )
        .unwrap();
        fs::write(
            vault.join("a.md"),
            format!("---\nid: know-a\ntitle: A\n{score}\n---\nCites [[know-b]].\n"),
        )
        .unwrap();
        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let indexer = service(vault.clone(), db.clone());

        indexer.process_changed_path("b.md").await.unwrap();
        indexer.process_changed_path("a.md").await.unwrap();
        assert_eq!(
            db.lock()
                .await
                .get_entity("know-a")
                .unwrap()
                .unwrap()
                .interest,
            Some(51.0)
        );
        assert_eq!(
            db.lock()
                .await
                .get_entity("know-b")
                .unwrap()
                .unwrap()
                .consensus,
            Some(52.0)
        );
        indexer.process_changed_path("a.md").await.unwrap();
        assert_eq!(
            db.lock()
                .await
                .get_entity("know-a")
                .unwrap()
                .unwrap()
                .interest,
            Some(51.0)
        );
        assert_eq!(
            db.lock()
                .await
                .get_entity("know-b")
                .unwrap()
                .unwrap()
                .consensus,
            Some(52.0)
        );
    }
}
