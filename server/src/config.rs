use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

/// 配置文件默认查找顺序：
///   1. `--config <path>` 命令行参数
///   2. `TINY_READER_CONFIG` 环境变量
///   3. 当前目录下的 `config.toml`
///   4. 可执行文件同级的 `config.toml`
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub refresh: RefreshConfig,
    #[serde(default)]
    pub rsshub: RsshubConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AuthConfig {
    /// 单用户模式：写死一个 token，客户端只要知道它就能访问。
    /// 也可以用环境变量 `TINY_READER_TOKEN` 覆盖。
    #[serde(default = "default_token")]
    pub token: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    #[serde(default = "default_database_path")]
    pub database_path: String,

    /// 每个源最多保留多少篇文章（超出后清理最旧的已读文章），0 表示不限制
    #[serde(default = "default_max_articles_per_feed")]
    pub max_articles_per_feed: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RefreshConfig {
    /// 并发拉取多少个源
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,

    /// 单个源的请求超时（秒）
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,

    /// 拉取时使用的 User-Agent
    #[serde(default = "default_user_agent")]
    pub user_agent: String,

    /// 是否开启定时自动刷新；0 表示关闭
    #[serde(default)]
    pub auto_interval_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RsshubConfig {
    /// 自部署的 RSSHub 实例地址，例如 "http://127.0.0.1:1200"。
    /// 留空表示关闭 RSSHub 支持（此时 rsshub:// 源会拉取失败并给出明确提示）。
    #[serde(default)]
    pub base_url: String,

    /// 拉取 RSSHub 路由的超时（秒）。RSSHub 是实时抓取，比普通 RSS 慢，
    /// 默认给得比 refresh.timeout_secs 更长。
    #[serde(default = "default_rsshub_timeout_secs")]
    pub timeout_secs: u64,

    /// Radar 规则（/api/radar/rules）的缓存有效期（秒），过期后后台重新拉取
    #[serde(default = "default_radar_cache_secs")]
    pub radar_cache_secs: u64,
}

fn default_host() -> String {
    "127.0.0.1".into()
}
fn default_port() -> u16 {
    8787
}
fn default_token() -> String {
    "change-me".into()
}
fn default_database_path() -> String {
    "data/tiny-reader.db".into()
}
fn default_max_articles_per_feed() -> usize {
    2000
}
fn default_concurrency() -> usize {
    8
}
fn default_timeout_secs() -> u64 {
    30
}
fn default_user_agent() -> String {
    "TinyReader/0.1 (+https://github.com/simon-lei/tiny-reader)".into()
}
fn default_rsshub_timeout_secs() -> u64 {
    60
}
fn default_radar_cache_secs() -> u64 {
    12 * 60 * 60
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}
impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            token: default_token(),
        }
    }
}
impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            max_articles_per_feed: default_max_articles_per_feed(),
        }
    }
}
impl Default for RefreshConfig {
    fn default() -> Self {
        Self {
            concurrency: default_concurrency(),
            timeout_secs: default_timeout_secs(),
            user_agent: default_user_agent(),
            auto_interval_secs: 0,
        }
    }
}
impl Default for RsshubConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            timeout_secs: default_rsshub_timeout_secs(),
            radar_cache_secs: default_radar_cache_secs(),
        }
    }
}
impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            auth: AuthConfig::default(),
            storage: StorageConfig::default(),
            refresh: RefreshConfig::default(),
            rsshub: RsshubConfig::default(),
        }
    }
}

impl RsshubConfig {
    /// 是否启用了 RSSHub 支持
    pub fn enabled(&self) -> bool {
        !self.base_url.trim().is_empty()
    }

    /// 去掉结尾斜杠的实例地址
    pub fn base_url_trimmed(&self) -> String {
        self.base_url.trim().trim_end_matches('/').to_string()
    }
}

impl Config {
    pub fn bind_addr(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    /// 数据库文件的绝对路径（相对路径按配置文件所在目录解析）
    pub fn database_abs_path(&self, config_dir: &Path) -> PathBuf {
        let p = PathBuf::from(&self.storage.database_path);
        if p.is_absolute() {
            p
        } else {
            config_dir.join(p)
        }
    }
}

/// 载入配置。找不到配置文件时，会生成一份带随机 token 的默认配置。
pub fn load(explicit: Option<PathBuf>) -> Result<(Config, PathBuf)> {
    let path = match explicit {
        Some(p) => Some(p),
        None => std::env::var_os("TINY_READER_CONFIG").map(PathBuf::from),
    };

    let path = match path {
        Some(p) => p,
        None => {
            let cwd = std::env::current_dir().context("无法获取当前目录")?;
            let candidates = [
                cwd.join("config.toml"),
                cwd.join("server").join("config.toml"),
                std::env::current_exe()
                    .ok()
                    .and_then(|e| e.parent().map(|p| p.join("config.toml")))
                    .unwrap_or_else(|| cwd.join("config.toml")),
            ];
            candidates
                .into_iter()
                .find(|p| p.is_file())
                .unwrap_or_else(|| cwd.join("config.toml"))
        }
    };

    if !path.is_file() {
        let cfg = Config {
            auth: AuthConfig {
                token: random_token(),
            },
            ..Config::default()
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let text = toml::to_string_pretty(&cfg)?;
        std::fs::write(&path, text).context("写入默认配置文件失败")?;
        tracing::warn!("未找到配置文件，已在 {} 生成默认配置", path.display());
        tracing::warn!("  生成的访问 token = {}", cfg.auth.token);
    }

    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
    let mut cfg: Config =
        toml::from_str(&text).with_context(|| format!("解析配置文件失败: {}", path.display()))?;

    // 环境变量优先级最高，方便容器化部署
    if let Ok(t) = std::env::var("TINY_READER_TOKEN") {
        cfg.auth.token = t;
    }
    if let Ok(h) = std::env::var("TINY_READER_HOST") {
        cfg.server.host = h;
    }
    if let Ok(p) = std::env::var("TINY_READER_PORT") {
        if let Ok(p) = p.parse() {
            cfg.server.port = p;
        }
    }
    if let Ok(d) = std::env::var("TINY_READER_DB") {
        cfg.storage.database_path = d;
    }
    if let Ok(b) = std::env::var("TINY_READER_RSSHUB") {
        cfg.rsshub.base_url = b;
    }

    if cfg.auth.token.trim().is_empty() {
        anyhow::bail!("auth.token 不能为空，请在配置文件中设置一个 token");
    }

    let config_dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    Ok((cfg, config_dir))
}

fn random_token() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 32)
}

/// 配置模板（供 `--print-example-config` 输出）
pub fn example_toml() -> String {
    let cfg = Config {
        auth: AuthConfig {
            token: random_token(),
        },
        ..Config::default()
    };
    toml::to_string_pretty(&cfg).unwrap_or_default()
}
