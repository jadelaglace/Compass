//! Compass - 评分引力场引擎（纯 Rust 单二进制）。
//! T1.6：加载配置 + 初始化 DB + 启动 FileWatcher + axum API。

mod application;
mod config;
mod domain;
#[cfg(test)]
mod e2e_tests;
mod infrastructure;
mod transport;
mod watcher;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use application::index_service::IndexService;
use application::ports::RepositoryHandle;
use infrastructure::sqlite_repository::SqliteRepository;
use infrastructure::vault_adapter::VaultAdapter;

fn frozen_web_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("compass-core manifest directory must have a workspace parent")
        .join("web")
}

fn build_router(
    cfg: Arc<config::Config>,
    db: RepositoryHandle,
    vault: Arc<VaultAdapter>,
) -> axum::Router {
    transport::http::apply_security(
        transport::http::router_from_config(Arc::clone(&cfg), db, vault)
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
    let db: RepositoryHandle = Arc::new(Mutex::new(SqliteRepository::open(&db_path)?));
    let vault = Arc::new(VaultAdapter::new(cfg.vault_path.clone()));
    let indexer = Arc::new(IndexService::new(vault.clone(), db.clone(), cfg.weights));
    info!(db = %db_path.display(), "database opened");
    let schema_version = db.lock().await.schema_version()?;
    info!(schema_version, "database schema ready");

    // 全量重建索引
    let stats = indexer.rebuild().await?;
    info!(
        indexed = stats.indexed,
        skipped = stats.skipped,
        duplicates = stats.duplicates,
        "rebuild complete"
    );

    // 启动 FileWatcher
    let mut file_watcher = watcher::FileWatcher::new(cfg.vault_path.clone(), indexer);
    file_watcher.start().await?;
    info!("FileWatcher started");

    // 启动 HTTP API
    let cfg = Arc::new(cfg);
    let app = build_router(cfg, db, vault);
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

    #[test]
    fn p52_layer_boundaries_exclude_transport_and_sql_details() {
        let domain_sources = [
            include_str!("domain/mod.rs"),
            include_str!("domain/entity.rs"),
            include_str!("domain/scoring.rs"),
            include_str!("domain/contracts.rs"),
        ];
        let application_sources = [include_str!("application/mod.rs")];

        for source in domain_sources.into_iter().chain(application_sources) {
            assert!(
                !source.contains("axum"),
                "inner layers must not import Axum"
            );
            assert!(
                !source.contains("rusqlite") && !source.contains("IndexedEntity"),
                "inner layers must not depend on SQLite mappings"
            );
        }

        assert!(include_str!("domain/mod.rs").contains("pub(crate) mod"));
        assert!(include_str!("transport/mod.rs").contains("pub(crate) mod http"));
        assert!(include_str!("transport/http.rs").contains("use axum::"));
    }

    #[test]
    fn p53a_vault_adapter_owns_markdown_scanning_and_parsing() {
        let database = include_str!("infrastructure/sqlite_repository.rs")
            .split("#[cfg(test)]\n    fn rebuild_from_vault")
            .next()
            .expect("db production source must exist");
        let adapter = include_str!("infrastructure/vault_adapter.rs");
        let port = include_str!("application/ports.rs");

        for forbidden in [
            "frontmatter::",
            "walk_md",
            "parse_entity",
            "fs::read_dir",
            "read_to_string",
            "fs::",
        ] {
            assert!(
                !database.contains(forbidden),
                "db.rs must not own Vault work: {forbidden}"
            );
        }
        assert!(adapter.contains("impl VaultPort for VaultAdapter"));
        assert!(adapter.contains("walk_markdown"));
        assert!(port.contains("trait VaultPort"));
    }

    #[test]
    fn p53b_index_service_owns_shared_projection_and_watcher_is_an_adapter() {
        let index_service = include_str!("application/index_service.rs");
        let watcher = include_str!("watcher.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("watcher production source must exist");
        let database = include_str!("infrastructure/sqlite_repository.rs");
        let production_database = database
            .split("#[cfg(test)]\n    fn rebuild_from_vault")
            .next()
            .expect("database production source must exist");

        assert!(index_service.contains("async fn rebuild"));
        assert!(index_service.contains("async fn process_changed_path"));
        assert!(index_service.contains("fn project"));
        assert!(!watcher.contains("crate::db"));
        for forbidden in [
            "upsert_entity",
            "delete_entity",
            "delete_entities_under_path",
            "IndexedEntity",
            "ScoreHistoryRow",
        ] {
            assert!(
                !watcher.contains(forbidden),
                "watcher must delegate SQLite work: {forbidden}"
            );
        }
        assert!(!production_database.contains("rebuild_from_vault"));
        assert!(database.contains("replace_index_projections"));
    }

    #[test]
    fn p54_sqlite_rows_are_private_and_query_snapshots_release_the_lock() {
        let repository = include_str!("infrastructure/sqlite_repository.rs");
        let query_service = include_str!("application/query_service.rs");

        assert!(repository.contains("struct EntityRow"));
        assert!(!repository.contains("pub struct EntityRow"));
        assert!(!query_service.contains("rusqlite"));
        assert!(!query_service.contains("EntityRow"));

        let replacement = repository
            .split("fn replace_index_projections")
            .nth(1)
            .expect("repository replacement method must exist");
        assert!(replacement.contains("upsert_projection_tx"));
        assert!(replacement.contains("tx.commit()?"));

        for snapshot in [
            "let entities = self.repository.lock().await.list_entities()?;",
            "let indexed = {",
            "let entity = self\n            .repository",
        ] {
            assert!(
                query_service.contains(snapshot),
                "query service must take a repository snapshot before Vault work"
            );
        }
    }

    #[test]
    fn p55_application_services_own_http_use_case_orchestration() {
        let services = [
            include_str!("application/query_service.rs"),
            include_str!("application/entity_service.rs"),
            include_str!("application/suggestion_service.rs"),
        ];
        for source in services {
            assert!(
                !source.contains("axum") && !source.contains("rusqlite"),
                "application services must not depend on HTTP or SQLite details"
            );
        }

        let http = include_str!("transport/http.rs");
        for delegation in [
            ".weekly_report(",
            ".feed(",
            ".create(",
            ".tag_suggestions(",
            ".related(",
        ] {
            assert!(
                http.contains(delegation),
                "HTTP routes must delegate to an application service: {delegation}"
            );
        }
    }

    #[test]
    fn p56_final_transport_map_has_no_legacy_handlers_or_temporary_exemptions() {
        let http = include_str!("transport/http.rs");
        let architecture = include_str!("../../docs/ARCHITECTURE.md");

        assert!(!http.contains("legacy_"));
        assert!(!http.contains("#![allow(dead_code)]"));
        assert!(http.contains("map_application_error"));
        assert!(
            !include_str!("infrastructure/vault_adapter.rs").contains("impl VaultPort for PathBuf")
        );
        assert!(architecture.contains("## 8. Implemented Module Map"));
        assert!(architecture.contains("watcher.rs"));
    }

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
            weights: domain::entity::Weights::default(),
        });
        let vault_adapter = Arc::new(VaultAdapter::new(cfg.vault_path.clone()));
        let app = build_router(
            cfg,
            Arc::new(Mutex::new(SqliteRepository::open_in_memory().unwrap())),
            vault_adapter,
        );

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
