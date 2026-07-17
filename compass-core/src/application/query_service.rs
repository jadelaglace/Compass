//! Read-side query and reporting use cases.
#![allow(clippy::possible_missing_else)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Serialize;

use crate::application::error::AppError;
use crate::application::ports::{IndexedEntity, RepositoryHandle, VaultPort};
use crate::domain::entity::{EffectiveScore, Weights};
use crate::domain::scoring;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct WeeklyReport {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) tz: String,
    pub(crate) generated_at: String,
    pub(crate) data_quality: DataQuality,
    pub(crate) score_changes: Vec<ScoreChange>,
    pub(crate) score_increases: Vec<ScoreChange>,
    pub(crate) score_decreases: Vec<ScoreChange>,
    pub(crate) access_count: u64,
    pub(crate) review_count: u64,
    pub(crate) access_stats: AccessStats,
    pub(crate) new_entities: Vec<String>,
    pub(crate) suggestion_stats: SuggestionStats,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct DataQuality {
    pub(crate) history_unavailable: bool,
    pub(crate) missing: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct ScoreChange {
    pub(crate) entity_id: String,
    pub(crate) delta: f64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct AccessStats {
    pub(crate) total: u64,
    pub(crate) glance: u64,
    pub(crate) read: u64,
    pub(crate) study: u64,
    pub(crate) apply: u64,
    pub(crate) review: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SuggestionStats {
    pub(crate) accepted: u64,
    pub(crate) rejected: u64,
    pub(crate) expired: u64,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EntitySummary {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) composite: Option<f64>,
    pub(crate) base_composite: Option<f64>,
    pub(crate) freshness_factor: Option<f64>,
    pub(crate) strategy: Option<f64>,
    pub(crate) last_boosted_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ScoreResponse {
    pub(crate) interest: f64,
    pub(crate) strategy: f64,
    pub(crate) consensus: f64,
    pub(crate) composite: f64,
    pub(crate) base_composite: f64,
    pub(crate) freshness_factor: f64,
    pub(crate) access_count: i64,
    pub(crate) updated_at: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct EntityDetail {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) file_path: String,
    pub(crate) score: Option<ScoreResponse>,
    pub(crate) refs: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphNode {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) composite: Option<f64>,
    pub(crate) base_composite: Option<f64>,
    pub(crate) freshness_factor: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphEdge {
    pub(crate) source: String,
    pub(crate) target: String,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct GraphData {
    pub(crate) nodes: Vec<GraphNode>,
    pub(crate) edges: Vec<GraphEdge>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SearchHit {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) snippet: Option<String>,
    pub(crate) layer: Option<String>,
    pub(crate) composite: Option<f64>,
    pub(crate) base_composite: Option<f64>,
    pub(crate) freshness_factor: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentContextEntry {
    pub(crate) id: String,
    pub(crate) title: Option<String>,
    pub(crate) content: Option<String>,
    pub(crate) composite: Option<f64>,
    pub(crate) base_composite: Option<f64>,
    pub(crate) freshness_factor: Option<f64>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct AgentContextResponse {
    pub(crate) context: Vec<AgentContextEntry>,
    pub(crate) reasoning: String,
}

pub(crate) struct QueryService {
    repository: RepositoryHandle,
    vault: Arc<dyn VaultPort>,
    _weights: Weights,
    now: DateTime<Utc>,
}

impl QueryService {
    pub(crate) fn new(
        repository: RepositoryHandle,
        vault: Arc<dyn VaultPort>,
        weights: Weights,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            repository,
            vault,
            _weights: weights,
            now,
        }
    }

    pub(crate) async fn weekly_report(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        tz: Option<&str>,
    ) -> Result<WeeklyReport, AppError> {
        let window = parse_report_window(from, to, tz)?;
        let (history, timeline, suggestion_stats, history_unavailable) = {
            let repository = self.repository.lock().await;
            (
                repository.score_history_between(&window.start_utc, &window.end_utc)?,
                repository.timeline_between(&window.start_utc, &window.end_utc)?,
                repository.suggestion_stats_between(&window.start_utc, &window.end_utc)?,
                !repository.has_report_history()?,
            )
        };
        let mut deltas = HashMap::<String, f64>::new();
        for row in history {
            if let (Some(old), Some(new)) = (row.old, row.new) {
                let delta = new - old;
                if delta.is_finite() {
                    *deltas.entry(row.entity_id).or_default() += delta;
                }
            }
        }
        let mut changes = deltas
            .into_iter()
            .filter(|(_, delta)| delta.abs() > f64::EPSILON)
            .map(|(entity_id, delta)| ScoreChange { entity_id, delta })
            .collect::<Vec<_>>();
        changes.sort_by(|left, right| {
            right
                .delta
                .partial_cmp(&left.delta)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.entity_id.cmp(&right.entity_id))
        });
        let increases = changes
            .iter()
            .filter(|change| change.delta > 0.0)
            .take(5)
            .cloned()
            .collect::<Vec<_>>();
        let mut decreases = changes
            .iter()
            .filter(|change| change.delta < 0.0)
            .cloned()
            .collect::<Vec<_>>();
        decreases.sort_by(|left, right| {
            left.delta
                .partial_cmp(&right.delta)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.entity_id.cmp(&right.entity_id))
        });
        decreases.truncate(5);
        let mut score_changes = increases.clone();
        score_changes.extend(decreases.clone());
        let mut stats = AccessStats {
            total: 0,
            glance: 0,
            read: 0,
            study: 0,
            apply: 0,
            review: 0,
        };
        let mut new_entities = Vec::new();
        let mut seen = HashSet::new();
        for event in timeline {
            if event.event_type == "create" {
                if seen.insert(event.entity_id.clone()) {
                    new_entities.push(event.entity_id);
                }
                continue;
            }
            if event.event_type != "access" {
                continue;
            }
            stats.total += 1;
            match event.intensity.unwrap_or_default() {
                value if value <= 0.0 => stats.glance += 1,
                value if value < 3.0 => stats.read += 1,
                value if value < 5.0 => {
                    stats.study += 1;
                    stats.review += 1;
                }
                _ => {
                    stats.apply += 1;
                    stats.review += 1;
                }
            }
        }
        Ok(WeeklyReport {
            from: window.from.format("%Y-%m-%d").to_string(),
            to: window.to.format("%Y-%m-%d").to_string(),
            tz: window.tz.to_string(),
            generated_at: format!("{}T00:00:00Z", window.to.format("%Y-%m-%d")),
            data_quality: DataQuality {
                history_unavailable,
                missing: if history_unavailable {
                    vec!["history".to_string()]
                } else {
                    Vec::new()
                },
            },
            score_changes,
            score_increases: increases,
            score_decreases: decreases,
            access_count: stats.total,
            review_count: stats.review,
            access_stats: stats,
            new_entities,
            suggestion_stats: SuggestionStats {
                accepted: suggestion_stats.accepted,
                rejected: suggestion_stats.rejected,
                expired: suggestion_stats.expired,
            },
        })
    }

    pub(crate) async fn feed(
        &self,
        mode: &str,
        limit: u32,
    ) -> Result<Vec<EntitySummary>, AppError> {
        if !matches!(mode, "explore" | "consolidate" | "strategic") {
            return Err(AppError::unprocessable(
                "mode must be explore, consolidate, or strategic",
            ));
        }
        let entities = self.repository.lock().await.list_entities()?;
        let now = self.now;
        let mut summaries = entities
            .into_iter()
            .map(|entity| self.summary(entity, now))
            .collect::<Result<Vec<_>, _>>()?;
        match mode {
            "strategic" => summaries.sort_by(|a, b| {
                b.strategy
                    .unwrap_or(0.0)
                    .partial_cmp(&a.strategy.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            "consolidate" => {
                summaries.sort_by(|a, b| match (&a.last_boosted_at, &b.last_boosted_at) {
                    (Some(a), Some(b)) => a.cmp(b),
                    (Some(_), None) => std::cmp::Ordering::Greater,
                    (None, Some(_)) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                })
            }
            _ => summaries.sort_by(|a, b| {
                b.composite
                    .unwrap_or(0.0)
                    .partial_cmp(&a.composite.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
        };
        summaries.truncate(limit as usize);
        Ok(summaries)
    }

    pub(crate) async fn top(
        &self,
        layer: Option<&str>,
        limit: u32,
    ) -> Result<Vec<EntitySummary>, AppError> {
        let entities = self.repository.lock().await.list_entities()?;
        let now = self.now;
        let mut summaries = entities
            .into_iter()
            .filter(|entity| layer.is_none_or(|layer| entity.layer.as_deref() == Some(layer)))
            .map(|entity| self.summary(entity, now))
            .collect::<Result<Vec<_>, _>>()?;
        summaries.sort_by(|a, b| {
            b.composite
                .unwrap_or(0.0)
                .partial_cmp(&a.composite.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        summaries.truncate(limit as usize);
        Ok(summaries)
    }

    pub(crate) async fn entity(&self, id: &str) -> Result<EntityDetail, AppError> {
        let entity = self
            .repository
            .lock()
            .await
            .get_entity(id)?
            .ok_or_else(|| AppError::not_found(id))?;
        let effective = self.effective_score(&entity, self.now)?;
        let note = self.vault.load(&entity.file_path).ok();
        let refs = note
            .as_ref()
            .map(|note| self.vault.refs(note))
            .unwrap_or_default();
        let score = entity.composite.map(|_| ScoreResponse {
            interest: entity.interest.unwrap_or(0.0),
            strategy: entity.strategy.unwrap_or(0.0),
            consensus: entity.consensus.unwrap_or(0.0),
            composite: effective.effective_composite,
            base_composite: effective.base_composite,
            freshness_factor: effective.freshness_factor,
            access_count: entity.access_count,
            updated_at: entity.updated_at.clone(),
        });
        Ok(EntityDetail {
            id: entity.id,
            title: entity.title,
            layer: entity.layer,
            status: entity.status,
            file_path: entity.file_path,
            score,
            refs,
        })
    }

    pub(crate) async fn search(&self, query: &str, limit: u32) -> Result<Vec<SearchHit>, AppError> {
        let indexed = {
            let repository = self.repository.lock().await;
            repository
                .search(query, limit)?
                .into_iter()
                .map(|hit| {
                    let entity = repository
                        .get_entity(&hit.id)?
                        .ok_or_else(|| AppError::not_found(&hit.id))?;
                    Ok((hit, entity))
                })
                .collect::<Result<Vec<_>, AppError>>()?
        };
        let now = self.now;
        let mut results = Vec::with_capacity(indexed.len());
        for (hit, entity) in indexed {
            let effective = self.effective_score(&entity, now)?;
            results.push(SearchHit {
                id: hit.id,
                title: hit.title,
                snippet: hit.snippet,
                layer: entity.layer,
                composite: Some(effective.effective_composite),
                base_composite: Some(effective.base_composite),
                freshness_factor: Some(effective.freshness_factor),
            });
        }
        results.sort_by(|left, right| {
            right
                .composite
                .partial_cmp(&left.composite)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(results)
    }

    pub(crate) async fn agent_context(
        &self,
        task: &str,
        top_k: usize,
    ) -> Result<AgentContextResponse, AppError> {
        if task.trim().is_empty() {
            return Err(AppError::unprocessable("task must not be empty"));
        }
        let recall_limit = (top_k * 3).max(10) as u32;
        let indexed = {
            let repository = self.repository.lock().await;
            repository
                .search(task, recall_limit)?
                .into_iter()
                .filter_map(|hit| {
                    repository
                        .get_entity(&hit.id)
                        .transpose()
                        .map(|entity| entity.map(|entity| (hit, entity)))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        let mut entries = Vec::new();
        for (hit, entity) in indexed {
            let effective = self.effective_score(&entity, self.now)?;
            entries.push(AgentContextEntry {
                id: entity.id,
                title: hit.title,
                content: hit.snippet,
                composite: Some(effective.effective_composite),
                base_composite: Some(effective.base_composite),
                freshness_factor: Some(effective.freshness_factor),
            });
        }
        entries.sort_by(|a, b| {
            b.composite
                .unwrap_or(0.0)
                .partial_cmp(&a.composite.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        entries.truncate(top_k);
        Ok(AgentContextResponse {
            reasoning: format!(
                "从 vault 中召回 {} 个相关实体，按 composite 评分加权取前 {} 个作为上下文。",
                entries.len(),
                top_k
            ),
            context: entries,
        })
    }

    pub(crate) async fn graph(&self) -> Result<GraphData, AppError> {
        let entities = self.repository.lock().await.list_entities()?;
        let now = self.now;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for entity in &entities {
            let effective = self.effective_score(entity, now)?;
            nodes.push(GraphNode {
                id: entity.id.clone(),
                title: entity.title.clone(),
                layer: entity.layer.clone(),
                composite: Some(effective.effective_composite),
                base_composite: Some(effective.base_composite),
                freshness_factor: Some(effective.freshness_factor),
            });
            if let Ok(note) = self.vault.load(&entity.file_path) {
                for target in self.vault.refs(&note) {
                    if entities.iter().any(|candidate| candidate.id == target) {
                        edges.push(GraphEdge {
                            source: entity.id.clone(),
                            target,
                        });
                    }
                }
            }
        }
        Ok(GraphData { nodes, edges })
    }

    fn summary(
        &self,
        entity: IndexedEntity,
        now: DateTime<Utc>,
    ) -> Result<EntitySummary, AppError> {
        let effective = self.effective_score(&entity, now)?;
        Ok(EntitySummary {
            id: entity.id,
            title: entity.title,
            layer: entity.layer,
            composite: Some(effective.effective_composite),
            base_composite: Some(effective.base_composite),
            freshness_factor: Some(effective.freshness_factor),
            strategy: entity
                .strategy
                .map(|value| value * effective.freshness_factor),
            last_boosted_at: entity.last_boosted_at,
        })
    }
    fn effective_score(
        &self,
        entity: &IndexedEntity,
        now: DateTime<Utc>,
    ) -> Result<EffectiveScore, AppError> {
        let base_composite = entity.composite.unwrap_or(0.0);
        let Ok(note) = self.vault.load(&entity.file_path) else {
            return Ok(EffectiveScore {
                base_composite,
                freshness_factor: 1.0,
                effective_composite: base_composite,
            });
        };
        let freshness = self
            .vault
            .freshness(&note)
            .map_err(|error| AppError::unprocessable(&format!("invalid freshness: {error}")))?;
        let updated_at = self
            .vault
            .content_updated_at(&note)
            .map_err(|error| {
                AppError::unprocessable(&format!("invalid content_updated_at: {error}"))
            })?
            .or_else(|| entity.updated_at.clone())
            .map(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(|error| {
                        AppError::unprocessable(&format!("invalid content_updated_at: {error}"))
                    })
            })
            .transpose()?;
        scoring::effective_score(base_composite, &freshness, updated_at, now)
            .map_err(|error| AppError::unprocessable(&error.to_string()))
    }
}

struct ReportWindow {
    from: NaiveDate,
    to: NaiveDate,
    tz: Tz,
    start_utc: String,
    end_utc: String,
}
fn parse_report_window(
    from: Option<&str>,
    to: Option<&str>,
    tz: Option<&str>,
) -> Result<ReportWindow, AppError> {
    let from = parse_report_date(from, "from")?;
    let to = parse_report_date(to, "to")?;
    if from >= to {
        return Err(AppError::unprocessable("from must be before to"));
    }
    let name = tz
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::unprocessable("tz is required"))?;
    let tz = name
        .parse::<Tz>()
        .map_err(|_| AppError::unprocessable("tz must be a valid IANA timezone"))?;
    Ok(ReportWindow {
        from,
        to,
        tz,
        start_utc: local_midnight_utc(from, tz, "from")?,
        end_utc: local_midnight_utc(to, tz, "to")?,
    })
}
fn parse_report_date(value: Option<&str>, field: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(
        value.ok_or_else(|| AppError::unprocessable(&format!("{field} is required")))?,
        "%Y-%m-%d",
    )
    .map_err(|_| AppError::unprocessable(&format!("{field} must be YYYY-MM-DD")))
}
fn local_midnight_utc(date: NaiveDate, tz: Tz, field: &str) -> Result<String, AppError> {
    let local = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::unprocessable(&format!("invalid {field} date")))?;
    let datetime = match tz.from_local_datetime(&local) {
        chrono::LocalResult::Single(value) | chrono::LocalResult::Ambiguous(value, _) => value,
        chrono::LocalResult::None => {
            return Err(AppError::unprocessable(&format!(
                "invalid {field} local midnight"
            )))
        }
    };
    Ok(datetime
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true))
}
