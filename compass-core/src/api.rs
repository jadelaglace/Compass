//! HTTP 路由（T1.6）：对齐 PRD_v3.0 §7 与 skills/compass 7 个 action。
//!
//! 端点：/health /feed /entities/top /entities/{id} /search /entities
//!        /entities/{id}/score /entities/{id}/access /agent/context /graph

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::db::{Db, EntityRow, ScoreHistoryRow, TimelineRow};
use crate::frontmatter::{self, Note};
use crate::models::{Score, Weights};
use crate::scoring::{self, AccessDepth, Trigger};

/// 共享应用状态。
#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub db: Arc<Mutex<Db>>,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/feed", get(feed))
        .route("/entities/top", get(entities_top))
        .route("/entities/search", get(entities_search))
        .route("/entities/:id", get(get_entity))
        .route("/entities", post(create_entity))
        .route("/entities/:id/score", patch(patch_score))
        .route("/entities/:id/access", patch(patch_access))
        .route("/agent/context", post(agent_context))
        .route("/graph", get(graph_data))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "name": "compass",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ---------- /feed ----------

#[derive(Debug, Deserialize)]
struct FeedQuery {
    #[serde(default)]
    mode: FeedMode,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
enum FeedMode {
    #[default]
    Explore,
    Consolidate,
    Strategic,
}

async fn feed(
    State(state): State<Arc<AppState>>,
    Query(q): Query<FeedQuery>,
) -> ApiResult<Json<FeedResponse>> {
    let db = state.db.lock().await;
    let mut items = db.list_entities().map_err(api_err)?;
    drop(db);

    match q.mode {
        FeedMode::Explore => {
            // 默认按 composite 降序
        }
        FeedMode::Strategic => {
            items.sort_by(|a, b| {
                let sa = a.strategy.unwrap_or(0.0);
                let sb = b.strategy.unwrap_or(0.0);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        FeedMode::Consolidate => {
            // 待复习：composite 高但 access_count 低 或 last_boosted_at 较久
            items.sort_by(|a, b| {
                let sa = a.composite.unwrap_or(0.0) / ((a.access_count as f64).max(1.0));
                let sb = b.composite.unwrap_or(0.0) / ((b.access_count as f64).max(1.0));
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    items.truncate(q.limit.max(1).min(100));
    let items = items.into_iter().map(entity_to_summary).collect();
    Ok(Json(FeedResponse { items }))
}

#[derive(Serialize)]
struct FeedResponse {
    items: Vec<EntitySummary>,
}

// ---------- /entities/top ----------

#[derive(Debug, Deserialize)]
struct TopQuery {
    layer: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}

async fn entities_top(
    State(state): State<Arc<AppState>>,
    Query(q): Query<TopQuery>,
) -> ApiResult<Json<Vec<EntitySummary>>> {
    let db = state.db.lock().await;
    let mut items = db.list_entities().map_err(api_err)?;
    drop(db);

    if let Some(layer) = q.layer {
        items.retain(|e| e.layer.as_deref() == Some(&layer));
    }
    items.truncate(q.limit.max(1).min(100));
    Ok(Json(items.into_iter().map(entity_to_summary).collect()))
}

// ---------- /search ----------

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

async fn entities_search(
    State(state): State<Arc<AppState>>,
    Query(q): Query<SearchQuery>,
) -> ApiResult<Json<Vec<SearchHit>>> {
    let db = state.db.lock().await;
    let hits = db
        .fts_search(&q.q, q.limit.max(1).min(100) as u32)
        .map_err(api_err)?;
    drop(db);

    let mut out = Vec::new();
    for h in hits {
        out.push(SearchHit {
            id: h.id,
            title: h.title,
            snippet: h.snippet,
        });
    }
    Ok(Json(out))
}

#[derive(Serialize)]
struct SearchHit {
    id: String,
    title: Option<String>,
    snippet: Option<String>,
}

// ---------- GET /entities/{id} ----------

async fn get_entity(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> ApiResult<Json<EntityDetail>> {
    let db = state.db.lock().await;
    let row = db.get_entity(&id).map_err(api_err)?;
    drop(db);

    let row = row.ok_or(ApiError::NotFound)?;
    let path = state.cfg.vault_path.join(&row.file_path);
    let note = frontmatter::read_note(&path).map_err(|e| {
        warn!(err = %e, "读取笔记失败");
        ApiError::Internal(e.to_string())
    })?;

    let refs = extract_refs(&note.body, &id);
    Ok(Json(EntityDetail {
        id: row.id.clone(),
        title: row.title.clone(),
        layer: row.layer.clone(),
        status: row.status.clone(),
        score: score_from_row(&row),
        body: note.body,
        refs,
        file_path: row.file_path.clone(),
    }))
}

// ---------- POST /entities ----------

#[derive(Debug, Deserialize)]
struct CreateEntityReq {
    title: String,
    #[serde(default)]
    layer: String,
    #[serde(default)]
    content: String,
}

#[derive(Serialize)]
struct CreateEntityResp {
    id: String,
    file_path: String,
    title: String,
}

async fn create_entity(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateEntityReq>,
) -> ApiResult<(StatusCode, Json<CreateEntityResp>)> {
    let layer = if req.layer.is_empty() {
        "knowledge".to_string()
    } else {
        req.layer
    };

    // 生成 id：know- + 6位递增数字（基于当前 vault 中 know- 数量）
    let id = generate_id(&state.cfg.vault_path, &layer).map_err(api_err)?;
    let file_name = format!("{id}.md");
    let file_path = state.cfg.vault_path.join(&file_name);

    // 确保父目录存在
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(api_err)?;
    }

    let now = chrono::Utc::now().to_rfc3339();
    let weights = state.cfg.weights;
    let composite = scoring::composite(5.0, 5.0, 5.0, &weights);
    let score = Score {
        interest: 5.0,
        strategy: 5.0,
        consensus: 5.0,
        composite,
        weights: Some(weights),
        updated_at: now.clone(),
        last_boosted_at: now.clone(),
        access_count: 0,
    };

    let frontmatter = build_frontmatter(&id, &req.title, &layer, &score);
    let content = format!("---\n{frontmatter}---\n\n{}", req.content);
    tokio::fs::write(&file_path, content)
        .await
        .map_err(api_err)?;

    // 索引到 SQLite
    let note = frontmatter::read_note(&file_path).map_err(api_err)?;
    let row = entity_row_from_note(&id, &file_name, &note, &score).map_err(api_err)?;
    let db = state.db.lock().await;
    db.upsert_entity(&row, &note.body).map_err(api_err)?;
    drop(db);

    info!(id = %id, path = %file_path.display(), "创建实体");
    Ok((
        StatusCode::CREATED,
        Json(CreateEntityResp {
            id,
            file_path: file_name,
            title: req.title,
        }),
    ))
}

// ---------- PATCH /entities/{id}/score ----------

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum PatchScoreReq {
    Single {
        dimension: String,
        value: f64,
        #[serde(default)]
        reason: String,
    },
    Batch {
        adjustments: Vec<ScoreAdjustment>,
        #[serde(default)]
        reason: String,
    },
}

#[derive(Debug, Deserialize)]
struct ScoreAdjustment {
    dimension: String,
    delta: f64,
}

async fn patch_score(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<PatchScoreReq>,
) -> ApiResult<Json<EntitySummary>> {
    let (path, mut score, mut row) = load_entity(&state, &id).await?;
    let weights = score.weights.unwrap_or(state.cfg.weights);
    let now = chrono::Utc::now().to_rfc3339();

    match req {
        PatchScoreReq::Single {
            dimension,
            value,
            reason,
        } => {
            let old = set_dimension(&mut score, &dimension, value)?;
            record_score_history(&state, &id, &dimension, old, value, &reason, "manual", &now)
                .await?;
        }
        PatchScoreReq::Batch {
            adjustments,
            reason,
        } => {
            for adj in adjustments {
                let old = score_for_dim(&score, &adj.dimension);
                let new = (old + adj.delta).clamp(0.0, 100.0);
                set_dimension(&mut score, &adj.dimension, new)?;
                record_score_history(
                    &state,
                    &id,
                    &adj.dimension,
                    old,
                    new,
                    &reason,
                    "manual",
                    &now,
                )
                .await?;
            }
        }
    }

    score.composite = scoring::composite(score.interest, score.strategy, score.consensus, &weights);
    score.updated_at = now.clone();
    score.weights = Some(weights);

    save_entity(&state, &path, &id, &score, &mut row).await?;
    Ok(Json(entity_summary_from_row(&row)))
}

// ---------- PATCH /entities/{id}/access ----------

#[derive(Debug, Deserialize)]
struct AccessReq {
    #[serde(default)]
    depth: AccessDepth,
}

async fn patch_access(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
    Json(req): Json<AccessReq>,
) -> ApiResult<Json<EntitySummary>> {
    let (path, score, mut row) = load_entity(&state, &id).await?;
    let now = chrono::Utc::now().to_rfc3339();

    let new_score = scoring::apply_access(&score, req.depth, &now);
    let old_composite = score.composite;

    // 记录 timeline
    let timeline = TimelineRow {
        entity_id: id.clone(),
        event_type: "access".to_string(),
        intensity: Some(access_intensity(&req.depth)),
        source: Some("api".to_string()),
        created_at: now.clone(),
    };
    let db = state.db.lock().await;
    db.insert_timeline(&timeline).map_err(api_err)?;
    drop(db);

    // 记录 score_history（composite 变化）
    if (new_score.composite - old_composite).abs() > 1e-9 {
        record_score_history(
            &state,
            &id,
            "composite",
            old_composite,
            new_score.composite,
            "access boost",
            "access_boost",
            &now,
        )
        .await?;
    }

    save_entity(&state, &path, &id, &new_score, &mut row).await?;
    Ok(Json(entity_summary_from_row(&row)))
}

// ---------- POST /agent/context ----------

#[derive(Debug, Deserialize)]
struct AgentContextReq {
    task: String,
    #[serde(default = "default_top_k")]
    top_k: usize,
}

#[derive(Serialize)]
struct AgentContextResp {
    context: Vec<EntitySummary>,
    suggested_entities: Vec<String>,
    reasoning: String,
}

async fn agent_context(
    State(state): State<Arc<AppState>>,
    Json(req): Json<AgentContextReq>,
) -> ApiResult<Json<AgentContextResp>> {
    let db = state.db.lock().await;
    let hits = db
        .fts_search(&req.task, (req.top_k * 2) as u32)
        .map_err(api_err)?;
    drop(db);

    let mut scored: Vec<(EntityRow, f64)> = Vec::new();
    for h in hits {
        let db = state.db.lock().await;
        if let Some(row) = db.get_entity(&h.id).map_err(api_err)? {
            let composite = row.composite.unwrap_or(0.0);
            scored.push((row, composite));
        }
        drop(db);
    }

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let top_k = req.top_k.max(1).min(20);
    let context: Vec<EntitySummary> = scored
        .iter()
        .take(top_k)
        .map(|(r, _)| entity_summary_from_row(r))
        .collect();
    let suggested: Vec<String> = scored
        .iter()
        .skip(top_k)
        .take(top_k)
        .map(|(r, _)| r.id.clone())
        .collect();

    let reasoning = if let Some(first) = context.first() {
        format!(
            "基于 FTS 搜索 '{}' 找到 {} 个候选，按 composite 取 top {}",
            req.task,
            scored.len(),
            top_k
        )
    } else {
        format!("未找到与 '{}' 相关的实体", req.task)
    };

    Ok(Json(AgentContextResp {
        context,
        suggested_entities: suggested,
        reasoning,
    }))
}

// ---------- GET /graph ----------

#[derive(Serialize)]
struct GraphData {
    nodes: Vec<GraphNode>,
    edges: Vec<GraphEdge>,
}

#[derive(Serialize)]
struct GraphNode {
    id: String,
    label: Option<String>,
    layer: Option<String>,
    score: f64,
}

#[derive(Serialize)]
struct GraphEdge {
    source: String,
    target: String,
}

async fn graph_data(State(state): State<Arc<AppState>>) -> ApiResult<Json<GraphData>> {
    let db = state.db.lock().await;
    let rows = db.list_entities().map_err(api_err)?;
    drop(db);

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut bodies: HashMap<String, String> = HashMap::new();

    for row in &rows {
        let path = state.cfg.vault_path.join(&row.file_path);
        if let Ok(note) = frontmatter::read_note(&path) {
            bodies.insert(row.id.clone(), note.body.clone());
            let refs = extract_refs(&note.body, &row.id);
            for target in refs {
                edges.push(GraphEdge {
                    source: row.id.clone(),
                    target,
                });
            }
        }
        nodes.push(GraphNode {
            id: row.id.clone(),
            label: row.title.clone(),
            layer: row.layer.clone(),
            score: row.composite.unwrap_or(0.0),
        });
    }

    Ok(Json(GraphData { nodes, edges }))
}

// ---------- 共享类型与工具 ----------

#[derive(Serialize)]
struct EntitySummary {
    id: String,
    title: Option<String>,
    layer: Option<String>,
    status: Option<String>,
    score: Option<ScoreView>,
    file_path: String,
}

#[derive(Serialize)]
struct ScoreView {
    interest: f64,
    strategy: f64,
    consensus: f64,
    composite: f64,
    access_count: i64,
}

#[derive(Serialize)]
struct EntityDetail {
    id: String,
    title: Option<String>,
    layer: Option<String>,
    status: Option<String>,
    score: Option<ScoreView>,
    body: String,
    refs: Vec<String>,
    file_path: String,
}

type ApiResult<T> = Result<T, ApiError>;

enum ApiError {
    NotFound,
    BadRequest(String),
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "entity not found".to_string()),
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

fn api_err(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

fn default_limit() -> usize {
    20
}

fn default_top_k() -> usize {
    5
}

fn entity_to_summary(row: EntityRow) -> EntitySummary {
    entity_summary_from_row(&row)
}

fn entity_summary_from_row(row: &EntityRow) -> EntitySummary {
    EntitySummary {
        id: row.id.clone(),
        title: row.title.clone(),
        layer: row.layer.clone(),
        status: row.status.clone(),
        score: score_from_row(row),
        file_path: row.file_path.clone(),
    }
}

fn score_from_row(row: &EntityRow) -> Option<ScoreView> {
    Some(ScoreView {
        interest: row.interest?,
        strategy: row.strategy?,
        consensus: row.consensus?,
        composite: row.composite?,
        access_count: row.access_count,
    })
}

fn score_for_dim(score: &Score, dim: &str) -> f64 {
    match dim {
        "interest" => score.interest,
        "strategy" => score.strategy,
        "consensus" => score.consensus,
        _ => 0.0,
    }
}

fn set_dimension(score: &mut Score, dim: &str, value: f64) -> Result<f64, ApiError> {
    let old = score_for_dim(score, dim);
    match dim {
        "interest" => score.interest = value.clamp(0.0, 100.0),
        "strategy" => score.strategy = value.clamp(0.0, 100.0),
        "consensus" => score.consensus = value.clamp(0.0, 100.0),
        _ => return Err(ApiError::BadRequest(format!("未知维度 {dim}"))),
    }
    Ok(old)
}

fn extract_refs(body: &str, current_id: &str) -> Vec<String> {
    let re = Regex::new(r"\[\[([a-zA-Z0-9_-]+)\]\]").unwrap();
    let mut refs: Vec<String> = re
        .captures_iter(body)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .filter(|id| id != current_id)
        .collect();
    refs.sort();
    refs.dedup();
    refs
}

fn build_frontmatter(id: &str, title: &str, layer: &str, score: &Score) -> String {
    let yaml = serde_yaml::to_string(score).unwrap_or_default();
    let score_block: String = yaml
        .lines()
        .map(|l| format!("  {l}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "id: {id}\ntitle: {title}\nlayer: {layer}\nstatus: active\nscore:\n{score_block}\ncreated_at: {now}\nupdated_at: {now}\n",
        now = score.updated_at
    )
}

fn entity_row_from_note(
    id: &str,
    file_path: &str,
    note: &Note,
    score: &Score,
) -> Result<EntityRow> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    note.body.hash(&mut h);
    let content_hash = format!("{:016x}", h.finish());

    Ok(EntityRow {
        id: id.to_string(),
        file_path: file_path.to_string(),
        title: extract_yaml_str(&note.frontmatter, "title"),
        layer: extract_yaml_str(&note.frontmatter, "layer"),
        status: extract_yaml_str(&note.frontmatter, "status"),
        interest: Some(score.interest),
        strategy: Some(score.strategy),
        consensus: Some(score.consensus),
        composite: Some(score.composite),
        access_count: score.access_count,
        last_boosted_at: Some(score.last_boosted_at.clone()),
        content_hash: Some(content_hash),
        updated_at: Some(score.updated_at.clone()),
    })
}

fn extract_yaml_str(frontmatter: &str, key: &str) -> Option<String> {
    let fm: serde_yaml::Value = serde_yaml::from_str(frontmatter).ok()?;
    let m = fm.as_mapping()?;
    m.get(&serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn generate_id(vault: &Path, layer: &str) -> Result<String> {
    let prefix = match layer {
        "direction" => "dir",
        "knowledge" => "know",
        "case" => "case",
        "log" => "log",
        "insight" => "ins",
        _ => "know",
    };
    let mut max = 0u32;
    for entry in std::fs::read_dir(vault)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some(stem) = name
            .strip_prefix(prefix)
            .and_then(|s| s.strip_suffix(".md"))
        {
            if let Ok(n) = stem.parse::<u32>() {
                max = max.max(n);
            }
        }
    }
    Ok(format!("{}{:06}", prefix, max + 1))
}

async fn load_entity(state: &AppState, id: &str) -> Result<(PathBuf, Score, EntityRow), ApiError> {
    let db = state.db.lock().await;
    let row = db
        .get_entity(id)
        .map_err(api_err)?
        .ok_or(ApiError::NotFound)?;
    drop(db);

    let path = state.cfg.vault_path.join(&row.file_path);
    let note = frontmatter::read_note(&path).map_err(api_err)?;
    let score = frontmatter::get_score(&note.frontmatter)
        .map_err(api_err)?
        .ok_or(ApiError::Internal("笔记缺少 score 块".to_string()))?;
    Ok((path, score, row))
}

async fn save_entity(
    state: &AppState,
    path: &Path,
    id: &str,
    score: &Score,
    row: &mut EntityRow,
) -> Result<(), ApiError> {
    frontmatter::write_score(path, score).map_err(api_err)?;

    let note = frontmatter::read_note(path).map_err(api_err)?;
    *row = entity_row_from_note(id, &row.file_path, &note, score).map_err(api_err)?;

    let db = state.db.lock().await;
    db.upsert_entity(row, &note.body).map_err(api_err)?;
    drop(db);
    Ok(())
}

async fn record_score_history(
    state: &AppState,
    entity_id: &str,
    dimension: &str,
    old: f64,
    new: f64,
    reason: &str,
    trigger: &str,
    now: &str,
) -> Result<(), ApiError> {
    let h = ScoreHistoryRow {
        entity_id: entity_id.to_string(),
        dimension: Some(dimension.to_string()),
        old: Some(old),
        new: Some(new),
        reason: Some(reason.to_string()),
        trigger: Some(trigger.to_string()),
        created_at: now.to_string(),
    };
    let db = state.db.lock().await;
    db.insert_score_history(&h).map_err(api_err)?;
    drop(db);
    Ok(())
}

fn access_intensity(depth: &AccessDepth) -> f64 {
    match depth {
        AccessDepth::Glance => 0.1,
        AccessDepth::Read => 1.0,
        AccessDepth::Study => 3.0,
        AccessDepth::Apply => 5.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_refs() {
        let body = "涉及 [[know-000001]] 和 [[case-000001]]，还有 [[know-000001]] 重复。";
        let refs = extract_refs(body, "know-000002");
        assert_eq!(refs, vec!["case-000001", "know-000001"]);
    }

    #[test]
    fn test_extract_refs_filters_self() {
        let body = "[[know-000001]] 自引用";
        let refs = extract_refs(body, "know-000001");
        assert!(refs.is_empty());
    }
}
