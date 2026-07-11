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
mod scheduler;
mod scoring;
mod watcher;

use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

use crate::scheduler::DecayScheduler;

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

    // 启动衰减调度器
    let decay_scheduler = DecayScheduler::new(
        db.clone(),
        cfg.vault_path.clone(),
        cfg.weights,
        cfg.decay.clone(),
    );
    decay_scheduler.start().await?;

    // 启动 HTTP API
    // 接线静态文件 + 根路由
    let web_dir = std::path::Path::new("web");
    let cfg = Arc::new(cfg);
    let app = api::apply_security(
        api::router_from_config(Arc::clone(&cfg), db)
            .fallback_service(tower_http::services::ServeDir::new(web_dir)),
        &cfg,
    );
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
