//! HTTP 路由（T1.6）：PRD §7 的 7 个端点 + /health。
//!
//! 字段统一（#160）：响应用 `id`/`composite`（PRD v3.0），非 v2.x 的 `entity_id`/`final_score`。
//! 写回端点（score/access/create）：读 frontmatter -> 改 score -> 原子写回 -> 更新 SQLite。

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::config::Config;
use crate::contracts::{
    candidate_key, note_content_hash, stable_suggestion_id, SuggestionKind, SuggestionStatus,
    TagSuggestion, TAG_ALGORITHM_VERSION,
};
use crate::db::{Db, EntityRow, SuggestionRow};
use crate::frontmatter::{self, MetadataPatch, MetadataPatchError};
use crate::models::Weights;
use crate::scoring;

/// 共享应用状态
#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Db>>,
    pub vault: PathBuf,
    pub weights: Weights,
}

#[derive(Debug, Serialize)]
pub struct EntitySummary {
    pub id: String,
    pub title: Option<String>,
    pub layer: Option<String>,
    pub composite: Option<f64>,
    pub strategy: Option<f64>,
    pub last_boosted_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScoreResponse {
    pub interest: f64,
    pub strategy: f64,
    pub consensus: f64,
    pub composite: f64,
    pub access_count: i64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EntityDetail {
    pub id: String,
    pub title: Option<String>,
    pub layer: Option<String>,
    pub status: Option<String>,
    pub file_path: String,
    pub score: Option<ScoreResponse>,
    pub refs: Vec<String>,
}
/// 引力场节点
#[derive(Debug, Serialize)]
pub struct GraphNode {
    pub id: String,
    pub title: Option<String>,
    pub layer: Option<String>,
    pub composite: Option<f64>,
}

/// 引力场边
#[derive(Debug, Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

/// 引力场数据（GET /graph）
#[derive(Debug, Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub id: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub layer: Option<String>,
    pub composite: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEntityRequest {
    pub title: String,
    #[serde(default = "default_layer")]
    pub layer: String,
    pub content: Option<String>,
    pub interest: Option<f64>,
    pub strategy: Option<f64>,
    pub consensus: Option<f64>,
}

fn default_layer() -> String {
    "knowledge".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ScoreUpdateRequest {
    pub interest: Option<f64>,
    pub strategy: Option<f64>,
    pub consensus: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct AccessRequest {
    pub depth: String,
}

#[derive(Debug, Deserialize)]
pub struct AgentContextRequest {
    pub task: String,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
}

fn default_top_k() -> usize {
    5
}

#[derive(Debug, Serialize)]
pub struct AgentContextEntry {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<String>,
    pub composite: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct AgentContextResponse {
    pub context: Vec<AgentContextEntry>,
    pub reasoning: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedQuery {
    #[serde(default = "default_feed_mode")]
    pub mode: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

fn default_feed_mode() -> String {
    "explore".to_string()
}

fn default_limit() -> u32 {
    20
}

#[derive(Debug, Deserialize)]
pub struct TopQuery {
    pub layer: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    #[serde(default = "default_limit")]
    pub limit: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct TagSuggestionsRequest {
    #[serde(default)]
    pub candidates: Vec<TagCandidateRequest>,
}

#[derive(Debug, Deserialize)]
pub struct TagCandidateRequest {
    pub tag: String,
    pub confidence: f64,
    pub reason: String,
    pub source: String,
    pub algorithm_version: String,
    pub content_hash: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/feed", get(feed))
        .route("/entities/top", get(entities_top))
        .route("/entities/:id", get(get_entity))
        .route("/search", get(search))
        .route("/entities/:id/tag-suggestions", post(tag_suggestions))
        .route(
            "/tag-suggestions/:suggestion_id/accept",
            post(accept_tag_suggestion),
        )
        .route(
            "/tag-suggestions/:suggestion_id/reject",
            post(reject_tag_suggestion),
        )
        .route("/entities", post(create_entity))
        .route("/entities/:id/score", patch(update_score))
        .route("/entities/:id/access", patch(record_access))
        .route("/agent/context", post(agent_context))
        .route("/graph", get(graph))
        .with_state(state)
}

pub(crate) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "name": "compass",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

pub(crate) async fn feed(
    State(state): State<AppState>,
    Query(q): Query<FeedQuery>,
) -> Result<Json<Vec<EntitySummary>>, AppError> {
    let db = state.db.lock().await;
    let entities = db.list_entities()?;
    let mut summaries: Vec<EntitySummary> = entities
        .into_iter()
        .map(|e| EntitySummary {
            id: e.id,
            title: e.title,
            layer: e.layer,
            composite: e.composite,
            strategy: e.strategy,
            last_boosted_at: e.last_boosted_at,
        })
        .collect();
    match q.mode.as_str() {
        "strategic" => {
            summaries.sort_by(|a, b| {
                b.strategy
                    .unwrap_or(0.0)
                    .partial_cmp(&a.strategy.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        "consolidate" => {
            summaries.sort_by(|a, b| match (&a.last_boosted_at, &b.last_boosted_at) {
                (Some(a), Some(b)) => a.cmp(b),
                (Some(_), None) => std::cmp::Ordering::Greater,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (None, None) => std::cmp::Ordering::Equal,
            });
        }
        _ => {
            summaries.sort_by(|a, b| {
                b.composite
                    .unwrap_or(0.0)
                    .partial_cmp(&a.composite.unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }
    summaries.truncate(q.limit as usize);
    Ok(Json(summaries))
}

pub(crate) async fn entities_top(
    State(state): State<AppState>,
    Query(q): Query<TopQuery>,
) -> Result<Json<Vec<EntitySummary>>, AppError> {
    let db = state.db.lock().await;
    let entities = db.list_entities()?;
    let mut summaries: Vec<EntitySummary> = entities
        .into_iter()
        .filter(|e| {
            q.layer
                .as_deref()
                .is_none_or(|l| e.layer.as_deref() == Some(l))
        })
        .map(|e| EntitySummary {
            id: e.id,
            title: e.title,
            layer: e.layer,
            composite: e.composite,
            strategy: e.strategy,
            last_boosted_at: e.last_boosted_at,
        })
        .collect();
    summaries.sort_by(|a, b| {
        b.composite
            .unwrap_or(0.0)
            .partial_cmp(&a.composite.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    summaries.truncate(q.limit as usize);
    Ok(Json(summaries))
}

pub(crate) async fn get_entity(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EntityDetail>, AppError> {
    let db = state.db.lock().await;
    let entity = db
        .get_entity(&id)?
        .ok_or_else(|| AppError::not_found(&id))?;
    let file_path = state.vault.join(&entity.file_path);
    let refs = extract_refs(&file_path).unwrap_or_default();
    let score = entity.composite.map(|composite| ScoreResponse {
        interest: entity.interest.unwrap_or(0.0),
        strategy: entity.strategy.unwrap_or(0.0),
        consensus: entity.consensus.unwrap_or(0.0),
        composite,
        access_count: entity.access_count,
        updated_at: entity.updated_at.clone(),
    });
    Ok(Json(EntityDetail {
        id: entity.id,
        title: entity.title,
        layer: entity.layer,
        status: entity.status,
        file_path: entity.file_path,
        score,
        refs,
    }))
}

pub(crate) async fn search(
    State(state): State<AppState>,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, AppError> {
    let db = state.db.lock().await;
    let hits = db.fts_search(&q.q, q.limit)?;
    let results: Vec<SearchHit> = hits
        .into_iter()
        .map(|h| {
            // 补充 composite/layer 供 skill render 显示评分与分类
            let (layer, composite) = db
                .get_entity(&h.id)
                .ok()
                .flatten()
                .map(|e| (e.layer, e.composite))
                .unwrap_or((None, None));
            SearchHit {
                id: h.id,
                title: h.title,
                snippet: h.snippet,
                layer,
                composite,
            }
        })
        .collect();
    Ok(Json(results))
}

pub(crate) async fn tag_suggestions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    body: Option<Json<TagSuggestionsRequest>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().await;
    let entity = db
        .get_entity(&id)?
        .ok_or_else(|| AppError::not_found(&id))?;
    let file_path = state.vault.join(&entity.file_path);
    let raw = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::internal(&format!("读取笔记失败: {e}")))?;
    let note = frontmatter::read_note(&file_path)
        .map_err(|e| AppError::internal(&format!("解析笔记失败: {e}")))?;
    let content_hash = note_content_hash(&raw)?;
    let existing = frontmatter::extract_tags(&note.frontmatter);
    let candidates = match body.map(|json| json.0).unwrap_or_default().candidates {
        candidates if candidates.is_empty() => {
            lexical_tag_candidates(&note.frontmatter, &note.body, &existing, &content_hash)
        }
        candidates => candidates
            .into_iter()
            .map(|candidate| {
                if candidate.content_hash != content_hash {
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
                let tag = crate::contracts::normalize_tag(&candidate.tag)
                    .map_err(|e| AppError::bad_request(&e.to_string()))?;
                Ok((
                    tag,
                    candidate.confidence,
                    candidate.reason,
                    candidate.source,
                    candidate.algorithm_version,
                    candidate.content_hash,
                ))
            })
            .collect::<Result<Vec<_>, AppError>>()?,
    };

    let now = chrono::Utc::now().to_rfc3339();
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
        let suggestion = SuggestionRow {
            suggestion_id: stable_suggestion_id(
                SuggestionKind::Tag,
                &id,
                &tag,
                &candidate_hash,
                &algorithm_version,
                &source,
            ),
            kind: "tag".to_string(),
            entity_id: id.clone(),
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
        db.upsert_suggestion(&suggestion)?;
        if let Some(row) = db.get_suggestion(&suggestion.suggestion_id)? {
            suggestions.push(tag_suggestion_json(&row)?);
        }
    }
    Ok(Json(serde_json::json!({
        "entity_id": id,
        "content_hash": content_hash,
        "suggestions": suggestions,
    })))
}

pub(crate) async fn accept_tag_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().await;
    let suggestion = db
        .get_suggestion(&suggestion_id)?
        .ok_or_else(|| AppError::not_found(&suggestion_id))?;
    ensure_suggestion_kind(&suggestion, "tag")?;
    match suggestion.status.as_str() {
        "accepted" | "rejected" | "expired" => {
            if suggestion.status == "rejected" {
                return Err(AppError::conflict(
                    "suggestion_rejected",
                    "rejected suggestion cannot be accepted",
                ));
            }
            return Ok(Json(tag_suggestion_json(&suggestion)?));
        }
        "pending" => {}
        _ => return Err(AppError::internal("invalid suggestion status")),
    }
    let entity = db
        .get_entity(&suggestion.entity_id)?
        .ok_or_else(|| AppError::not_found(&suggestion.entity_id))?;
    let file_path = state.vault.join(&entity.file_path);
    let raw = std::fs::read_to_string(&file_path)
        .map_err(|e| AppError::internal(&format!("读取笔记失败: {e}")))?;
    let actual_hash = note_content_hash(&raw)?;
    if actual_hash != suggestion.content_hash {
        expire_suggestion(&db, &suggestion.suggestion_id)?;
        return Err(AppError::conflict(
            "suggestion_expired",
            "suggestion content hash is stale",
        ));
    }
    let result = frontmatter::patch_metadata(
        &file_path,
        &suggestion.content_hash,
        &[MetadataPatch::AddTag(suggestion.candidate.clone())],
    )
    .map_err(map_metadata_patch_error)?;
    refresh_relationship_index(&db, &suggestion.entity_id, &file_path)?;
    db.update_suggestion_status(
        &suggestion.suggestion_id,
        "accepted",
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let updated = db
        .get_suggestion(&suggestion.suggestion_id)?
        .ok_or_else(|| AppError::internal("accepted suggestion disappeared"))?;
    let mut response = tag_suggestion_json(&updated)?;
    response["changed"] = serde_json::json!(result.changed);
    response["content_hash"] = serde_json::json!(result.content_hash);
    Ok(Json(response))
}

pub(crate) async fn reject_tag_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().await;
    let suggestion = db
        .get_suggestion(&suggestion_id)?
        .ok_or_else(|| AppError::not_found(&suggestion_id))?;
    ensure_suggestion_kind(&suggestion, "tag")?;
    match suggestion.status.as_str() {
        "accepted" => {
            return Err(AppError::conflict(
                "suggestion_accepted",
                "accepted suggestion cannot be rejected",
            ));
        }
        "rejected" | "expired" => return Ok(Json(tag_suggestion_json(&suggestion)?)),
        "pending" => {}
        _ => return Err(AppError::internal("invalid suggestion status")),
    }
    db.update_suggestion_status(
        &suggestion.suggestion_id,
        "rejected",
        &chrono::Utc::now().to_rfc3339(),
    )?;
    let updated = db
        .get_suggestion(&suggestion.suggestion_id)?
        .ok_or_else(|| AppError::internal("rejected suggestion disappeared"))?;
    Ok(Json(tag_suggestion_json(&updated)?))
}

pub(crate) async fn create_entity(
    State(state): State<AppState>,
    Json(req): Json<CreateEntityRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let db = state.db.lock().await;
    let prefix = layer_prefix(&req.layer);
    let id = next_id(&db, prefix)?;
    let dir = layer_dir(&req.layer);
    let file_dir = state.vault.join(&dir);
    std::fs::create_dir_all(&file_dir)
        .map_err(|e| AppError::internal(&format!("创建目录失败: {e}")))?;
    let file_path = file_dir.join(format!("{id}.md"));
    let interest = req.interest.unwrap_or(5.0);
    let strategy = req.strategy.unwrap_or(5.0);
    let consensus = req.consensus.unwrap_or(5.0);
    let composite = scoring::composite(interest, strategy, consensus, &state.weights);
    let now = chrono::Utc::now().to_rfc3339();
    let content = req.content.unwrap_or_default();
    let md = format!(
        "---\nid: {id}\ntitle: {}\nlayer: {}\nstatus: active\nscore:\n  interest: {interest}\n  strategy: {strategy}\n  consensus: {consensus}\n  composite: {composite}\n  weights:\n    interest: {}\n    strategy: {}\n    consensus: {}\n  updated_at: '{now}'\n  last_boosted_at: '{now}'\n  access_count: 0\n---\n{content}\n",
        req.title, req.layer,
        state.weights.interest, state.weights.strategy, state.weights.consensus
    );
    std::fs::write(&file_path, md).map_err(|e| AppError::internal(&format!("写文件失败: {e}")))?;
    let rel = file_path
        .strip_prefix(&state.vault)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file_path.to_string_lossy().into_owned());
    let entity = EntityRow {
        id: id.clone(),
        file_path: rel,
        title: Some(req.title.clone()),
        layer: Some(req.layer.clone()),
        status: Some("active".to_string()),
        interest: Some(interest),
        strategy: Some(strategy),
        consensus: Some(consensus),
        composite: Some(composite),
        access_count: 0,
        last_boosted_at: Some(now.clone()),
        content_hash: Some(content_hash(&content)),
        updated_at: Some(now),
    };
    db.upsert_entity(&entity, &content)?;
    info!(id = %id, "entity created");
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "id": id,
            "title": req.title,
            "file_path": entity.file_path,
            "composite": composite,
        })),
    ))
}

pub(crate) async fn update_score(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ScoreUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().await;
    let entity = db
        .get_entity(&id)?
        .ok_or_else(|| AppError::not_found(&id))?;
    let file_path = state.vault.join(&entity.file_path);
    let note = frontmatter::read_note(&file_path)
        .map_err(|e| AppError::internal(&format!("读取笔记失败: {e}")))?;
    let mut score = frontmatter::get_score(&note.frontmatter)
        .map_err(|e| AppError::internal(&format!("解析 score 失败: {e}")))?
        .ok_or_else(|| AppError::internal("笔记无 score 块"))?;
    let old_composite = score.composite;
    if let Some(i) = req.interest {
        score.interest = i.clamp(0.0, 100.0);
    }
    if let Some(s) = req.strategy {
        score.strategy = s.clamp(0.0, 100.0);
    }
    if let Some(c) = req.consensus {
        score.consensus = c.clamp(0.0, 100.0);
    }
    let w = score.weights.unwrap_or(state.weights);
    score.composite = scoring::composite(score.interest, score.strategy, score.consensus, &w);
    let now = chrono::Utc::now().to_rfc3339();
    score.updated_at = now.clone();
    frontmatter::write_score(&file_path, &score)
        .map_err(|e| AppError::internal(&format!("写回 score 失败: {e}")))?;
    let _ = db.insert_score_history(&crate::db::ScoreHistoryRow {
        entity_id: id.clone(),
        dimension: Some("manual".to_string()),
        old: Some(old_composite),
        new: Some(score.composite),
        reason: Some("manual_adjust".to_string()),
        trigger: Some("ManualMark".to_string()),
        created_at: now.clone(),
    });
    let updated = EntityRow {
        id: id.clone(),
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
    };
    db.upsert_entity(&updated, &note.body)?;
    debug!(id = %id, old = old_composite, new = score.composite, "score updated");
    Ok(Json(serde_json::json!({
        "id": id,
        "score": {
            "interest": score.interest,
            "strategy": score.strategy,
            "consensus": score.consensus,
            "composite": score.composite,
            "access_count": score.access_count,
            "updated_at": score.updated_at,
        }
    })))
}

pub(crate) async fn record_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AccessRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let db = state.db.lock().await;
    let entity = db
        .get_entity(&id)?
        .ok_or_else(|| AppError::not_found(&id))?;
    let file_path = state.vault.join(&entity.file_path);
    let note = frontmatter::read_note(&file_path)
        .map_err(|e| AppError::internal(&format!("读取笔记失败: {e}")))?;
    let mut score = frontmatter::get_score(&note.frontmatter)
        .map_err(|e| AppError::internal(&format!("解析 score 失败: {e}")))?
        .ok_or_else(|| AppError::internal("笔记无 score 块"))?;
    let depth = parse_access_depth(&req.depth)
        .ok_or_else(|| AppError::bad_request(&format!("未知 access depth: {}", req.depth)))?;
    let now = chrono::Utc::now().to_rfc3339();
    score = scoring::apply_access(&score, depth, &now);
    frontmatter::write_score(&file_path, &score)
        .map_err(|e| AppError::internal(&format!("写回 score 失败: {e}")))?;
    let _ = db.insert_timeline(&crate::db::TimelineRow {
        entity_id: id.clone(),
        event_type: "access".to_string(),
        intensity: Some(match depth {
            scoring::AccessDepth::Glance => 0.0,
            scoring::AccessDepth::Read => 1.0,
            scoring::AccessDepth::Study => 3.0,
            scoring::AccessDepth::Apply => 5.0,
        }),
        source: Some("api".to_string()),
        created_at: now.clone(),
    });
    let updated = EntityRow {
        id: id.clone(),
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
    };
    db.upsert_entity(&updated, &note.body)?;
    debug!(id = %id, ?depth, composite = score.composite, "access recorded");
    Ok(Json(serde_json::json!({
        "id": id,
        "score": {
            "interest": score.interest,
            "strategy": score.strategy,
            "consensus": score.consensus,
            "composite": score.composite,
            "access_count": score.access_count,
            "updated_at": score.updated_at,
        }
    })))
}

pub(crate) async fn agent_context(
    State(state): State<AppState>,
    Json(req): Json<AgentContextRequest>,
) -> Result<Json<AgentContextResponse>, AppError> {
    let db = state.db.lock().await;

    // 1. FTS5 语义召回（最多 3*top_k，留足排序空间）
    let recall_limit = (req.top_k * 3).max(10) as u32;
    let hits = db.fts_search(&req.task, recall_limit)?;

    // 2. 组装上下文：读实体 + 内容片段，按 composite 加权排序
    let mut entries: Vec<AgentContextEntry> = Vec::new();
    for hit in hits {
        let entity = match db.get_entity(&hit.id)? {
            Some(e) => e,
            None => continue,
        };
        entries.push(AgentContextEntry {
            id: entity.id,
            title: hit.title,
            content: hit.snippet,
            composite: entity.composite,
        });
    }
    entries.sort_by(|a, b| {
        b.composite
            .unwrap_or(0.0)
            .partial_cmp(&a.composite.unwrap_or(0.0))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    entries.truncate(req.top_k);

    let reasoning = format!(
        "从 vault 中召回 {} 个相关实体，按 composite 评分加权取前 {} 个作为上下文。",
        entries.len(),
        req.top_k
    );

    Ok(Json(AgentContextResponse {
        context: entries,
        reasoning,
    }))
}

pub(crate) async fn graph(State(state): State<AppState>) -> Result<Json<GraphData>, AppError> {
    let db = state.db.lock().await;
    let entities = db.list_entities()?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for entity in &entities {
        nodes.push(GraphNode {
            id: entity.id.clone(),
            title: entity.title.clone(),
            layer: entity.layer.clone(),
            composite: entity.composite,
        });

        // 提取 [[id]] 引用作为边
        let file_path = state.vault.join(&entity.file_path);
        if let Ok(refs) = extract_refs(&file_path) {
            for target in refs {
                // 只添加指向已存在实体的边
                if entities.iter().any(|e| e.id == target) {
                    edges.push(GraphEdge {
                        source: entity.id.clone(),
                        target,
                    });
                }
            }
        }
    }

    Ok(Json(GraphData { nodes, edges }))
}
fn extract_refs(file_path: &std::path::Path) -> Result<Vec<String>, anyhow::Error> {
    let note = frontmatter::read_note(file_path)?;
    Ok(frontmatter::extract_refs(&note.body))
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

fn layer_dir(layer: &str) -> String {
    match layer.to_lowercase().as_str() {
        "direction" => "Direction",
        "knowledge" => "Knowledge",
        "case" => "Cases",
        "log" => "Logs",
        "insight" => "Insights",
        _ => "Inbox",
    }
    .to_string()
}

fn next_id(db: &Db, prefix: &str) -> Result<String, anyhow::Error> {
    let entities = db.list_entities()?;
    let max_seq = entities
        .iter()
        .filter_map(|e| {
            if e.id.starts_with(prefix) {
                e.id[prefix.len()..].parse::<u32>().ok()
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);
    Ok(format!("{prefix}{:06}", max_seq + 1))
}

fn parse_access_depth(s: &str) -> Option<scoring::AccessDepth> {
    match s.to_lowercase().as_str() {
        "glance" => Some(scoring::AccessDepth::Glance),
        "read" => Some(scoring::AccessDepth::Read),
        "study" => Some(scoring::AccessDepth::Study),
        "apply" => Some(scoring::AccessDepth::Apply),
        _ => None,
    }
}

fn lexical_tag_candidates(
    frontmatter: &str,
    body: &str,
    existing: &[String],
    content_hash: &str,
) -> Vec<(String, f64, String, String, String, String)> {
    let mut frequencies = BTreeMap::<String, usize>::new();
    if let Some(category) = yaml_scalar(frontmatter, "category") {
        add_lexical_terms(&mut frequencies, &category, 2);
    }
    if let Some(title) = yaml_scalar(frontmatter, "title") {
        add_lexical_terms(&mut frequencies, &title, 3);
    }
    add_lexical_terms(&mut frequencies, body, 1);

    let existing = existing
        .iter()
        .map(|value| value.to_lowercase())
        .collect::<std::collections::HashSet<_>>();
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
        .take(crate::contracts::MAX_SUGGESTIONS_PER_REQUEST)
        .map(|(term, count)| {
            let confidence = (0.45 + count as f64 * 0.1).min(0.99);
            (
                term,
                confidence,
                format!("lexical overlap count: {count}"),
                "rust_lexical".to_string(),
                TAG_ALGORITHM_VERSION.to_string(),
                content_hash.to_string(),
            )
        })
        .collect()
}

fn add_lexical_terms(frequencies: &mut BTreeMap<String, usize>, text: &str, weight: usize) {
    let mut current = String::new();
    let mut flush = |current: &mut String| {
        if current.chars().count() >= 2 {
            let term = current.to_lowercase();
            if !is_tag_stopword(&term) && !term.chars().all(char::is_numeric) {
                *frequencies.entry(term).or_default() += weight;
            }
        }
        current.clear();
    };
    for character in text.chars() {
        if character.is_alphanumeric() {
            current.push(character);
        } else {
            flush(&mut current);
        }
    }
    flush(&mut current);
}

fn is_tag_stopword(term: &str) -> bool {
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
    let value = serde_yaml::from_str::<serde_yaml::Value>(frontmatter).ok()?;
    value
        .as_mapping()?
        .get(serde_yaml::Value::String(key.to_string()))?
        .as_str()
        .map(ToString::to_string)
}

fn tag_suggestion_json(row: &SuggestionRow) -> Result<serde_json::Value, AppError> {
    let status = parse_suggestion_status(&row.status)?;
    let suggestion = TagSuggestion {
        suggestion_id: row.suggestion_id.clone(),
        entity_id: row.entity_id.clone(),
        tag: row.candidate.clone(),
        confidence: row.confidence.unwrap_or(0.0),
        reason: row.reason.clone(),
        source: row.source.clone(),
        algorithm_version: row.algorithm_version.clone(),
        content_hash: row.content_hash.clone(),
        status,
    };
    serde_json::to_value(suggestion)
        .map_err(|e| AppError::internal(&format!("serialize suggestion failed: {e}")))
}

fn parse_suggestion_status(status: &str) -> Result<SuggestionStatus, AppError> {
    match status {
        "pending" => Ok(SuggestionStatus::Pending),
        "accepted" => Ok(SuggestionStatus::Accepted),
        "rejected" => Ok(SuggestionStatus::Rejected),
        "expired" => Ok(SuggestionStatus::Expired),
        _ => Err(AppError::internal("invalid suggestion status")),
    }
}

fn ensure_suggestion_kind(suggestion: &SuggestionRow, expected: &str) -> Result<(), AppError> {
    if suggestion.kind != expected {
        return Err(AppError::not_found(&suggestion.suggestion_id));
    }
    Ok(())
}

fn expire_suggestion(db: &Db, suggestion_id: &str) -> Result<(), AppError> {
    db.update_suggestion_status(suggestion_id, "expired", &chrono::Utc::now().to_rfc3339())?;
    Ok(())
}

fn refresh_relationship_index(
    db: &Db,
    entity_id: &str,
    path: &std::path::Path,
) -> Result<(), AppError> {
    let note = frontmatter::read_note(path)
        .map_err(|e| AppError::internal(&format!("重新索引笔记失败: {e}")))?;
    db.replace_entity_relationships(
        entity_id,
        &frontmatter::extract_tags(&note.frontmatter),
        &frontmatter::extract_refs(&note.body),
    )?;
    Ok(())
}

fn map_metadata_patch_error(error: anyhow::Error) -> AppError {
    if error.downcast_ref::<MetadataPatchError>().is_some() {
        return AppError::conflict("suggestion_expired", &error.to_string());
    }
    AppError::internal(&error.to_string())
}

fn content_hash(s: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    BadRequest(String),
    Conflict { code: String, message: String },
    Internal(String),
}

impl AppError {
    fn not_found(id: &str) -> Self {
        Self::NotFound(id.to_string())
    }
    fn bad_request(msg: &str) -> Self {
        Self::BadRequest(msg.to_string())
    }
    fn conflict(code: &str, msg: &str) -> Self {
        Self::Conflict {
            code: code.to_string(),
            message: msg.to_string(),
        }
    }
    fn internal(msg: &str) -> Self {
        Self::Internal(msg.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match self {
            AppError::NotFound(id) => (StatusCode::NOT_FOUND, format!("entity not found: {id}")),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Conflict { code, message } => {
                return (
                    StatusCode::CONFLICT,
                    Json(serde_json::json!({
                        "code": code,
                        "message": message,
                        "details": {}
                    })),
                )
                    .into_response();
            }
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };
        (status, Json(serde_json::json!({ "error": msg }))).into_response()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Internal(format!("db error: {e}"))
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(format!("{e}"))
    }
}

pub fn router_from_config(cfg: Arc<Config>, db: Arc<Mutex<Db>>) -> Router {
    let state = AppState {
        db,
        vault: cfg.vault_path.clone(),
        weights: cfg.weights,
    };
    router(state)
}

pub fn apply_security(router: Router, cfg: &Config) -> Router {
    router
        .layer(middleware::from_fn_with_state(
            cfg.auth_token.clone(),
            require_auth,
        ))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(
            cfg.request_body_limit_bytes,
        ))
}

async fn require_auth(
    State(expected): State<Option<String>>,
    request: Request,
    next: Next,
) -> Response {
    let authorized = expected.as_deref().is_none_or(|expected| {
        request
            .headers()
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .is_some_and(|provided| constant_time_eq(provided.as_bytes(), expected.as_bytes()))
    });

    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "authentication required"})),
        )
            .into_response();
    }

    next.run(request).await
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |difference, (left, right)| difference | (left ^ right))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use std::fs;
    use tempfile::tempdir;
    use tower::ServiceExt;

    fn setup_state(vault: &std::path::Path) -> AppState {
        AppState {
            db: Arc::new(Mutex::new(Db::open_in_memory().unwrap())),
            vault: vault.to_path_buf(),
            weights: Weights::default(),
        }
    }

    fn sample_md(id: &str, title: &str, composite: f64, body: &str) -> String {
        format!(
            "---\nid: {id}\ntitle: {title}\nlayer: knowledge\nstatus: active\nscore:\n  interest: 80.0\n  strategy: 90.0\n  consensus: 70.0\n  composite: {composite}\n  updated_at: '2026-07-09T00:00:00Z'\n  last_boosted_at: '2026-07-09T00:00:00Z'\n  access_count: 5\n---\n{body}\n"
        )
    }

    fn sample_entity(id: &str, fp: &str, composite: f64, layer: &str) -> EntityRow {
        EntityRow {
            id: id.to_string(),
            file_path: fp.to_string(),
            title: Some("T".to_string()),
            layer: Some(layer.to_string()),
            status: Some("active".to_string()),
            interest: Some(80.0),
            strategy: Some(90.0),
            consensus: Some(70.0),
            composite: Some(composite),
            access_count: 5,
            last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
            content_hash: Some("abc".to_string()),
            updated_at: Some("2026-07-09T00:00:00Z".to_string()),
        }
    }

    fn test_config(vault: &std::path::Path, auth_token: Option<&str>, limit: usize) -> Config {
        Config {
            vault_path: vault.to_path_buf(),
            bind: "127.0.0.1".to_string(),
            port: 8080,
            allow_non_local: false,
            auth_token: auth_token.map(str::to_string),
            request_body_limit_bytes: limit,
            db_path: None,
            decay: crate::config::DecayConfig::default(),
            weights: Weights::default(),
        }
    }

    #[tokio::test]
    async fn test_http_authentication_middleware() {
        let dir = tempdir().unwrap();
        let web_dir = dir.path().join("web");
        fs::create_dir_all(&web_dir).unwrap();
        fs::write(web_dir.join("index.html"), "private static content").unwrap();
        let cfg = Arc::new(test_config(dir.path(), Some("test-secret"), 1024));
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let app = apply_security(
            router_from_config(Arc::clone(&cfg), db)
                .fallback_service(tower_http::services::ServeDir::new(web_dir)),
            &cfg,
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index.html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/missing")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/index.html")
                    .header("authorization", "Bearer test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .header("authorization", "Bearer test-secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_http_request_body_limit() {
        let dir = tempdir().unwrap();
        let cfg = Arc::new(test_config(dir.path(), Some("test-secret"), 64));
        let db = Arc::new(Mutex::new(Db::open_in_memory().unwrap()));
        let app = apply_security(router_from_config(cfg.clone(), db), &cfg);
        let body =
            r#"{"title":"a note whose request is intentionally too large","content":"body"}"#;

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/entities")
                    .header("content-type", "application/json")
                    .header("content-length", body.len())
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn test_tag_suggestions_accept_is_idempotent_and_reject_is_read_only() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let path = dir.path().join("note.md");
        fs::write(
            &path,
            sample_md(
                "know-tag",
                "Rust SQLite",
                70.0,
                "Rust and SQLite make a durable local index.",
            ),
        )
        .unwrap();
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know-tag", "note.md", 70.0, "knowledge"),
            "Rust and SQLite make a durable local index.",
        )
        .unwrap();
        drop(db);

        let created = tag_suggestions(State(state.clone()), Path("know-tag".to_string()), None)
            .await
            .unwrap()
            .0;
        let suggestions = created["suggestions"].as_array().unwrap();
        assert!(!suggestions.is_empty());
        let accept_id = suggestions[0]["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();
        let reject_id = suggestions[1]["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();

        let accepted = accept_tag_suggestion(State(state.clone()), Path(accept_id.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(accepted["status"], "accepted");
        let after_accept = fs::read_to_string(&path).unwrap();
        assert!(after_accept.contains("tags:"));
        assert!(after_accept.contains(&format!("  - {}", suggestions[0]["tag"].as_str().unwrap())));

        let repeated = accept_tag_suggestion(State(state.clone()), Path(accept_id))
            .await
            .unwrap()
            .0;
        assert_eq!(repeated["status"], "accepted");
        assert_eq!(fs::read_to_string(&path).unwrap(), after_accept);

        let before_reject = fs::read_to_string(&path).unwrap();
        let rejected = reject_tag_suggestion(State(state), Path(reject_id))
            .await
            .unwrap()
            .0;
        assert_eq!(rejected["status"], "rejected");
        assert_eq!(fs::read_to_string(&path).unwrap(), before_reject);
    }

    #[tokio::test]
    async fn test_tag_suggestion_stale_accept_returns_conflict_and_expires() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let path = dir.path().join("note.md");
        fs::write(&path, sample_md("know-stale", "Rust", 70.0, "Rust body.")).unwrap();
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know-stale", "note.md", 70.0, "knowledge"),
            "Rust body.",
        )
        .unwrap();
        drop(db);

        let created = tag_suggestions(State(state.clone()), Path("know-stale".to_string()), None)
            .await
            .unwrap()
            .0;
        let suggestion_id = created["suggestions"][0]["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();
        fs::write(
            &path,
            format!("{}\nchanged", fs::read_to_string(&path).unwrap()),
        )
        .unwrap();

        let error = accept_tag_suggestion(State(state.clone()), Path(suggestion_id.clone()))
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Conflict { .. }));
        let row = state
            .db
            .lock()
            .await
            .get_suggestion(&suggestion_id)
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "expired");
    }

    #[test]
    fn test_layer_prefix() {
        assert_eq!(layer_prefix("knowledge"), "know");
        assert_eq!(layer_prefix("case"), "case");
        assert_eq!(layer_prefix("direction"), "dir");
        assert_eq!(layer_prefix("unknown"), "know");
    }

    #[test]
    fn test_layer_dir() {
        assert_eq!(layer_dir("knowledge"), "Knowledge");
        assert_eq!(layer_dir("case"), "Cases");
        assert_eq!(layer_dir("unknown"), "Inbox");
    }

    #[test]
    fn test_parse_access_depth() {
        assert_eq!(
            parse_access_depth("glance"),
            Some(scoring::AccessDepth::Glance)
        );
        assert_eq!(parse_access_depth("read"), Some(scoring::AccessDepth::Read));
        assert_eq!(
            parse_access_depth("study"),
            Some(scoring::AccessDepth::Study)
        );
        assert_eq!(
            parse_access_depth("apply"),
            Some(scoring::AccessDepth::Apply)
        );
        assert_eq!(parse_access_depth("invalid"), None);
    }

    #[test]
    fn test_next_id_empty() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(next_id(&db, "know").unwrap(), "know000001");
    }

    #[test]
    fn test_next_id_increment() {
        let db = Db::open_in_memory().unwrap();
        db.upsert_entity(
            &sample_entity("know000001", "a.md", 50.0, "knowledge"),
            "body",
        )
        .unwrap();
        assert_eq!(next_id(&db, "know").unwrap(), "know000002");
        assert_eq!(next_id(&db, "case").unwrap(), "case000001");
    }

    #[test]
    fn test_extract_refs() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(
            &path,
            sample_md("know-1", "T", 50.0, "links [[know-2]] and [[know-3]]"),
        )
        .unwrap();
        let refs = extract_refs(&path).unwrap();
        assert_eq!(refs, vec!["know-2", "know-3"]);
    }

    #[tokio::test]
    async fn test_feed_sorted_by_composite() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know000001", "a.md", 80.0, "knowledge"),
            "body a",
        )
        .unwrap();
        db.upsert_entity(
            &sample_entity("know000002", "b.md", 60.0, "knowledge"),
            "body b",
        )
        .unwrap();
        drop(db);
        let q = FeedQuery {
            mode: "explore".to_string(),
            limit: 10,
        };
        let r = feed(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].id, "know000001");
    }

    #[tokio::test]
    async fn test_feed_strategic_mode() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-a".to_string(),
                file_path: "a.md".to_string(),
                title: Some("A".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(90.0),
                strategy: Some(30.0),
                consensus: Some(50.0),
                composite: Some(70.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("a".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "body a",
        )
        .unwrap();
        db.upsert_entity(
            &EntityRow {
                id: "know-b".to_string(),
                file_path: "b.md".to_string(),
                title: Some("B".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(30.0),
                strategy: Some(95.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("b".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "body b",
        )
        .unwrap();
        drop(db);
        let q = FeedQuery {
            mode: "strategic".to_string(),
            limit: 10,
        };
        let r = feed(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r[0].id, "know-b", "strategic 按 strategy 降序");
        assert_eq!(r[1].id, "know-a");
    }

    #[tokio::test]
    async fn test_feed_consolidate_mode() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-old".to_string(),
                file_path: "a.md".to_string(),
                title: Some("Old".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(50.0),
                strategy: Some(50.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: Some("2026-01-01T00:00:00Z".to_string()),
                content_hash: Some("a".to_string()),
                updated_at: Some("2026-01-01T00:00:00Z".to_string()),
            },
            "body a",
        )
        .unwrap();
        db.upsert_entity(
            &EntityRow {
                id: "know-recent".to_string(),
                file_path: "b.md".to_string(),
                title: Some("Recent".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(50.0),
                strategy: Some(50.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-08T00:00:00Z".to_string()),
                content_hash: Some("b".to_string()),
                updated_at: Some("2026-07-08T00:00:00Z".to_string()),
            },
            "body b",
        )
        .unwrap();
        drop(db);
        let q = FeedQuery {
            mode: "consolidate".to_string(),
            limit: 10,
        };
        let r = feed(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r[0].id, "know-old", "consolidate 按 last_boosted_at 升序");
        assert_eq!(r[1].id, "know-recent");
    }

    #[tokio::test]
    async fn test_feed_consolidate_null_first() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-null".to_string(),
                file_path: "a.md".to_string(),
                title: Some("Null".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(50.0),
                strategy: Some(50.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: None,
                content_hash: Some("a".to_string()),
                updated_at: None,
            },
            "body a",
        )
        .unwrap();
        db.upsert_entity(
            &EntityRow {
                id: "know-has".to_string(),
                file_path: "b.md".to_string(),
                title: Some("Has".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(50.0),
                strategy: Some(50.0),
                consensus: Some(50.0),
                composite: Some(50.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-01T00:00:00Z".to_string()),
                content_hash: Some("b".to_string()),
                updated_at: Some("2026-07-01T00:00:00Z".to_string()),
            },
            "body b",
        )
        .unwrap();
        drop(db);
        let q = FeedQuery {
            mode: "consolidate".to_string(),
            limit: 10,
        };
        let r = feed(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r[0].id, "know-null", "NULL last_boosted_at 排最前");
        assert_eq!(r[1].id, "know-has");
    }

    #[tokio::test]
    async fn test_feed_explore_default() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-low".to_string(),
                file_path: "a.md".to_string(),
                title: Some("Low".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(30.0),
                strategy: Some(30.0),
                consensus: Some(30.0),
                composite: Some(30.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("a".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "body a",
        )
        .unwrap();
        db.upsert_entity(
            &EntityRow {
                id: "know-high".to_string(),
                file_path: "b.md".to_string(),
                title: Some("High".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(90.0),
                strategy: Some(90.0),
                consensus: Some(90.0),
                composite: Some(90.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("b".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "body b",
        )
        .unwrap();
        drop(db);
        let q = FeedQuery {
            mode: "explore".to_string(),
            limit: 10,
        };
        let r = feed(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r[0].id, "know-high", "explore 按 composite 降序");
        assert_eq!(r[1].id, "know-low");
    }
    #[tokio::test]
    async fn test_entities_top_layer_filter() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know000001", "a.md", 80.0, "knowledge"),
            "body a",
        )
        .unwrap();
        db.upsert_entity(&sample_entity("case000001", "b.md", 90.0, "case"), "body b")
            .unwrap();
        drop(db);
        let q = TopQuery {
            layer: Some("knowledge".to_string()),
            limit: 10,
        };
        let r = entities_top(State(state.clone()), Query(q))
            .await
            .unwrap()
            .0;
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "know000001");
        let q2 = TopQuery {
            layer: None,
            limit: 10,
        };
        let r2 = entities_top(State(state), Query(q2)).await.unwrap().0;
        assert_eq!(r2.len(), 2);
        assert_eq!(r2[0].id, "case000001");
    }

    #[tokio::test]
    async fn test_get_entity_with_refs() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("know000001.md"),
            sample_md("know000001", "T", 50.0, "refs [[know-2]] [[know-3]]"),
        )
        .unwrap();
        let state = setup_state(&vault);
        state
            .db
            .lock()
            .await
            .upsert_entity(
                &sample_entity("know000001", "know000001.md", 50.0, "knowledge"),
                "refs [[know-2]] [[know-3]]",
            )
            .unwrap();
        let r = get_entity(State(state), Path("know000001".to_string()))
            .await
            .unwrap()
            .0;
        assert_eq!(r.refs, vec!["know-2", "know-3"]);
        assert!((r.score.unwrap().composite - 50.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_graph_returns_nodes_and_edges() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let md_a = sample_md("know-a", "A", 80.0, "refs [[know-b]]");
        fs::write(vault.join("know-a.md"), md_a).unwrap();
        let md_b = sample_md("know-b", "B", 60.0, "no refs");
        fs::write(vault.join("know-b.md"), md_b).unwrap();
        let state = setup_state(&vault);
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-a".to_string(),
                file_path: "know-a.md".to_string(),
                title: Some("A".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(80.0),
                strategy: Some(80.0),
                consensus: Some(80.0),
                composite: Some(80.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("a".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "refs [[know-b]]",
        )
        .unwrap();
        db.upsert_entity(
            &EntityRow {
                id: "know-b".to_string(),
                file_path: "know-b.md".to_string(),
                title: Some("B".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(60.0),
                strategy: Some(60.0),
                consensus: Some(60.0),
                composite: Some(60.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("b".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "no refs",
        )
        .unwrap();
        drop(db);
        let result = graph(State(state)).await.unwrap().0;
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(result.edges.len(), 1, "know-a -> know-b 一条边");
        assert_eq!(result.edges[0].source, "know-a");
        assert_eq!(result.edges[0].target, "know-b");
    }

    #[tokio::test]
    async fn test_graph_no_edges_for_orphan_refs() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let md = sample_md("know-a", "A", 80.0, "refs [[know-ghost]]");
        fs::write(vault.join("know-a.md"), md).unwrap();
        let state = setup_state(&vault);
        let db = state.db.lock().await;
        db.upsert_entity(
            &EntityRow {
                id: "know-a".to_string(),
                file_path: "know-a.md".to_string(),
                title: Some("A".to_string()),
                layer: Some("knowledge".to_string()),
                status: Some("active".to_string()),
                interest: Some(80.0),
                strategy: Some(80.0),
                consensus: Some(80.0),
                composite: Some(80.0),
                access_count: 0,
                last_boosted_at: Some("2026-07-09T00:00:00Z".to_string()),
                content_hash: Some("a".to_string()),
                updated_at: Some("2026-07-09T00:00:00Z".to_string()),
            },
            "refs [[know-ghost]]",
        )
        .unwrap();
        drop(db);
        let result = graph(State(state)).await.unwrap().0;
        assert_eq!(result.nodes.len(), 1);
        assert_eq!(result.edges.len(), 0, "指向不存在实体的引用不应产生边");
    }

    #[tokio::test]
    async fn test_graph_empty_vault() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let state = setup_state(&vault);
        let result = graph(State(state)).await.unwrap().0;
        assert!(result.nodes.is_empty());
        assert!(result.edges.is_empty());
    }
    #[tokio::test]
    async fn test_search_hits() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know000001", "a.md", 80.0, "knowledge"),
            "Nash equilibrium game theory",
        )
        .unwrap();
        drop(db);
        let q = SearchQuery {
            q: "Nash".to_string(),
            limit: 10,
        };
        let r = search(State(state), Query(q)).await.unwrap().0;
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "know000001");
        assert_eq!(r[0].layer.as_deref(), Some("knowledge"));
        assert!((r[0].composite.unwrap() - 80.0).abs() < 1e-9);
        assert!(r[0]
            .snippet
            .as_deref()
            .unwrap_or("")
            .to_lowercase()
            .contains("nash"));
    }

    #[tokio::test]
    async fn test_create_entity() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let state = setup_state(&vault);
        let req = CreateEntityRequest {
            title: "New Note".to_string(),
            layer: "knowledge".to_string(),
            content: Some("content".to_string()),
            interest: Some(70.0),
            strategy: Some(80.0),
            consensus: Some(60.0),
        };
        let (status, json) = create_entity(State(state.clone()), Json(req))
            .await
            .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        let id = json["id"].as_str().unwrap();
        assert!(id.starts_with("know"));
        assert!(vault.join("Knowledge").join(format!("{id}.md")).exists());
        assert!(state.db.lock().await.get_entity(id).unwrap().is_some());
    }

    #[tokio::test]
    async fn test_update_score_recalculates() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("know000001.md"),
            sample_md("know000001", "T", 50.0, "body"),
        )
        .unwrap();
        let state = setup_state(&vault);
        state
            .db
            .lock()
            .await
            .upsert_entity(
                &sample_entity("know000001", "know000001.md", 50.0, "knowledge"),
                "body",
            )
            .unwrap();
        let req = ScoreUpdateRequest {
            interest: Some(95.0),
            strategy: None,
            consensus: None,
        };
        let json = update_score(
            State(state.clone()),
            Path("know000001".to_string()),
            Json(req),
        )
        .await
        .unwrap()
        .0;
        let c = json["score"]["composite"].as_f64().unwrap();
        assert!((c - 87.0).abs() < 1e-6, "95*0.4+90*0.35+70*0.25=87");
        let content = fs::read_to_string(vault.join("know000001.md")).unwrap();
        let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
        assert!((frontmatter::get_score(&fm).unwrap().unwrap().interest - 95.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_record_access_study() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("know000001.md"),
            sample_md("know000001", "T", 50.0, "body"),
        )
        .unwrap();
        let state = setup_state(&vault);
        state
            .db
            .lock()
            .await
            .upsert_entity(
                &sample_entity("know000001", "know000001.md", 50.0, "knowledge"),
                "body",
            )
            .unwrap();
        let req = AccessRequest {
            depth: "study".to_string(),
        };
        let json = record_access(
            State(state.clone()),
            Path("know000001".to_string()),
            Json(req),
        )
        .await
        .unwrap()
        .0;
        assert!(
            (json["score"]["interest"].as_f64().unwrap() - 83.0).abs() < 1e-9,
            "80+3=83"
        );
        assert_eq!(
            json["score"]["access_count"].as_f64().unwrap(),
            6.0,
            "5+1=6"
        );
        let content = fs::read_to_string(vault.join("know000001.md")).unwrap();
        let (fm, _) = frontmatter::split_frontmatter(&content).unwrap();
        assert_eq!(
            frontmatter::get_score(&fm).unwrap().unwrap().access_count,
            6
        );
    }

    #[tokio::test]
    async fn test_get_entity_not_found() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let r = get_entity(State(state), Path("nope".to_string())).await;
        assert!(matches!(r.unwrap_err(), AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_record_access_invalid_depth() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(
            vault.join("know000001.md"),
            sample_md("know000001", "T", 50.0, "body"),
        )
        .unwrap();
        let state = setup_state(&vault);
        state
            .db
            .lock()
            .await
            .upsert_entity(
                &sample_entity("know000001", "know000001.md", 50.0, "knowledge"),
                "body",
            )
            .unwrap();
        let req = AccessRequest {
            depth: "invalid".to_string(),
        };
        assert!(
            record_access(State(state), Path("know000001".to_string()), Json(req))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_agent_context() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.db.lock().await;
        db.upsert_entity(
            &sample_entity("know-high", "a.md", 90.0, "knowledge"),
            "Nash equilibrium is a core concept in game theory",
        )
        .unwrap();
        db.upsert_entity(
            &sample_entity("know-low", "b.md", 30.0, "knowledge"),
            "Nash equilibrium is a core concept in game theory",
        )
        .unwrap();
        drop(db);

        let req = AgentContextRequest {
            task: "Nash equilibrium".to_string(),
            top_k: 1,
        };
        let resp = agent_context(State(state), Json(req)).await.unwrap().0;
        assert_eq!(resp.context.len(), 1);
        assert_eq!(resp.context[0].id, "know-high", "应按 composite 加权取最高");
        assert!(resp.context[0]
            .content
            .as_deref()
            .unwrap_or("")
            .contains("Nash"));
        assert!(!resp.reasoning.is_empty());
    }

    #[tokio::test]
    async fn test_agent_context_empty() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let req = AgentContextRequest {
            task: "nonexistent query".to_string(),
            top_k: 5,
        };
        let resp = agent_context(State(state), Json(req)).await.unwrap().0;
        assert!(resp.context.is_empty());
    }
}
