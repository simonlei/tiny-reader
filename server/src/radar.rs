//! RSSHub Radar 规则：`/api/radar/rules` 的拉取、缓存与「网页地址 -> RSSHub 路由」匹配。
//!
//! 规则形如：
//! ```json
//! {
//!   "zhihu.com": {
//!     "_name": "知乎",
//!     "www": [ { "title": "用户想法", "docs": "...",
//!               "source": ["/people/:id/pins"], "target": "/zhihu/people/pins/:id" } ]
//!   }
//! }
//! ```
//! 二级 key 是子域名；key 为 "." 表示域名本身。

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::RsshubConfig;

/// 一条可用于订阅的候选路由
#[derive(Debug, Clone, Serialize)]
pub struct RadarCandidate {
    /// 规则里的标题，例如「用户想法」
    pub title: String,
    /// RSSHub 路由，例如 "/zhihu/people/pins/simonlei"
    pub route: String,
    /// 可直接存进 feeds.url 的形态，例如 "rsshub://zhihu/people/pins/simonlei"
    pub rsshub_url: String,
    /// 官方文档链接，便于用户查看该路由的参数说明
    pub docs: String,
}

#[derive(Debug, Clone)]
struct RadarRule {
    title: String,
    docs: String,
    /// 网页路径模式，例如 "/people/:id/pins"
    source: String,
    /// RSSHub 路由模板，例如 "/zhihu/people/pins/:id"
    target: String,
}

#[derive(Debug, Deserialize)]
struct RuleItem {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    docs: Option<String>,
    #[serde(default)]
    source: Vec<String>,
    #[serde(default)]
    target: Option<String>,
}

struct Inner {
    /// 完整 host（小写）-> 该 host 下的规则
    by_host: HashMap<String, Vec<RadarRule>>,
    fetched_at: Option<Instant>,
    rule_count: usize,
}

pub struct Radar {
    inner: std::sync::RwLock<Inner>,
    /// 防止并发重复拉取
    reload_lock: tokio::sync::Mutex<()>,
    client: reqwest::Client,
    base_url: String,
    cache_ttl: Duration,
}

impl Radar {
    pub fn new(cfg: &RsshubConfig) -> Result<Self, reqwest::Error> {
        let timeout = Duration::from_secs(cfg.timeout_secs.max(1));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::limited(5))
            .build()?;
        Ok(Self {
            inner: std::sync::RwLock::new(Inner {
                by_host: HashMap::new(),
                fetched_at: None,
                rule_count: 0,
            }),
            reload_lock: tokio::sync::Mutex::new(()),
            client,
            base_url: cfg.base_url_trimmed(),
            cache_ttl: Duration::from_secs(cfg.radar_cache_secs.max(60)),
        })
    }

    pub fn enabled(&self) -> bool {
        !self.base_url.is_empty()
    }

    /// 已加载的规则条数（用于健康检查/日志）
    pub fn rule_count(&self) -> usize {
        self.inner.read().map(|g| g.rule_count).unwrap_or(0)
    }

    /// 确保规则已加载且未过期。首次调用会真正拉取一次。
    /// 已过期但拉取失败时**不会**报错，继续用旧索引，避免「规则挂了就不能订阅」。
    pub async fn ensure_loaded(&self) -> Result<(), String> {
        if !self.enabled() {
            return Err("未配置 rsshub.base_url，无法使用 RSSHub".to_string());
        }
        if self.is_fresh() {
            return Ok(());
        }

        let _guard = self.reload_lock.lock().await;
        if self.is_fresh() {
            return Ok(());
        }

        let has_old = self
            .inner
            .read()
            .map(|g| g.fetched_at.is_some())
            .unwrap_or(false);

        match self.reload().await {
            Ok(n) => {
                tracing::info!("Radar 规则已加载：{} 条", n);
                Ok(())
            }
            Err(e) if has_old => {
                tracing::warn!("Radar 规则刷新失败，继续使用旧缓存: {e}");
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn is_fresh(&self) -> bool {
        match self.inner.read() {
            Ok(g) => match g.fetched_at {
                Some(t) => t.elapsed() < self.cache_ttl,
                None => false,
            },
            Err(_) => false,
        }
    }

    /// 拉取并重建索引，返回规则条数
    pub async fn reload(&self) -> Result<usize, String> {
        let url = format!("{}/api/radar/rules", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("拉取 Radar 规则失败: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("拉取 Radar 规则失败: HTTP {}", resp.status()));
        }

        // reqwest 未启用 json feature（保持 default-features=false），自己用 serde_json 解
        let body = resp
            .bytes()
            .await
            .map_err(|e| format!("读取 Radar 规则失败: {e}"))?;
        let json: HashMap<String, HashMap<String, Value>> = serde_json::from_slice(&body)
            .map_err(|e| format!("解析 Radar 规则失败: {e}"))?;

        let mut by_host: HashMap<String, Vec<RadarRule>> = HashMap::new();
        let mut count = 0usize;

        for (domain, subs) in json {
            for (sub, value) in subs {
                if sub == "_name" {
                    continue;
                }
                let arr = match value {
                    Value::Array(a) => a,
                    _ => continue,
                };
                let host = if sub == "." {
                    domain.clone()
                } else {
                    format!("{sub}.{domain}")
                }
                .to_lowercase();

                let list = by_host.entry(host).or_default();
                for raw in arr {
                    let item: RuleItem = match serde_json::from_value(raw) {
                        Ok(i) => i,
                        Err(_) => continue,
                    };
                    let target = match item.target.as_deref().map(str::trim) {
                        Some(t) if !t.is_empty() => t.to_string(),
                        _ => continue,
                    };
                    let title = item.title.clone().unwrap_or_default();
                    let docs = item.docs.clone().unwrap_or_default();
                    // 一条规则可能有多个 source 模式，摊平成多条
                    for src in &item.source {
                        let src = src.trim();
                        if src.is_empty() {
                            continue;
                        }
                        list.push(RadarRule {
                            title: title.clone(),
                            docs: docs.clone(),
                            source: src.to_string(),
                            target: target.clone(),
                        });
                        count += 1;
                    }
                }
            }
        }

        let n = count;
        if let Ok(mut g) = self.inner.write() {
            g.by_host = by_host;
            g.fetched_at = Some(Instant::now());
            g.rule_count = n;
        }
        Ok(n)
    }

    /// 为一个网页地址找出所有可用的 RSSHub 路由（已按 route 去重）
    pub fn discover(&self, url: &str) -> Vec<RadarCandidate> {
        let parsed = match reqwest::Url::parse(url) {
            Ok(u) => u,
            Err(_) => return Vec::new(),
        };
        let host = parsed.host_str().unwrap_or("").to_lowercase();
        if host.is_empty() {
            return Vec::new();
        }
        let path = parsed.path();

        let mut out = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        if let Ok(g) = self.inner.read() {
            if let Some(rules) = g.by_host.get(&host) {
                for r in rules {
                    let caps = match match_path(&r.source, path) {
                        Some(c) => c,
                        None => continue,
                    };
                    let route = match fill_target(&r.target, &caps) {
                        Some(t) => t,
                        None => continue,
                    };
                    if !seen.insert(route.clone()) {
                        continue;
                    }
                    out.push(RadarCandidate {
                        title: if r.title.is_empty() {
                            route.clone()
                        } else {
                            r.title.clone()
                        },
                        rsshub_url: format!("rsshub://{}", route.trim_start_matches('/')),
                        route,
                        docs: r.docs.clone(),
                    });
                }
            }
        }

        out
    }
}

/// 按 `/` 分段，忽略空段
fn segments(s: &str) -> Vec<&str> {
    s.split('/').filter(|x| !x.is_empty()).collect()
}

/// 用 source 模式匹配网页路径，返回捕获到的参数。
/// 支持 `:name` 捕获与**尾部** `*` 通配（`/*` 匹配任意路径，含根路径）。
fn match_path(pattern: &str, path: &str) -> Option<HashMap<String, String>> {
    let p = segments(pattern);
    let q = segments(path);
    let mut caps: HashMap<String, String> = HashMap::new();

    let mut i = 0usize;
    let mut j = 0usize;
    while i < p.len() {
        let seg = p[i];
        if seg == "*" {
            // 只支持尾部通配；中间的 `*` 语义不明确，直接放弃这条规则
            if i + 1 != p.len() {
                return None;
            }
            j = q.len();
            break;
        }
        let actual = match q.get(j) {
            Some(a) => *a,
            None => return None,
        };
        if let Some(name) = seg.strip_prefix(':') {
            caps.insert(name.to_string(), actual.to_string());
        } else if seg != actual {
            return None;
        }
        i += 1;
        j += 1;
    }

    if j != q.len() {
        return None;
    }
    Some(caps)
}

/// 用捕获到的参数填充 target 模板。
/// `:name` 必须捕获到，`:name?` 为可选段（没捕获就整段省略）。
fn fill_target(target: &str, caps: &HashMap<String, String>) -> Option<String> {
    let mut out = String::new();
    for seg in segments(target) {
        if let Some(rest) = seg.strip_prefix(':') {
            let (name, optional) = match rest.strip_suffix('?') {
                Some(n) => (n, true),
                None => (rest, false),
            };
            match caps.get(name) {
                Some(v) => {
                    out.push('/');
                    out.push_str(v);
                }
                None if optional => {}
                None => return None,
            }
        } else {
            out.push('/');
            out.push_str(seg);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_captures_params() {
        let caps = match_path("/people/:id/pins", "/people/simonlei/pins").unwrap();
        assert_eq!(caps.get("id").map(String::as_str), Some("simonlei"));
    }

    #[test]
    fn match_literal_mismatch() {
        assert!(match_path("/people/:id/pins", "/people/simonlei/answers").is_none());
    }

    #[test]
    fn match_segment_count() {
        assert!(match_path("/:id", "/a/b").is_none());
        assert!(match_path("/a/:id", "/a").is_none());
    }

    #[test]
    fn match_trailing_star() {
        assert!(match_path("/*", "/").is_some());
        assert!(match_path("/*", "/a/b/c").is_some());
        // 中间的通配不支持，直接放弃
        assert!(match_path("/*/x", "/a/b/x").is_none());
    }

    #[test]
    fn fill_required_and_optional() {
        let mut caps = HashMap::new();
        caps.insert("id".to_string(), "42".to_string());
        assert_eq!(
            fill_target("/zhihu/people/pins/:id", &caps).as_deref(),
            Some("/zhihu/people/pins/42")
        );
        // 必填参数缺失 -> 规则不可用
        assert!(fill_target("/x/:uid", &caps).is_none());
        // 可选参数缺失 -> 整段省略
        assert_eq!(
            fill_target("/weibo/search/hot/:fulltext?", &caps).as_deref(),
            Some("/weibo/search/hot")
        );
    }
}
