//! 加载 compass.toml 运行时配置。

use crate::models::Weights;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub vault_path: PathBuf,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub allow_non_local: bool,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_request_body_limit_bytes")]
    pub request_body_limit_bytes: usize,
    /// SQLite 索引库路径。相对配置文件目录解析；缺省 `.compass/index.db`。
    #[serde(default)]
    pub db_path: Option<PathBuf>,
    #[serde(default)]
    pub weights: Weights,
}

fn default_port() -> u16 {
    8080
}
fn default_bind() -> String {
    "127.0.0.1".to_string()
}
fn default_request_body_limit_bytes() -> usize {
    1024 * 1024
}

impl Config {
    /// 从 `COMPASS_CONFIG` 环境变量或默认 `compass.toml` 加载。
    /// `vault_path` 与 `db_path` 若为相对路径，相对配置文件目录解析为绝对路径。
    /// 校验权重归一化（F3）。
    pub fn load() -> anyhow::Result<Self> {
        let path = std::env::var("COMPASS_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("compass.toml"));
        let raw = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("读取配置失败 {}: {e}", path.display()))?;
        let mut cfg: Config =
            toml::from_str(&raw).map_err(|e| anyhow::anyhow!("解析配置失败: {e}"))?;

        // F3: 校验权重归一化
        cfg.validate_server_settings()?;

        if !cfg.weights.is_normalized() {
            return Err(anyhow::anyhow!(
                "weights 归一化错误：和为 {}，应为 1.0（检查 compass.toml [weights]）",
                cfg.weights.sum()
            ));
        }

        // 配置文件父目录（相对路径基准）；为空时回退 CWD
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("无法获取配置文件父目录 {}", path.display()))?;
        let base = if parent.as_os_str().is_empty() {
            std::env::current_dir().map_err(|e| anyhow::anyhow!("获取当前目录失败: {e}"))?
        } else {
            parent
                .canonicalize()
                .map_err(|e| anyhow::anyhow!("配置文件父目录 canonicalize 失败: {e}"))?
        };

        // F2: 错误传播；解析 vault_path 为绝对
        if cfg.vault_path.is_relative() {
            cfg.vault_path = base.join(&cfg.vault_path);
        }
        cfg.vault_path = cfg
            .vault_path
            .canonicalize()
            .map_err(|e| anyhow::anyhow!("vault_path 不存在 {}: {e}", cfg.vault_path.display()))?;

        // db_path：缺省 .compass/index.db，相对配置目录解析为绝对
        let mut db_path = cfg
            .db_path
            .take()
            .unwrap_or_else(|| PathBuf::from(".compass").join("index.db"));
        if db_path.is_relative() {
            db_path = base.join(&db_path);
        }
        cfg.db_path = Some(db_path);

        Ok(cfg)
    }

    fn validate_server_settings(&self) -> anyhow::Result<()> {
        if self.bind.trim().is_empty() {
            return Err(anyhow::anyhow!("bind cannot be empty"));
        }
        if self.request_body_limit_bytes == 0 {
            return Err(anyhow::anyhow!(
                "request_body_limit_bytes must be greater than zero"
            ));
        }
        if self.auth_token.as_deref().is_some_and(str::is_empty) {
            return Err(anyhow::anyhow!("auth_token cannot be empty"));
        }
        if !is_local_bind(&self.bind) && !self.allow_non_local {
            return Err(anyhow::anyhow!(
                "non-local bind ({}) requires allow_non_local = true",
                self.bind
            ));
        }
        Ok(())
    }
}

fn is_local_bind(bind: &str) -> bool {
    let bind = bind
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(bind);
    bind.eq_ignore_ascii_case("localhost")
        || bind
            .parse::<std::net::IpAddr>()
            .map(|addr| addr.is_loopback())
            .unwrap_or(false)
}

pub(crate) fn format_bind_address(bind: &str, port: u16) -> String {
    let host = bind
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(bind);
    if host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(bind: &str, allow_non_local: bool) -> Config {
        Config {
            vault_path: PathBuf::from("vault"),
            bind: bind.to_string(),
            port: 8080,
            allow_non_local,
            auth_token: None,
            request_body_limit_bytes: default_request_body_limit_bytes(),
            db_path: None,
            weights: Weights::default(),
        }
    }

    #[test]
    fn default_server_settings_are_local_only() {
        assert_eq!(default_bind(), "127.0.0.1");
        assert!(is_local_bind("127.0.0.1"));
        assert!(is_local_bind("::1"));
        assert!(is_local_bind("[::1]"));
        assert!(is_local_bind("localhost"));
        assert!(!is_local_bind("0.0.0.0"));
        assert_eq!(default_request_body_limit_bytes(), 1024 * 1024);
        assert_eq!(format_bind_address("127.0.0.1", 8080), "127.0.0.1:8080");
        assert_eq!(format_bind_address("::1", 8080), "[::1]:8080");
        assert_eq!(format_bind_address("[::1]", 8080), "[::1]:8080");
        assert!(config("127.0.0.1", false)
            .validate_server_settings()
            .is_ok());
    }

    #[test]
    fn legacy_config_gets_secure_server_defaults() {
        let config: Config = toml::from_str("vault_path = \"vault\"").unwrap();

        assert_eq!(config.bind, "127.0.0.1");
        assert!(!config.allow_non_local);
        assert!(config.auth_token.is_none());
        assert_eq!(config.request_body_limit_bytes, 1024 * 1024);
        assert!(config.validate_server_settings().is_ok());
    }

    #[test]
    fn non_local_bind_requires_explicit_opt_in() {
        let invalid = config("0.0.0.0", false);
        let error = invalid.validate_server_settings().unwrap_err().to_string();
        assert!(error.contains("allow_non_local"));

        assert!(config("0.0.0.0", true).validate_server_settings().is_ok());
    }

    #[test]
    fn invalid_server_settings_are_rejected() {
        let mut config = config("127.0.0.1", false);
        config.request_body_limit_bytes = 0;
        assert!(config.validate_server_settings().is_err());

        config.request_body_limit_bytes = default_request_body_limit_bytes();
        config.auth_token = Some(String::new());
        assert!(config.validate_server_settings().is_err());
    }

    /// TC-H01: Bearer token 不能绕过非本地绑定限制。
    #[test]
    fn non_local_bind_requires_explicit_opt_in_even_with_auth_token() {
        let mut cfg = config("0.0.0.0", false);
        cfg.auth_token = Some("secret".to_string());
        let error = cfg.validate_server_settings().unwrap_err().to_string();
        assert!(error.contains("allow_non_local"));
    }

    /// TC-H01: 显式允许后非本地绑定通过校验。
    #[test]
    fn explicit_non_local_opt_in_is_accepted() {
        let cfg = config("0.0.0.0", true);
        assert!(cfg.validate_server_settings().is_ok());
    }
}
