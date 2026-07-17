//! Tag and relationship suggestion use cases.
#![allow(clippy::possible_missing_else)]

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};

use crate::application::error::AppError;
use crate::application::ports::{CachedSuggestion, IndexedEntity, RepositoryHandle, VaultPort};
use crate::domain::contracts::{
    candidate_key, normalize_tag, note_content_hash, stable_suggestion_id, RelatedSignals,
    RelatedSuggestion, SuggestionKind, SuggestionStatus, TagSuggestion,
    MAX_SUGGESTIONS_PER_REQUEST, RELATED_ALGORITHM_VERSION, TAG_ALGORITHM_VERSION,
};
use crate::domain::vault::{MetadataPatch, MetadataPatchError};

#[derive(Debug, Clone)]
pub(crate) struct TagCandidate {
    pub(crate) tag: String,
    pub(crate) confidence: f64,
    pub(crate) reason: String,
    pub(crate) source: String,
    pub(crate) algorithm_version: String,
    pub(crate) content_hash: String,
}

pub(crate) struct SuggestionService {
    repository: RepositoryHandle,
    vault: Arc<dyn VaultPort>,
}

impl SuggestionService {
    pub(crate) fn new(
        repository: RepositoryHandle,
        vault: Arc<dyn VaultPort>,
        _weights: crate::domain::entity::Weights,
    ) -> Self {
        Self { repository, vault }
    }

    pub(crate) async fn tag_suggestions(
        &self,
        id: &str,
        requested: Vec<TagCandidate>,
    ) -> Result<Value, AppError> {
        if requested.len() > MAX_SUGGESTIONS_PER_REQUEST {
            return Err(AppError::unprocessable(
                "candidates must contain at most 20 items",
            ));
        }
        let entity = self.entity(id).await?;
        let raw = self
            .vault
            .read_raw(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        let note = self
            .vault
            .load(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("parse note failed: {error}")))?;
        let hash = note_content_hash(&raw)?;
        let existing = self.vault.tags(&note);
        let candidates = if requested.is_empty() {
            lexical_tag_candidates(&note.frontmatter, &note.body, &existing, &hash)
        } else {
            requested
                .into_iter()
                .map(|candidate| {
                    if candidate.content_hash != hash {
                        return Err(AppError::conflict(
                            "suggestion_expired",
                            "agent candidate content hash is stale",
                        ));
                    }
                    if candidate.source.trim().is_empty()
                        || candidate.algorithm_version.trim().is_empty()
                    {
                        return Err(AppError::bad_request(
                            "agent candidate source and algorithm_version are required",
                        ));
                    }
                    Ok((
                        normalize_tag(&candidate.tag)
                            .map_err(|error| AppError::bad_request(&error.to_string()))?,
                        candidate.confidence,
                        candidate.reason,
                        candidate.source,
                        candidate.algorithm_version,
                        candidate.content_hash,
                    ))
                })
                .collect::<Result<Vec<_>, AppError>>()?
        };
        let now = Utc::now().to_rfc3339();
        let mut suggestions = Vec::new();
        for (tag, confidence, reason, source, algorithm_version, candidate_hash) in candidates {
            if existing
                .iter()
                .any(|value| value.eq_ignore_ascii_case(&tag))
            {
                continue;
            }
            if !(0.0..=1.0).contains(&confidence) {
                return Err(AppError::bad_request("confidence must be between 0 and 1"));
            }
            let suggestion = CachedSuggestion {
                suggestion_id: stable_suggestion_id(
                    SuggestionKind::Tag,
                    id,
                    &tag,
                    &candidate_hash,
                    &algorithm_version,
                    &source,
                ),
                kind: "tag".to_string(),
                entity_id: id.to_string(),
                candidate: tag.clone(),
                candidate_key: candidate_key(SuggestionKind::Tag, &tag),
                confidence: Some(confidence),
                reason,
                source,
                algorithm_version,
                content_hash: candidate_hash,
                status: "pending".to_string(),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            let row = {
                let repository = self.repository.lock().await;
                repository.upsert_suggestion(&suggestion)?;
                repository.get_suggestion(&suggestion.suggestion_id)?
            };
            if let Some(row) = row {
                suggestions.push(tag_suggestion_json(&row)?);
            }
        }
        Ok(json!({"entity_id": id, "content_hash": hash, "suggestions": suggestions}))
    }

    pub(crate) async fn accept_tag(&self, suggestion_id: &str) -> Result<Value, AppError> {
        self.accept(suggestion_id, "tag", MetadataPatch::AddTag, |row, _| {
            tag_suggestion_json(row)
        })
        .await
    }
    pub(crate) async fn reject_tag(&self, suggestion_id: &str) -> Result<Value, AppError> {
        self.reject(suggestion_id, "tag", |row, _| tag_suggestion_json(row))
            .await
    }

    pub(crate) async fn related(&self, id: &str, limit: u32) -> Result<Value, AppError> {
        let (source, source_neighbors, candidates) = {
            let repository = self.repository.lock().await;
            let source = repository
                .get_entity(id)?
                .ok_or_else(|| AppError::not_found(id))?;
            let source_neighbors = repository
                .directly_linked_entities(id)?
                .into_iter()
                .collect::<HashSet<_>>();
            let candidates = repository
                .list_entities()?
                .into_iter()
                .map(|candidate| {
                    Ok((
                        candidate.clone(),
                        repository
                            .directly_linked_entities(&candidate.id)?
                            .into_iter()
                            .collect::<HashSet<_>>(),
                    ))
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            (source, source_neighbors, candidates)
        };
        let raw = self
            .vault
            .read_raw(&source.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        let source_note = self
            .vault
            .load(&source.file_path)
            .map_err(|error| AppError::internal(&format!("parse note failed: {error}")))?;
        let source_hash = note_content_hash(&raw)?;
        let source_terms = lexical_term_set(&source_note.frontmatter, &source_note.body);
        let source_tags = self
            .vault
            .tags(&source_note)
            .into_iter()
            .map(|tag| tag.to_lowercase())
            .collect::<HashSet<_>>();
        let now = Utc::now();
        let timestamp = now.to_rfc3339();
        let mut ranked = Vec::new();
        for (candidate, candidate_neighbors) in candidates {
            if candidate.id == id
                || candidate
                    .status
                    .as_deref()
                    .is_some_and(|status| status.eq_ignore_ascii_case("archived"))
                || source_neighbors.contains(&candidate.id)
            {
                continue;
            }
            let Ok(note) = self.vault.load(&candidate.file_path) else {
                continue;
            };
            let candidate_terms = lexical_term_set(&note.frontmatter, &note.body);
            let candidate_tags = self
                .vault
                .tags(&note)
                .into_iter()
                .map(|tag| tag.to_lowercase())
                .collect::<HashSet<_>>();
            let term_overlap = source_terms.intersection(&candidate_terms).count();
            let tag_overlap = source_tags.intersection(&candidate_tags).count();
            let shared_neighbors = source_neighbors.intersection(&candidate_neighbors).count();
            let term_signal = (term_overlap as f64 / 5.0).min(1.0) * 0.45;
            let tag_signal = (tag_overlap as f64 / 3.0).min(1.0) * 0.30;
            let graph_signal = (shared_neighbors as f64 / 2.0).min(1.0) * 0.15;
            let effective = self.effective(&candidate, now)?;
            let composite_signal = (effective.effective_composite.clamp(0.0, 100.0) / 100.0) * 0.10;
            let score = term_signal + tag_signal + graph_signal + composite_signal;
            if score <= 0.0 {
                continue;
            }
            let mut reasons = Vec::new();
            if term_overlap > 0 {
                reasons.push(format!("shared terms: {term_overlap}"));
            }
            if tag_overlap > 0 {
                reasons.push(format!("shared tags: {tag_overlap}"));
            }
            if shared_neighbors > 0 {
                reasons.push(format!("shared graph neighbors: {shared_neighbors}"));
            }
            reasons.push(format!("composite signal: {:.3}", composite_signal));
            ranked.push((
                candidate,
                score,
                reasons,
                RelatedSignals {
                    term_overlap,
                    tag_overlap,
                    shared_neighbors,
                    term_signal,
                    tag_signal,
                    graph_signal,
                    composite_signal,
                },
                effective,
            ));
        }
        ranked.sort_by(|(left, left_score, ..), (right, right_score, ..)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.id.cmp(&right.id))
        });
        ranked.truncate(limit.min(20) as usize);
        let mut suggestions = Vec::new();
        for (candidate, score, reasons, signals, effective) in ranked {
            let reason = serde_json::to_string(&json!({"reasons": reasons, "signals": signals}))
                .map_err(|error| {
                    AppError::internal(&format!("serialize reasons failed: {error}"))
                })?;
            let suggestion = CachedSuggestion {
                suggestion_id: stable_suggestion_id(
                    SuggestionKind::Related,
                    id,
                    &candidate.id,
                    &source_hash,
                    RELATED_ALGORITHM_VERSION,
                    "rust_lexical",
                ),
                kind: "related".to_string(),
                entity_id: id.to_string(),
                candidate: candidate.id.clone(),
                candidate_key: candidate_key(SuggestionKind::Related, &candidate.id),
                confidence: Some(score),
                reason,
                source: "rust_lexical".to_string(),
                algorithm_version: RELATED_ALGORITHM_VERSION.to_string(),
                content_hash: source_hash.clone(),
                status: "pending".to_string(),
                created_at: timestamp.clone(),
                updated_at: timestamp.clone(),
            };
            let row = {
                let repository = self.repository.lock().await;
                repository.upsert_suggestion(&suggestion)?;
                repository.get_suggestion(&suggestion.suggestion_id)?
            };
            if let Some(row) = row {
                let mut response = related_suggestion_json(&row, &candidate, score)?;
                response["composite"] = json!(effective.effective_composite);
                response["base_composite"] = json!(effective.base_composite);
                response["freshness_factor"] = json!(effective.freshness_factor);
                suggestions.push(response);
            }
        }
        Ok(json!({"entity_id": id, "content_hash": source_hash, "suggestions": suggestions}))
    }

    pub(crate) async fn accept_related(&self, suggestion_id: &str) -> Result<Value, AppError> {
        self.accept(
            suggestion_id,
            "related",
            MetadataPatch::AddLink,
            related_suggestion_json_from_row,
        )
        .await
    }
    pub(crate) async fn reject_related(&self, suggestion_id: &str) -> Result<Value, AppError> {
        self.reject(suggestion_id, "related", related_suggestion_json_from_row)
            .await
    }

    async fn accept<F>(
        &self,
        suggestion_id: &str,
        kind: &str,
        patch: fn(String) -> MetadataPatch,
        render: F,
    ) -> Result<Value, AppError>
    where
        F: Fn(&CachedSuggestion, Option<&IndexedEntity>) -> Result<Value, AppError>,
    {
        let (suggestion, entity, candidate) = {
            let repository = self.repository.lock().await;
            let suggestion = repository
                .get_suggestion(suggestion_id)?
                .ok_or_else(|| AppError::not_found(suggestion_id))?;
            let entity = repository.get_entity(&suggestion.entity_id)?;
            let candidate = repository.get_entity(&suggestion.candidate)?;
            (suggestion, entity, candidate)
        };
        ensure_kind(&suggestion, kind)?;
        match suggestion.status.as_str() {
            "accepted" | "expired" if kind == "related" => {
                return render(&suggestion, candidate.as_ref())
            }
            "accepted" | "expired" => return render(&suggestion, None),
            "rejected" => {
                return Err(AppError::conflict(
                    "suggestion_rejected",
                    "rejected suggestion cannot be accepted",
                ))
            }
            "pending" => {}
            _ => return Err(AppError::internal("invalid suggestion status")),
        }
        let entity = entity.ok_or_else(|| AppError::not_found(&suggestion.entity_id))?;
        if kind == "related" && candidate.is_none() {
            return Err(AppError::not_found(&suggestion.candidate));
        }
        let raw = self
            .vault
            .read_raw(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        if note_content_hash(&raw)? != suggestion.content_hash {
            self.expire(&suggestion.suggestion_id).await?;
            return Err(AppError::conflict(
                "suggestion_expired",
                "suggestion content hash is stale",
            ));
        }
        let result = self
            .vault
            .patch_metadata(
                &entity.file_path,
                &suggestion.content_hash,
                &[patch(suggestion.candidate.clone())],
            )
            .map_err(map_patch_error)?;
        self.refresh_relationships(&suggestion.entity_id, &entity.file_path)
            .await?;
        let (updated, target) = {
            let repository = self.repository.lock().await;
            repository.update_suggestion_status(
                &suggestion.suggestion_id,
                "accepted",
                &Utc::now().to_rfc3339(),
            )?;
            let updated = repository
                .get_suggestion(&suggestion.suggestion_id)?
                .ok_or_else(|| AppError::internal("accepted suggestion disappeared"))?;
            let target = repository.get_entity(&updated.candidate)?;
            (updated, target)
        };
        let mut response = render(&updated, target.as_ref())?;
        response["changed"] = json!(result.changed);
        response["content_hash"] = json!(result.content_hash);
        Ok(response)
    }

    async fn reject<F>(&self, suggestion_id: &str, kind: &str, render: F) -> Result<Value, AppError>
    where
        F: Fn(&CachedSuggestion, Option<&IndexedEntity>) -> Result<Value, AppError>,
    {
        let (suggestion, target) = {
            let repository = self.repository.lock().await;
            let suggestion = repository
                .get_suggestion(suggestion_id)?
                .ok_or_else(|| AppError::not_found(suggestion_id))?;
            let target = repository.get_entity(&suggestion.candidate)?;
            (suggestion, target)
        };
        ensure_kind(&suggestion, kind)?;
        match suggestion.status.as_str() {
            "accepted" => {
                return Err(AppError::conflict(
                    "suggestion_accepted",
                    "accepted suggestion cannot be rejected",
                ))
            }
            "rejected" | "expired" => return render(&suggestion, target.as_ref()),
            "pending" => {}
            _ => return Err(AppError::internal("invalid suggestion status")),
        };
        let updated = {
            let repository = self.repository.lock().await;
            repository.update_suggestion_status(
                &suggestion.suggestion_id,
                "rejected",
                &Utc::now().to_rfc3339(),
            )?;
            repository
                .get_suggestion(&suggestion.suggestion_id)?
                .ok_or_else(|| AppError::internal("rejected suggestion disappeared"))?
        };
        let target = {
            self.repository
                .lock()
                .await
                .get_entity(&updated.candidate)?
        };
        render(&updated, target.as_ref())
    }
    async fn entity(&self, id: &str) -> Result<IndexedEntity, AppError> {
        self.repository
            .lock()
            .await
            .get_entity(id)?
            .ok_or_else(|| AppError::not_found(id))
    }
    async fn expire(&self, id: &str) -> Result<(), AppError> {
        self.repository.lock().await.update_suggestion_status(
            id,
            "expired",
            &Utc::now().to_rfc3339(),
        )?;
        Ok(())
    }
    async fn refresh_relationships(
        &self,
        entity_id: &str,
        file_path: &str,
    ) -> Result<(), AppError> {
        let note = self
            .vault
            .load(file_path)
            .map_err(|error| AppError::internal(&format!("reindex note failed: {error}")))?;
        self.repository.lock().await.replace_entity_relationships(
            entity_id,
            &self.vault.tags(&note),
            &self.vault.refs(&note),
        )?;
        Ok(())
    }
    fn effective(
        &self,
        entity: &IndexedEntity,
        now: chrono::DateTime<Utc>,
    ) -> Result<crate::domain::entity::EffectiveScore, AppError> {
        let note = self
            .vault
            .load(&entity.file_path)
            .map_err(|error| AppError::internal(&format!("read note failed: {error}")))?;
        let freshness = self
            .vault
            .freshness(&note)
            .map_err(|error| AppError::unprocessable(&format!("invalid freshness: {error}")))?;
        let updated = self
            .vault
            .content_updated_at(&note)
            .map_err(|error| {
                AppError::unprocessable(&format!("invalid content_updated_at: {error}"))
            })?
            .or_else(|| entity.updated_at.clone())
            .map(|value| {
                chrono::DateTime::parse_from_rfc3339(&value)
                    .map(|value| value.with_timezone(&Utc))
                    .map_err(|error| {
                        AppError::unprocessable(&format!("invalid content_updated_at: {error}"))
                    })
            })
            .transpose()?;
        crate::domain::scoring::effective_score(
            entity.composite.unwrap_or(0.0),
            &freshness,
            updated,
            now,
        )
        .map_err(|error| AppError::unprocessable(&error.to_string()))
    }
}

fn lexical_tag_candidates(
    frontmatter: &str,
    body: &str,
    existing: &[String],
    hash: &str,
) -> Vec<(String, f64, String, String, String, String)> {
    let mut frequencies = BTreeMap::new();
    if let Some(value) = yaml_scalar(frontmatter, "category") {
        add_terms(&mut frequencies, &value, 2);
    }
    if let Some(value) = yaml_scalar(frontmatter, "title") {
        add_terms(&mut frequencies, &value, 3);
    }
    add_terms(&mut frequencies, body, 1);
    let existing = existing
        .iter()
        .map(|value| value.to_lowercase())
        .collect::<HashSet<_>>();
    let mut values = frequencies
        .into_iter()
        .filter(|(term, count)| {
            *count > 0
                && term.chars().count() >= 2
                && term.chars().count() <= 48
                && !existing.contains(term)
        })
        .collect::<Vec<_>>();
    values.sort_by(|(left_term, left_count), (right_term, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_term.cmp(right_term))
    });
    values
        .into_iter()
        .take(MAX_SUGGESTIONS_PER_REQUEST)
        .map(|(term, count)| {
            (
                term,
                (0.45 + count as f64 * 0.1).min(0.99),
                format!("lexical overlap count: {count}"),
                "rust_lexical".to_string(),
                TAG_ALGORITHM_VERSION.to_string(),
                hash.to_string(),
            )
        })
        .collect()
}
fn add_terms(frequencies: &mut BTreeMap<String, usize>, text: &str, weight: usize) {
    let mut current = String::new();
    let flush = |current: &mut String, frequencies: &mut BTreeMap<String, usize>| {
        if current.chars().count() >= 2 {
            let term = current.to_lowercase();
            if !is_stopword(&term) && !term.chars().all(char::is_numeric) {
                *frequencies.entry(term).or_default() += weight;
            }
        }
        current.clear();
    };
    for character in text.chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else {
            flush(&mut current, frequencies);
        }
    }
    flush(&mut current, frequencies);
}
fn lexical_term_set(frontmatter: &str, body: &str) -> HashSet<String> {
    let mut frequencies = BTreeMap::new();
    if let Some(value) = yaml_scalar(frontmatter, "category") {
        add_terms(&mut frequencies, &value, 1);
    }
    if let Some(value) = yaml_scalar(frontmatter, "title") {
        add_terms(&mut frequencies, &value, 1);
    }
    add_terms(&mut frequencies, body, 1);
    frequencies.into_keys().collect()
}
fn is_stopword(term: &str) -> bool {
    matches!(
        term,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "is"
            | "it"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "with"
    )
}
fn yaml_scalar(frontmatter: &str, key: &str) -> Option<String> {
    serde_yaml::from_str::<serde_yaml::Value>(frontmatter)
        .ok()?
        .as_mapping()?
        .get(serde_yaml::Value::String(key.to_string()))?
        .as_str()
        .map(ToString::to_string)
}
fn ensure_kind(suggestion: &CachedSuggestion, kind: &str) -> Result<(), AppError> {
    if suggestion.kind != kind {
        return Err(AppError::not_found(&suggestion.suggestion_id));
    }
    Ok(())
}
fn map_patch_error(error: anyhow::Error) -> AppError {
    if error.downcast_ref::<MetadataPatchError>().is_some() {
        AppError::conflict("suggestion_expired", &error.to_string())
    } else {
        AppError::internal(&error.to_string())
    }
}
fn tag_suggestion_json(row: &CachedSuggestion) -> Result<Value, AppError> {
    let suggestion = TagSuggestion {
        suggestion_id: row.suggestion_id.clone(),
        entity_id: row.entity_id.clone(),
        tag: row.candidate.clone(),
        confidence: row.confidence.unwrap_or(0.0),
        reason: row.reason.clone(),
        source: row.source.clone(),
        algorithm_version: row.algorithm_version.clone(),
        content_hash: row.content_hash.clone(),
        status: parse_status(&row.status)?,
    };
    serde_json::to_value(suggestion)
        .map_err(|error| AppError::internal(&format!("serialize suggestion failed: {error}")))
}
fn related_suggestion_json(
    row: &CachedSuggestion,
    candidate: &IndexedEntity,
    score: f64,
) -> Result<Value, AppError> {
    let mut value = related_suggestion_json_from_row(row, Some(candidate))?;
    value["score"] = json!(score);
    Ok(value)
}
fn related_suggestion_json_from_row(
    row: &CachedSuggestion,
    candidate: Option<&IndexedEntity>,
) -> Result<Value, AppError> {
    let payload = serde_json::from_str::<Value>(&row.reason).ok();
    let reasons = payload
        .as_ref()
        .and_then(|value| value.get("reasons"))
        .and_then(|value| serde_json::from_value::<Vec<String>>(value.clone()).ok())
        .or_else(|| serde_json::from_str::<Vec<String>>(&row.reason).ok())
        .unwrap_or_else(|| vec![row.reason.clone()]);
    let signals = payload
        .as_ref()
        .and_then(|value| value.get("signals"))
        .and_then(|value| serde_json::from_value::<RelatedSignals>(value.clone()).ok())
        .unwrap_or_default();
    serde_json::to_value(RelatedSuggestion {
        suggestion_id: row.suggestion_id.clone(),
        entity_id: row.entity_id.clone(),
        id: row.candidate.clone(),
        title: candidate.and_then(|value| value.title.clone()),
        composite: candidate.and_then(|value| value.composite),
        score: row.confidence.unwrap_or(0.0),
        reasons,
        signals,
        content_hash: row.content_hash.clone(),
        source: row.source.clone(),
        algorithm_version: row.algorithm_version.clone(),
        status: parse_status(&row.status)?,
    })
    .map_err(|error| AppError::internal(&format!("serialize related suggestion failed: {error}")))
}
fn parse_status(status: &str) -> Result<SuggestionStatus, AppError> {
    match status {
        "pending" => Ok(SuggestionStatus::Pending),
        "accepted" => Ok(SuggestionStatus::Accepted),
        "rejected" => Ok(SuggestionStatus::Rejected),
        "expired" => Ok(SuggestionStatus::Expired),
        _ => Err(AppError::internal("invalid suggestion status")),
    }
}
