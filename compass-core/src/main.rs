//! Compass - 评分引力场引擎（纯 Rust 单二进制）。
//! T1.6：加载配置 + 初始化 DB + 启动 FileWatcher + axum API。

mod api;
mod config;
// T4.0 freezes contracts before the endpoint tasks consume them.
#[allow(dead_code)]
mod contracts;
mod db;
#[cfg(test)]
mod e2e_tests;
mod frontmatter;
mod models;
mod scoring;
mod watcher;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

fn frozen_web_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compass-core manifest directory must have a workspace parent")
        .join("web")
}

fn build_router(cfg: Arc<config::Config>, db: Arc<Mutex<db::Db>>) -> axum::Router {
    api::apply_security(
        api::router_from_config(Arc::clone(&cfg), db)
            .fallback_service(tower_http::services::ServeDir::new(frozen_web_dir())),
        &cfg,
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::load()?;
    let addr = config::format_bind_address(&cfg.bind, cfg.port);
    info!(vault = %cfg.vault_path.display(), bind = %cfg.bind, port = cfg.port, "Compass starting");

    // 初始化 DB
    let db_path = cfg
        .db_path
        .clone()
        .unwrap_or_else(|| cfg.vault_path.join(".compass").join("index.db"));
    let db = Arc::new(Mutex::new(db::Db::open(&db_path)?));
    info!(db = %db_path.display(), "database opened");
    let schema_version = db.lock().await.schema_version()?;
    info!(schema_version, "database schema ready");

    // 全量重建索引
    let stats = db.lock().await.rebuild_from_vault(&cfg.vault_path)?;
    info!(
        indexed = stats.indexed,
        skipped = stats.skipped,
        duplicates = stats.duplicates,
        "rebuild complete"
    );

    // 启动 FileWatcher
    let mut file_watcher =
        watcher::FileWatcher::new(cfg.vault_path.clone(), db.clone(), cfg.weights);
    file_watcher.start().await?;
    info!("FileWatcher started");

    // 启动 HTTP API
    let cfg = Arc::new(cfg);
    let app = build_router(cfg, db);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use tempfile::tempdir;
    use tower::ServiceExt;

    #[tokio::test]
    async fn production_router_serves_frozen_web_assets_and_graph() {
        for asset in ["index.html", "style.css", "app.js"] {
            assert!(
                frozen_web_dir().join(asset).is_file(),
                "frozen Web asset is missing: {asset}"
            );
        }

        let dir = tempdir().unwrap();
        let vault = dir.path().join("vault");
        std::fs::create_dir_all(&vault).unwrap();
        let cfg = Arc::new(config::Config {
            vault_path: vault,
            bind: "127.0.0.1".to_string(),
            port: 8080,
            allow_non_local: false,
            auth_token: None,
            request_body_limit_bytes: 1024 * 1024,
            db_path: None,
            weights: models::Weights::default(),
        });
        let app = build_router(cfg, Arc::new(Mutex::new(db::Db::open_in_memory().unwrap())));

        let response = app
            .clone()
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let index = String::from_utf8(
            to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(index.contains("href=\"style.css\""));
        assert!(index.contains("src=\"app.js\""));

        for path in ["/style.css", "/app.js", "/graph"] {
            let response = app
                .clone()
                .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "expected {path} to remain reachable"
            );
        }
    }
}
