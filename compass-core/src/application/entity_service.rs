//! Entity write use cases.

use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use tracing::{debug, info};

use crate::application::error::AppError;
use crate::application::ports::{
    IndexedEntity, RepositoryHandle, ScoreHistoryEntry, TimelineEntry, VaultPort,
};
use crate::domain::entity::Weights;
use crate::domain::scoring::{self, AccessDepth};

#[derive(Debug)]
pub(crate) struct CreateEntity {
    pub(crate) title: String,
    pub(crate) layer: String,
    pub(crate) content: Option<String>,
    pub(crate) interest: Option<f64>,
    pub(crate) strategy: Option<f64>,
    pub(crate) consensus: Option<f64>,
}

#[derive(Debug)]
pub(crate) struct ScoreUpdate {
    pub(crate) interest: Option<f64>,
    pub(crate) strategy: Option<f64>,
    pub(crate) consensus: Option<f64>,
}

pub(crate) struct EntityService {
    repository: RepositoryHandle,
    vault: Arc<dyn VaultPort>,
    weights: Weights,
}

impl EntityService {
    pub(crate) fn new(
        repository: RepositoryHandle,
        vault: Arc<dyn VaultPort>,
        weights: Weights,
    ) -> Self {
        Self {
            repository,
            vault,
            weights,
        }
    }

    pub(crate) async fn create(&self, request: CreateEntity) -> Result<Value, AppError> {
        let prefix = layer_prefix(&request.layer);
        let id = {
            let repository = self.repository.lock().await;
            next_id(&*repository, prefix)?
        };
        let interest = request.interest.unwrap_or(5.0);
        let strategy = request.strategy.unwrap_or(5.0);
        let consensus = request.consensus.unwrap_or(5.0);
        let composite = scoring::composite(interest, strategy, consensus, &self.weights);
        let now = Utc::now().to_rfc3339();
        let content = request.content.unwrap_or_default();
        let file_path = format!("{}/{id}.md", layer_dir(&request.layer));
        let markdown = format!(
            "---\nid: {id}\ntitle: {}\nlayer: {}\nstatus: active\ncreated_at: '{now}'\ncontent_updated_at: '{now}'\nscore:\n  interest: {interest}\n  strategy: {strategy}\n  consensus: {consensus}\n  composite: {composite}\n  weights:\n    interest: {}\n    strategy: {}\n    consensus: {}\n  updated_at: '{now}'\n  last_boosted_at: '{now}'\n  access_count: 0\n---\n{content}\n",
            request.title, request.layer, self.weights.interest, self.weights.strategy, self.weights.consensus
        );
        self.vault
            .create(&file_path, &markdown)
            .map_err(|error| AppError::internal(&format!("write entity failed: {error}")))?;
        let entity = IndexedEntity {
            id: id.clone(),
            file_path: file_path.clone(),
            title: Some(request.title.clone()),
            layer: Some(request.layer),
            status: Some("active".to_string()),
            interest: Some(interest),
            strategy: Some(strategy),
            consensus: Some(consensus),
            composite: Some(composite),
            access_count: 0,
            last_boosted_at: Some(now.clone()),
            content_hash: Some(content_hash(&content)),
            updated_at: Some(now.clone()),
        };
        let repository = self.repository.lock().await;
        repository.upsert_indexed_entity(&entity, &content)?;
        repository.record_timeline(&TimelineEntry {
            entity_id: id.clone(),
            event_type: "create".to_string(),
            intensity: None,
            source: Some("api".to_string()),
            created_at: now,
        })?;
        info!(id = %id, "entity created");
        Ok(
            json!({"id": id, "title": request.title, "file_path": file_path, "composite": composite}),
        )
    }

    pub(crate) async fn update_score(
        &self,
        id: &str,
        request: ScoreUpdate,
    ) -> Result<Value, AppError> {
        let entity = self.entity(id).await?;
        let note = self
            .vault
            .load(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        let mut score = self
            .vault
            .score(&note)
            .map_err(|error| AppError::internal(&format!("parse score failed: {error}")))?
            .ok_or_else(|| AppError::internal("note has no score block"))?;
        let old_composite = score.composite;
        if let Some(value) = request.interest {
            score.interest = value.clamp(0.0, 100.0);
        }
        if let Some(value) = request.strategy {
            score.strategy = value.clamp(0.0, 100.0);
        }
        if let Some(value) = request.consensus {
            score.consensus = value.clamp(0.0, 100.0);
        }
        let weights = score.weights.unwrap_or(self.weights);
        score.composite =
            scoring::composite(score.interest, score.strategy, score.consensus, &weights);
        let now = Utc::now().to_rfc3339();
        score.updated_at = now.clone();
        self.vault
            .write_score(&entity.file_path, &score)
            .map_err(|error| AppError::internal(&format!("write score failed: {error}")))?;
        let updated = indexed_with_score(id, entity, &score);
        let repository = self.repository.lock().await;
        let _ = repository.record_score_history(&ScoreHistoryEntry {
            entity_id: id.to_string(),
            dimension: Some("manual".to_string()),
            old: Some(old_composite),
            new: Some(score.composite),
            reason: Some("manual_adjust".to_string()),
            trigger: Some("ManualMark".to_string()),
            created_at: now,
        });
        repository.upsert_indexed_entity(&updated, &note.body)?;
        debug!(
            id,
            old = old_composite,
            new = score.composite,
            "score updated"
        );
        Ok(score_response(id, &score))
    }

    pub(crate) async fn record_access(&self, id: &str, depth: &str) -> Result<Value, AppError> {
        let depth = parse_access_depth(depth)
            .ok_or_else(|| AppError::bad_request(&format!("unknown access depth: {depth}")))?;
        let entity = self.entity(id).await?;
        let note = self
            .vault
            .load(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        let score = self
            .vault
            .score(&note)
            .map_err(|error| AppError::internal(&format!("parse score failed: {error}")))?
            .ok_or_else(|| AppError::internal("note has no score block"))?;
        let now = Utc::now().to_rfc3339();
        let score = scoring::apply_access(&score, depth, &now);
        self.vault
            .write_score(&entity.file_path, &score)
            .map_err(|error| AppError::internal(&format!("write score failed: {error}")))?;
        let updated = indexed_with_score(id, entity, &score);
        let repository = self.repository.lock().await;
        let _ = repository.record_timeline(&TimelineEntry {
            entity_id: id.to_string(),
            event_type: "access".to_string(),
            intensity: Some(match depth {
                AccessDepth::Glance => 0.0,
                AccessDepth::Read => 1.0,
                AccessDepth::Study => 3.0,
                AccessDepth::Apply => 5.0,
            }),
            source: Some("api".to_string()),
            created_at: now,
        });
        repository.upsert_indexed_entity(&updated, &note.body)?;
        debug!(id, ?depth, composite = score.composite, "access recorded");
        Ok(score_response(id, &score))
    }

    async fn entity(&self, id: &str) -> Result<IndexedEntity, AppError> {
        self.repository
            .lock()
            .await
            .get_entity(id)?
            .ok_or_else(|| AppError::not_found(id))
    }
}

fn indexed_with_score(
    id: &str,
    entity: IndexedEntity,
    score: &crate::domain::entity::Score,
) -> IndexedEntity {
    IndexedEntity {
        id: id.to_string(),
        file_path: entity.file_path,
        title: entity.title,
        layer: entity.layer,
        status: entity.status,
        interest: Some(score.interest),
        strategy: Some(score.strategy),
        consensus: Some(score.consensus),
        composite: Some(score.composite),
        access_count: score.access_count,
        last_boosted_at: Some(score.last_boosted_at.clone()),
        content_hash: entity.content_hash,
        updated_at: Some(score.updated_at.clone()),
    }
}

fn score_response(id: &str, score: &crate::domain::entity::Score) -> Value {
    json!({"id": id, "score": {"interest": score.interest, "strategy": score.strategy,
        "consensus": score.consensus, "composite": score.composite, "access_count": score.access_count,
        "updated_at": score.updated_at}})
}

fn layer_prefix(layer: &str) -> &str {
    match layer.to_lowercase().as_str() {
        "direction" => "dir",
        "knowledge" => "know",
        "case" => "case",
        "log" => "log",
        "insight" => "ins",
        _ => "know",
    }
}

fn layer_dir(layer: &str) -> &'static str {
    match layer.to_lowercase().as_str() {
        "direction" => "Direction",
        "knowledge" => "Knowledge",
        "case" => "Cases",
        "log" => "Logs",
        "insight" => "Insights",
        _ => "Inbox",
    }
}

fn next_id(
    repository: &dyn crate::application::ports::RepositoryPort,
    prefix: &str,
) -> anyhow::Result<String> {
    let max = repository
        .list_entities()?
        .iter()
        .filter_map(|entity| entity.id.strip_prefix(prefix)?.parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    Ok(format!("{prefix}{:06}", max + 1))
}

fn parse_access_depth(depth: &str) -> Option<AccessDepth> {
    match depth.to_lowercase().as_str() {
        "glance" => Some(AccessDepth::Glance),
        "read" => Some(AccessDepth::Read),
        "study" => Some(AccessDepth::Study),
        "apply" => Some(AccessDepth::Apply),
        _ => None,
    }
}

fn content_hash(value: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
