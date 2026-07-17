//! HTTP 路由（T1.6）：PRD §7 的 7 个端点 + /health。
//!
//! 字段统一（#160）：响应用 `id`/`composite`（PRD v3.0），非 v2.x 的 `entity_id`/`final_score`。
//! 写回端点（score/access/create）委托应用服务执行。

use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::{header::AUTHORIZATION, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;

use crate::application::ports::{RepositoryHandle, VaultPort};
use crate::config::Config;
use crate::domain::entity::Weights;

/// 共享应用状态
#[derive(Clone)]
pub(crate) struct AppState {
    pub repository: RepositoryHandle,
    pub vault: Arc<dyn VaultPort>,
    pub weights: Weights,
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

#[derive(Debug, Deserialize)]
pub struct RelatedQuery {
    #[serde(default = "default_related_limit")]
    pub limit: u32,
}

#[derive(Debug, Default, Deserialize)]
pub struct WeeklyReportQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub tz: Option<String>,
}

fn default_related_limit() -> u32 {
    10
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
        .route("/entities/:id/related", get(related_entities))
        .route(
            "/related-suggestions/:suggestion_id/accept",
            post(accept_related_suggestion),
        )
        .route(
            "/related-suggestions/:suggestion_id/reject",
            post(reject_related_suggestion),
        )
        .route("/reports/weekly", get(weekly_report))
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

// Transport adapts HTTP DTOs to application inputs and serializes application
// read models. The use-case orchestration lives in application services.
fn query_service(state: &AppState) -> crate::application::query_service::QueryService {
    crate::application::query_service::QueryService::new(
        state.repository.clone(),
        state.vault.clone(),
        state.weights,
        Utc::now(),
    )
}

fn entity_service(state: &AppState) -> crate::application::entity_service::EntityService {
    crate::application::entity_service::EntityService::new(
        state.repository.clone(),
        state.vault.clone(),
        state.weights,
    )
}

fn suggestion_service(
    state: &AppState,
) -> crate::application::suggestion_service::SuggestionService {
    crate::application::suggestion_service::SuggestionService::new(
        state.repository.clone(),
        state.vault.clone(),
        state.weights,
    )
}

pub(crate) async fn weekly_report(
    State(state): State<AppState>,
    Query(query): Query<WeeklyReportQuery>,
) -> Result<Json<crate::application::query_service::WeeklyReport>, AppError> {
    Ok(Json(
        query_service(&state)
            .weekly_report(
                query.from.as_deref(),
                query.to.as_deref(),
                query.tz.as_deref(),
            )
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn feed(
    State(state): State<AppState>,
    Query(query): Query<FeedQuery>,
) -> Result<Json<Vec<crate::application::query_service::EntitySummary>>, AppError> {
    Ok(Json(
        query_service(&state)
            .feed(&query.mode, query.limit)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn entities_top(
    State(state): State<AppState>,
    Query(query): Query<TopQuery>,
) -> Result<Json<Vec<crate::application::query_service::EntitySummary>>, AppError> {
    Ok(Json(
        query_service(&state)
            .top(query.layer.as_deref(), query.limit)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn get_entity(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<crate::application::query_service::EntityDetail>, AppError> {
    Ok(Json(
        query_service(&state)
            .entity(&id)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn search(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<crate::application::query_service::SearchHit>>, AppError> {
    Ok(Json(
        query_service(&state)
            .search(&query.q, query.limit)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn tag_suggestions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<TagSuggestionsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let candidates = request
        .candidates
        .into_iter()
        .map(
            |candidate| crate::application::suggestion_service::TagCandidate {
                tag: candidate.tag,
                confidence: candidate.confidence,
                reason: candidate.reason,
                source: candidate.source,
                algorithm_version: candidate.algorithm_version,
                content_hash: candidate.content_hash,
            },
        )
        .collect();
    Ok(Json(
        suggestion_service(&state)
            .tag_suggestions(&id, candidates)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn accept_tag_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        suggestion_service(&state)
            .accept_tag(&suggestion_id)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn reject_tag_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        suggestion_service(&state)
            .reject_tag(&suggestion_id)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn related_entities(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<RelatedQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        suggestion_service(&state)
            .related(&id, query.limit)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn accept_related_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        suggestion_service(&state)
            .accept_related(&suggestion_id)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn reject_related_suggestion(
    State(state): State<AppState>,
    Path(suggestion_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        suggestion_service(&state)
            .reject_related(&suggestion_id)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn create_entity(
    State(state): State<AppState>,
    Json(request): Json<CreateEntityRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), AppError> {
    let response = entity_service(&state)
        .create(crate::application::entity_service::CreateEntity {
            title: request.title,
            layer: request.layer,
            content: request.content,
            interest: request.interest,
            strategy: request.strategy,
            consensus: request.consensus,
        })
        .await
        .map_err(map_application_error)?;
    Ok((StatusCode::CREATED, Json(response)))
}

pub(crate) async fn update_score(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<ScoreUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        entity_service(&state)
            .update_score(
                &id,
                crate::application::entity_service::ScoreUpdate {
                    interest: request.interest,
                    strategy: request.strategy,
                    consensus: request.consensus,
                },
            )
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn record_access(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<AccessRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    Ok(Json(
        entity_service(&state)
            .record_access(&id, &request.depth)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn agent_context(
    State(state): State<AppState>,
    Json(request): Json<AgentContextRequest>,
) -> Result<Json<crate::application::query_service::AgentContextResponse>, AppError> {
    Ok(Json(
        query_service(&state)
            .agent_context(&request.task, request.top_k)
            .await
            .map_err(map_application_error)?,
    ))
}

pub(crate) async fn graph(
    State(state): State<AppState>,
) -> Result<Json<crate::application::query_service::GraphData>, AppError> {
    Ok(Json(
        query_service(&state)
            .graph()
            .await
            .map_err(map_application_error)?,
    ))
}

/// Transport-only mapping for the established HTTP error response contract.
#[derive(Debug)]
pub(crate) enum AppError {
    NotFound(String),
    BadRequest(String),
    Unprocessable(String),
    Conflict { code: String, message: String },
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound(id) => (StatusCode::NOT_FOUND, format!("entity not found: {id}")),
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message),
            Self::Unprocessable(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            Self::Conflict { code, message } => {
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
            Self::Internal(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

fn map_application_error(error: crate::application::error::AppError) -> AppError {
    match error {
        crate::application::error::AppError::NotFound(id) => AppError::NotFound(id),
        crate::application::error::AppError::BadRequest(message) => AppError::BadRequest(message),
        crate::application::error::AppError::Unprocessable(message) => {
            AppError::Unprocessable(message)
        }
        crate::application::error::AppError::Conflict { code, message } => {
            AppError::Conflict { code, message }
        }
        crate::application::error::AppError::Internal(message) => AppError::Internal(message),
    }
}

pub(crate) fn router_from_config(
    cfg: Arc<Config>,
    repository: RepositoryHandle,
    vault: Arc<crate::infrastructure::vault_adapter::VaultAdapter>,
) -> Router {
    let state = AppState {
        repository,
        vault,
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
    use crate::application::ports::{
        CachedSuggestion, IndexedEntity, ScoreHistoryEntry, TimelineEntry,
    };
    use crate::infrastructure::sqlite_repository::SqliteRepository;
    use crate::infrastructure::vault_adapter as frontmatter;
    use axum::body::Body;
    use axum::http::Request;
    use std::fs;
    use tempfile::tempdir;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    fn setup_state(vault: &std::path::Path) -> AppState {
        AppState {
            repository: Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap())),
            vault: Arc::new(frontmatter::VaultAdapter::new(vault.to_path_buf())),
            weights: Weights::default(),
        }
    }

    fn sample_md(id: &str, title: &str, composite: f64, body: &str) -> String {
        format!(
            "---\nid: {id}\ntitle: {title}\nlayer: knowledge\nstatus: active\nscore:\n  interest: 80.0\n  strategy: 90.0\n  consensus: 70.0\n  composite: {composite}\n  updated_at: '2026-07-09T00:00:00Z'\n  last_boosted_at: '2026-07-09T00:00:00Z'\n  access_count: 5\n---\n{body}\n"
        )
    }

    fn sample_entity(id: &str, fp: &str, composite: f64, layer: &str) -> IndexedEntity {
        IndexedEntity {
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
        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let app = apply_security(
            router_from_config(
                Arc::clone(&cfg),
                db,
                Arc::new(frontmatter::VaultAdapter::new(cfg.vault_path.clone())),
            )
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
        let db = Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap()));
        let app = apply_security(
            router_from_config(
                cfg.clone(),
                db,
                Arc::new(frontmatter::VaultAdapter::new(cfg.vault_path.clone())),
            ),
            &cfg,
        );
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &sample_entity("know-tag", "note.md", 70.0, "knowledge"),
            "Rust and SQLite make a durable local index.",
        )
        .unwrap();
        drop(db);

        let created = tag_suggestions(
            State(state.clone()),
            Path("know-tag".to_string()),
            Json(TagSuggestionsRequest::default()),
        )
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &sample_entity("know-stale", "note.md", 70.0, "knowledge"),
            "Rust body.",
        )
        .unwrap();
        drop(db);

        let created = tag_suggestions(
            State(state.clone()),
            Path("know-stale".to_string()),
            Json(TagSuggestionsRequest::default()),
        )
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
            .repository
            .lock()
            .await
            .get_suggestion(&suggestion_id)
            .unwrap()
            .unwrap();
        assert_eq!(row.status, "expired");
    }

    #[tokio::test]
    async fn test_related_recommendations_are_explainable_and_filter_existing_links() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let notes = [
            (
                "know-source",
                "Source",
                "active",
                "Rust SQLite architecture",
                vec!["Rust"],
                vec!["know-direct"],
            ),
            (
                "know-related",
                "Related",
                "active",
                "Rust SQLite indexing",
                vec!["Rust", "Index"],
                Vec::new(),
            ),
            (
                "know-direct",
                "Already linked",
                "active",
                "Rust SQLite direct",
                vec!["Rust"],
                Vec::new(),
            ),
            (
                "know-archived",
                "Archived",
                "archived",
                "Rust SQLite archived",
                vec!["Rust"],
                Vec::new(),
            ),
        ];
        let db = state.repository.lock().await;
        for (id, title, status, body, tags, links) in &notes {
            let file_name = format!("{id}.md");
            let mut note = sample_md(id, title, 70.0, body);
            note = note.replace("status: active", &format!("status: {status}"));
            note = note.replace(
                "score:\n",
                &format!(
                    "tags:\n{}\nscore:\n",
                    tags.iter()
                        .map(|tag| format!("  - {tag}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                ),
            );
            fs::write(dir.path().join(&file_name), note).unwrap();
            let mut entity = sample_entity(id, &file_name, 70.0, "knowledge");
            entity.title = Some((*title).to_string());
            entity.status = Some((*status).to_string());
            let tags = tags
                .iter()
                .map(|tag| (*tag).to_string())
                .collect::<Vec<_>>();
            let links = links
                .iter()
                .map(|link| (*link).to_string())
                .collect::<Vec<_>>();
            db.upsert_indexed_entity_with_relationships(&entity, body, &tags, &links)
                .unwrap();
        }
        drop(db);

        let response = related_entities(
            State(state),
            Path("know-source".to_string()),
            Query(RelatedQuery { limit: 10 }),
        )
        .await
        .unwrap()
        .0;
        let suggestions = response["suggestions"].as_array().unwrap();
        let ids = suggestions
            .iter()
            .map(|suggestion| suggestion["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert!(ids.contains(&"know-related"));
        assert!(!ids.contains(&"know-source"));
        assert!(!ids.contains(&"know-direct"));
        assert!(!ids.contains(&"know-archived"));
        let related = suggestions
            .iter()
            .find(|suggestion| suggestion["id"] == "know-related")
            .unwrap();
        assert!(related["score"].as_f64().unwrap() > 0.0);
        assert!(!related["reasons"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_related_accept_reject_and_stale_are_idempotent() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let source_path = dir.path().join("source.md");
        let target_path = dir.path().join("target.md");
        let reject_path = dir.path().join("reject.md");
        fs::write(
            &source_path,
            sample_md("know-source", "Source", 70.0, "Rust SQLite architecture"),
        )
        .unwrap();
        fs::write(
            &target_path,
            sample_md("know-target", "Target", 60.0, "Rust SQLite indexing"),
        )
        .unwrap();
        fs::write(
            &reject_path,
            sample_md("know-reject", "Reject", 50.0, "Rust SQLite review"),
        )
        .unwrap();
        let db = state.repository.lock().await;
        for (id, path, title, body, score) in [
            (
                "know-source",
                "source.md",
                "Source",
                "Rust SQLite architecture",
                70.0,
            ),
            (
                "know-target",
                "target.md",
                "Target",
                "Rust SQLite indexing",
                60.0,
            ),
            (
                "know-reject",
                "reject.md",
                "Reject",
                "Rust SQLite review",
                50.0,
            ),
        ] {
            let mut entity = sample_entity(id, path, score, "knowledge");
            entity.title = Some(title.to_string());
            db.upsert_indexed_entity_with_relationships(&entity, body, &["Rust".to_string()], &[])
                .unwrap();
        }
        drop(db);

        let response = related_entities(
            State(state.clone()),
            Path("know-source".to_string()),
            Query(RelatedQuery { limit: 10 }),
        )
        .await
        .unwrap()
        .0;
        let suggestions = response["suggestions"].as_array().unwrap();
        let target_id = suggestions
            .iter()
            .find(|suggestion| suggestion["id"] == "know-target")
            .unwrap()["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();
        let reject_id = suggestions
            .iter()
            .find(|suggestion| suggestion["id"] == "know-reject")
            .unwrap()["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();

        let accepted = accept_related_suggestion(State(state.clone()), Path(target_id.clone()))
            .await
            .unwrap()
            .0;
        assert_eq!(accepted["status"], "accepted");
        let after_accept = fs::read_to_string(&source_path).unwrap();
        assert!(after_accept.contains("[[know-target]]"));
        assert_eq!(
            state
                .repository
                .lock()
                .await
                .entity_links("know-source")
                .unwrap(),
            vec!["know-target"]
        );

        let repeated = accept_related_suggestion(State(state.clone()), Path(target_id))
            .await
            .unwrap()
            .0;
        assert_eq!(repeated["status"], "accepted");
        assert_eq!(fs::read_to_string(&source_path).unwrap(), after_accept);

        let before_reject = fs::read_to_string(&source_path).unwrap();
        let rejected = reject_related_suggestion(State(state.clone()), Path(reject_id))
            .await
            .unwrap()
            .0;
        assert_eq!(rejected["status"], "rejected");
        assert_eq!(fs::read_to_string(&source_path).unwrap(), before_reject);

        let stale_response = related_entities(
            State(state.clone()),
            Path("know-source".to_string()),
            Query(RelatedQuery { limit: 10 }),
        )
        .await
        .unwrap()
        .0;
        let stale_id = stale_response["suggestions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|suggestion| suggestion["id"] == "know-reject")
            .unwrap()["suggestion_id"]
            .as_str()
            .unwrap()
            .to_string();
        fs::write(
            &source_path,
            format!("{}\nchanged", fs::read_to_string(&source_path).unwrap()),
        )
        .unwrap();
        let error = accept_related_suggestion(State(state.clone()), Path(stale_id.clone()))
            .await
            .unwrap_err();
        assert!(matches!(error, AppError::Conflict { .. }));
        assert_eq!(
            state
                .repository
                .lock()
                .await
                .get_suggestion(&stale_id)
                .unwrap()
                .unwrap()
                .status,
            "expired"
        );
    }

    #[tokio::test]
    async fn test_feed_sorted_by_composite() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &sample_entity("know000001", "a.md", 80.0, "knowledge"),
            "body a",
        )
        .unwrap();
        db.upsert_indexed_entity(
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        db.upsert_indexed_entity(
            &IndexedEntity {
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
    async fn test_feed_rejects_unknown_mode() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let error = feed(
            State(state),
            Query(FeedQuery {
                mode: "invalid".to_string(),
                limit: 10,
            }),
        )
        .await
        .unwrap_err();

        assert!(matches!(error, AppError::Unprocessable(_)));
    }

    #[tokio::test]
    async fn test_entities_top_layer_filter() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &sample_entity("know000001", "a.md", 80.0, "knowledge"),
            "body a",
        )
        .unwrap();
        db.upsert_indexed_entity(&sample_entity("case000001", "b.md", 90.0, "case"), "body b")
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
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &IndexedEntity {
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
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
        assert!(state
            .repository
            .lock()
            .await
            .get_entity(id)
            .unwrap()
            .is_some());
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
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
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
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
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
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
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
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(
            &sample_entity("know-high", "a.md", 90.0, "knowledge"),
            "Nash equilibrium is a core concept in game theory",
        )
        .unwrap();
        db.upsert_indexed_entity(
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

    #[tokio::test]
    async fn test_weekly_report_aggregates_history_and_is_deterministic() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let db = state.repository.lock().await;
        db.upsert_indexed_entity(&sample_entity("know-up", "up.md", 80.0, "knowledge"), "up")
            .unwrap();
        db.upsert_indexed_entity(
            &sample_entity("know-down", "down.md", 60.0, "knowledge"),
            "down",
        )
        .unwrap();
        db.record_score_history(&ScoreHistoryEntry {
            entity_id: "know-up".into(),
            dimension: Some("manual".into()),
            old: Some(10.0),
            new: Some(15.0),
            reason: None,
            trigger: Some("ManualMark".into()),
            created_at: "2026-07-06T01:00:00Z".into(),
        })
        .unwrap();
        db.record_score_history(&ScoreHistoryEntry {
            entity_id: "know-up".into(),
            dimension: Some("manual".into()),
            old: Some(15.0),
            new: Some(12.0),
            reason: None,
            trigger: Some("ManualMark".into()),
            created_at: "2026-07-07T01:00:00Z".into(),
        })
        .unwrap();
        db.record_score_history(&ScoreHistoryEntry {
            entity_id: "know-down".into(),
            dimension: Some("interest".into()),
            old: Some(50.0),
            new: Some(40.0),
            reason: None,
            trigger: Some("Decay".into()),
            created_at: "2026-07-08T01:00:00Z".into(),
        })
        .unwrap();
        for (intensity, created_at) in [
            (0.0, "2026-07-06T02:00:00Z"),
            (1.0, "2026-07-06T03:00:00Z"),
            (3.0, "2026-07-06T04:00:00Z"),
            (5.0, "2026-07-06T05:00:00Z"),
        ] {
            db.record_timeline(&TimelineEntry {
                entity_id: "know-up".into(),
                event_type: "access".into(),
                intensity: Some(intensity),
                source: Some("test".into()),
                created_at: created_at.into(),
            })
            .unwrap();
        }
        db.record_timeline(&TimelineEntry {
            entity_id: "know-up".into(),
            event_type: "create".into(),
            intensity: None,
            source: Some("test".into()),
            created_at: "2026-07-06T06:00:00Z".into(),
        })
        .unwrap();
        for (id, status) in [
            ("s-accepted", "accepted"),
            ("s-rejected", "rejected"),
            ("s-expired", "expired"),
        ] {
            db.upsert_suggestion(&CachedSuggestion {
                suggestion_id: id.into(),
                kind: "tag".into(),
                entity_id: "know-up".into(),
                candidate: id.into(),
                candidate_key: id.into(),
                confidence: Some(0.5),
                reason: "test".into(),
                source: "test".into(),
                algorithm_version: "test-v1".into(),
                content_hash: "a".repeat(64),
                status: status.into(),
                created_at: "2026-07-01T00:00:00Z".into(),
                updated_at: "2026-07-09T01:00:00Z".into(),
            })
            .unwrap();
        }
        drop(db);

        let query = WeeklyReportQuery {
            from: Some("2026-07-06".into()),
            to: Some("2026-07-13".into()),
            tz: Some("Asia/Shanghai".into()),
        };
        let first = weekly_report(State(state.clone()), Query(query))
            .await
            .unwrap()
            .0;
        let second = weekly_report(
            State(state),
            Query(WeeklyReportQuery {
                from: Some("2026-07-06".into()),
                to: Some("2026-07-13".into()),
                tz: Some("Asia/Shanghai".into()),
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(first, second);
        assert_eq!(first.score_increases[0].entity_id, "know-up");
        assert!((first.score_increases[0].delta - 2.0).abs() < 1e-9);
        assert_eq!(first.score_decreases[0].entity_id, "know-down");
        assert_eq!(first.access_count, 4);
        assert_eq!(first.review_count, 2);
        assert_eq!(first.access_stats.glance, 1);
        assert_eq!(first.access_stats.read, 1);
        assert_eq!(first.new_entities, vec!["know-up"]);
        assert_eq!(first.suggestion_stats.accepted, 1);
        assert_eq!(first.suggestion_stats.rejected, 1);
        assert_eq!(first.suggestion_stats.expired, 1);
        assert_eq!(first.generated_at, "2026-07-13T00:00:00Z");
    }

    #[tokio::test]
    async fn test_weekly_report_rejects_missing_or_invalid_timezone() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let error = weekly_report(
            State(state.clone()),
            Query(WeeklyReportQuery {
                from: Some("2026-07-06".into()),
                to: Some("2026-07-13".into()),
                tz: Some("Not/AZone".into()),
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::Unprocessable(_)));

        let error = weekly_report(
            State(state),
            Query(WeeklyReportQuery {
                from: Some("2026-07-06".into()),
                to: Some("2026-07-13".into()),
                tz: None,
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, AppError::Unprocessable(_)));
    }

    #[tokio::test]
    async fn feed_exposes_dynamic_effective_score_without_writing_note() {
        let dir = tempdir().unwrap();
        let state = setup_state(dir.path());
        let path = dir.path().join("old.md");
        let content = "---\nid: know-old\ntitle: Old\nlayer: knowledge\nfreshness:\n  mode: decay\n  half_life_days: 30\n  floor: 0.4\ncontent_updated_at: '2026-01-01T00:00:00Z'\nscore:\n  interest: 80.0\n  strategy: 80.0\n  consensus: 80.0\n  composite: 80.0\n  updated_at: '2026-01-01T00:00:00Z'\n  last_boosted_at: '2026-01-01T00:00:00Z'\n  access_count: 0\n---\nold body\n";
        fs::write(&path, content).unwrap();
        state
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
                &sample_entity("know-old", "old.md", 80.0, "knowledge"),
                "old body",
            )
            .unwrap();

        let response = feed(
            State(state),
            Query(FeedQuery {
                mode: "explore".to_string(),
                limit: 10,
            }),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(response[0].base_composite, Some(80.0));
        assert!(response[0].freshness_factor.unwrap() < 1.0);
        assert!(response[0].composite.unwrap() < 80.0);
        assert_eq!(fs::read_to_string(path).unwrap(), content);
    }

    /// TC-D03: 时间推移本身不会写回 Vault；所有读取端点共享同一不变量。
    #[tokio::test]
    async fn read_endpoints_do_not_mutate_vault_when_freshness_changes() {
        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        let state = setup_state(&vault);
        let path = vault.join("old.md");
        let content = "---\nid: know-old\ntitle: Old\nlayer: knowledge\nfreshness:\n  mode: decay\n  half_life_days: 30\n  floor: 0.4\ncontent_updated_at: '2026-01-01T00:00:00Z'\nscore:\n  interest: 80.0\n  strategy: 80.0\n  consensus: 80.0\n  composite: 80.0\n  updated_at: '2026-01-01T00:00:00Z'\n  last_boosted_at: '2026-01-01T00:00:00Z'\n  access_count: 0\n---\nold body\n";
        fs::write(&path, content).unwrap();
        state
            .repository
            .lock()
            .await
            .upsert_indexed_entity(
                &IndexedEntity {
                    id: "know-old".to_string(),
                    file_path: "old.md".to_string(),
                    title: Some("Old".to_string()),
                    layer: Some("knowledge".to_string()),
                    status: Some("active".to_string()),
                    interest: Some(80.0),
                    strategy: Some(80.0),
                    consensus: Some(80.0),
                    composite: Some(80.0),
                    access_count: 0,
                    last_boosted_at: Some("2026-01-01T00:00:00Z".to_string()),
                    content_hash: Some("h".to_string()),
                    updated_at: Some("2026-01-01T00:00:00Z".to_string()),
                },
                "old body",
            )
            .unwrap();

        let _ = feed(
            State(state.clone()),
            Query(FeedQuery {
                mode: "explore".to_string(),
                limit: 10,
            }),
        )
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let _ = entities_top(
            State(state.clone()),
            Query(TopQuery {
                layer: None,
                limit: 10,
            }),
        )
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let _ = get_entity(State(state.clone()), Path("know-old".to_string()))
            .await
            .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let _ = search(
            State(state.clone()),
            Query(SearchQuery {
                q: "old".to_string(),
                limit: 10,
            }),
        )
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let _ = graph(State(state.clone())).await.unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);

        let _ = agent_context(
            State(state.clone()),
            Json(AgentContextRequest {
                task: "old body".to_string(),
                top_k: 5,
            }),
        )
        .await
        .unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }
}
