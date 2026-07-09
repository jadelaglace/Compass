//! Compass — 评分引力场引擎（纯 Rust 单二进制）。
//! T1.6：加载配置 → 打开 SQLite → 启动 FileWatcher → 启动 axum API。

mod api;
mod config;
mod db;
mod frontmatter;
mod models;
mod scoring;
mod watcher;

use std::sync::Arc;
use tracing::info;

use crate::api::AppState;
use crate::db::Db;
use crate::watcher::FileWatcher;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cfg = config::Config::load()?;
    let addr = format!("0.0.0.0:{}", cfg.port);
    info!(vault = %cfg.vault_path.display(), port = cfg.port, "Compass starting");

    // 打开 SQLite 索引库
    let db_path = cfg
        .db_path
        .clone()
        .expect("Config::load 已设置默认 db_path");
    let db = Arc::new(tokio::sync::Mutex::new(Db::open(&db_path)?));
    info!(db = %db_path.display(), "SQLite opened");

    // 启动 FileWatcher 监听 vault
    let mut watcher = FileWatcher::new(cfg.vault_path.clone(), db.clone(), cfg.weights);
    watcher.start().await?;
    info!("FileWatcher started");

    let state = Arc::new(AppState {
        cfg: Arc::new(cfg),
        db,
    });

    let app = api::router(state);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
