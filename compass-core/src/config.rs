//! 加载 compass.toml 运行时配置。

use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::models::Weights;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub vault_path: PathBuf,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub decay: DecayConfig,
    #[serde(default)]
    pub weights: Weights,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DecayConfig {
    #[serde(default = "default_rate")]
    pub daily_rate: f64,
    #[serde(default = "default_floor")]
    pub floor: f64,
    #[serde(default = "default_boost_protection")]
    pub boost_protection_days: i64,
    #[serde(default = "default_direction_factor")]
    pub direction_layer_factor: f64,
}

fn default_port() -> u16 { 8080 }
fn default_rate() -> f64 { 0.98 }
fn default_floor() -> f64 { 0.5 }
fn default_boost_protection() -> i64 { 3 }
fn default_direction_factor() -> f64 { 0.5 }

impl Default for DecayConfig {
    fn default() -> Self {
        Self {
            daily_rate: default_rate(),
            floor: default_floor(),
            boost_protection_days: default_boost_protection(),
            direction_layer_factor: default_direction_factor(),
        }
    }
}

impl Config {
    /// 从 `COMPASS_CONFIG` 环境变量或默认 `compass.toml` 加载。
    /// `vault_path` 若为相对路径，相对 config 文件目录解析为绝对路径。
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("COMPASS_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("compass.toml"));
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("读取配置失败 {}: {e}", path.display()))?;
        let mut cfg: Config = toml::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("解析配置失败: {e}"))?;
        if cfg.vault_path.is_relative() {
            let base = path
                .parent()
                .unwrap_or(Path::new("."))
                .canonicalize()
                .unwrap_or_default();
            cfg.vault_path = base.join(&cfg.vault_path);
        }
        cfg.vault_path = cfg.vault_path.canonicalize().map_err(|e| {
            anyhow::anyhow!("vault_path 不存在 {}: {e}", cfg.vault_path.display())
        })?;
        Ok(cfg)
    }
}
