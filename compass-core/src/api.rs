//! HTTP 路由。T1.1 仅 /health；其余端点见 PRD_v3.0 §7，T1.6 实现。

use axum::{routing::get, Json, Router};
use std::sync::Arc;
use crate::config::Config;

pub fn router(_cfg: Arc<Config>) -> Router {
    Router::new().route("/health", get(health))
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "name": "compass",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
