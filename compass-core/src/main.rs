//! Compass — 评分引力场引擎（纯 Rust 单二进制）。
//! T1.1 骨架：加载配置 + 启动 axum /health。

mod api;
mod config;
mod db;
mod frontmatter;
mod models;
mod scoring;
mod watcher;

use std::sync::Arc;
use tracing::info;

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

    let app = api::router(Arc::new(cfg));
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
