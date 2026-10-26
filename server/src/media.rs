//! 代拉带防盗链的图片。
//!
//! 煎蛋图床（dfyun / wangmoyu）不带 `Referer: https://jandan.net/` 会直接断开连接。
//! 桌面端和安卓端在本机加这个头。这里留给普通浏览器里的阅读器：页面发 `<img>` 时改不了 Referer。
//! 目标若是 http，先改成 https 再拉：不少图床两种都开着，代拉只走加密连接。

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::header::{ACCEPT, CONTENT_TYPE, LOCATION, REFERER};
use reqwest::redirect::Policy;
use reqwest::Client;

const MAX_BYTES: usize = 15 * 1024 * 1024;
const MAX_HOPS: usize = 4;
const IMAGE_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

/// 配置里的图片域名白名单，以及代拉时带的 Referer。
#[derive(Debug, Clone)]
pub struct MediaPolicy {
    patterns: Vec<String>,
    referer: String,
}

impl MediaPolicy {
    pub fn new(hosts: &[String], referer: &str) -> Self {
        let patterns = hosts
            .iter()
            .map(|s| normalize_pattern(s))
            .filter(|s| !s.is_empty())
            .collect();
        Self {
            patterns,
            referer: referer.trim().to_string(),
        }
    }

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn referer(&self) -> &str {
        &self.referer
    }

    pub fn matches_host(&self, host: &str) -> bool {
        let host = normalize_host(host);
        if host.is_empty() {
            return false;
        }
        self.patterns.iter().any(|p| host_matches(p, &host))
    }
}

/// 精确域名，或以 `.` / `*.` 开头表示只匹配子域名。
pub fn host_matches(pattern: &str, host: &str) -> bool {
    let host = normalize_host(host);
    let pattern = normalize_pattern(pattern);
    if host.is_empty() || pattern.is_empty() {
        return false;
    }
    if let Some(suffix) = pattern.strip_prefix('.') {
        if suffix.is_empty() {
            return false;
        }
        return host.ends_with(&format!(".{suffix}"));
    }
    host == pattern
}

fn normalize_host(host: &str) -> String {
    host.trim().trim_end_matches('.').to_ascii_lowercase()
}

fn normalize_pattern(pattern: &str) -> String {
    let pattern = pattern.trim().trim_end_matches('.').to_ascii_lowercase();
    if let Some(rest) = pattern.strip_prefix("*.") {
        if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        }
    } else {
        pattern
    }
}

/// 明文 http 改成 https。显式的 `:80` 去掉，避免打到 https 的 80 端口。
pub fn upgrade_http_to_https(url: &str) -> Result<String, String> {
    let mut u = reqwest::Url::parse(url).map_err(|_| "图片地址不合法".to_string())?;
    if u.scheme() == "http" {
        u.set_scheme("https")
            .map_err(|_| "图片地址不合法".to_string())?;
        if u.port() == Some(80) {
            u.set_port(None).map_err(|_| "图片地址不合法".to_string())?;
        }
    }
    Ok(u.to_string())
}

/// 校验地址并返回应附带的 Referer。不在白名单里就拒绝，避免变成开放代理。
/// 只接受已经是 https 的地址；http 要先经过 [`upgrade_http_to_https`]。
pub fn hotlink_referer(url: &str, policy: &MediaPolicy) -> Result<String, String> {
    let u = reqwest::Url::parse(url).map_err(|_| "图片地址不合法".to_string())?;
    if u.scheme() != "https" {
        return Err("只代理 https 图片".into());
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err("图片地址不合法".into());
    }
    if let Some(port) = u.port() {
        if port != 443 {
            return Err("图片地址端口不被允许".into());
        }
    }
    let host = u.host_str().unwrap_or("");
    if !policy.matches_host(host) {
        return Err("这个图片域名不走代理".into());
    }
    Ok(policy.referer.clone())
}

pub fn resolve_next_url(
    current: &str,
    location: &str,
    policy: &MediaPolicy,
) -> Result<String, String> {
    let base = reqwest::Url::parse(current).map_err(|_| "图片地址不合法".to_string())?;
    let next = base
        .join(location.trim())
        .map_err(|_| "图片重定向地址不合法".to_string())?;
    let next = upgrade_http_to_https(next.as_str())?;
    hotlink_referer(&next, policy)?;
    Ok(next)
}

/// 带 Referer 把白名单里的图片拉回来。跟随的跳转也必须仍在白名单内。
/// http 会先改成 https，真正发出去的请求始终是 https。
pub async fn fetch_hotlink_image(
    url: &str,
    policy: &MediaPolicy,
) -> Result<(String, Vec<u8>), String> {
    let mut current = upgrade_http_to_https(url)?;
    hotlink_referer(&current, policy)?;
    let client = image_client();
    for _ in 0..MAX_HOPS {
        let referer = hotlink_referer(&current, policy)?;
        let mut req = client
            .get(&current)
            .header(ACCEPT, "image/avif,image/webp,image/apng,image/*,*/*;q=0.8");
        if !referer.is_empty() {
            let value = reqwest::header::HeaderValue::from_str(&referer)
                .map_err(|_| "Referer 不合法".to_string())?;
            req = req.header(REFERER, value);
        }
        let resp = req.send().await.map_err(|e| format!("图片请求失败: {e}"))?;
        let status = resp.status();
        if status.is_redirection() {
            let loc = resp
                .headers()
                .get(LOCATION)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if loc.is_empty() {
                return Err(format!("图片重定向缺少地址（HTTP {status}）"));
            }
            current = resolve_next_url(&current, loc, policy)?;
            continue;
        }
        if !status.is_success() {
            return Err(format!("图片 HTTP {status}"));
        }
        let ct = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if let Some(len) = resp.content_length() {
            if len as usize > MAX_BYTES {
                return Err("图片过大".into());
            }
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("读取图片失败: {e}"))?;
        if bytes.len() > MAX_BYTES {
            return Err("图片过大".into());
        }
        if !is_image(&ct, &bytes) {
            return Err("返回的不是图片".into());
        }
        return Ok((normalize_content_type(&ct, &bytes), bytes.to_vec()));
    }
    Err("图片重定向次数过多".into())
}

fn image_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .redirect(Policy::none())
            .timeout(Duration::from_secs(20))
            .user_agent(IMAGE_UA)
            .build()
            .expect("初始化图片客户端失败")
    })
}

fn is_image(content_type: &str, body: &[u8]) -> bool {
    let ct = content_type.to_ascii_lowercase();
    if ct.starts_with("image/") {
        return true;
    }
    looks_like_image_magic(body)
}

fn looks_like_image_magic(body: &[u8]) -> bool {
    body.starts_with(b"\x89PNG")
        || body.starts_with(&[0xFF, 0xD8])
        || body.starts_with(b"GIF8")
        || (body.starts_with(b"RIFF") && body.len() >= 12 && &body[8..12] == b"WEBP")
}

fn normalize_content_type(content_type: &str, body: &[u8]) -> String {
    let ct = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if ct.starts_with("image/") {
        return ct;
    }
    if body.starts_with(b"\x89PNG") {
        "image/png".into()
    } else if body.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg".into()
    } else if body.starts_with(b"GIF8") {
        "image/gif".into()
    } else if body.starts_with(b"RIFF") {
        "image/webp".into()
    } else {
        "application/octet-stream".into()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        host_matches, hotlink_referer, resolve_next_url, upgrade_http_to_https, MediaPolicy,
    };

    fn jandan_policy() -> MediaPolicy {
        MediaPolicy::new(
            &["img.wangmoyu.com".into(), "*.cdn.dfyun.com.cn".into()],
            "https://jandan.net/",
        )
    }

    #[test]
    fn referer_is_part_of_the_public_policy() {
        let policy = jandan_policy();
        assert_eq!(policy.referer(), "https://jandan.net/");
        let custom = MediaPolicy::new(&["photos.example.org".into()], "  https://cdn.example/  ");
        assert_eq!(custom.referer(), "https://cdn.example/");
    }

    #[test]
    fn allows_configured_hosts_only() {
        let policy = jandan_policy();
        assert!(policy.matches_host("wangmoyuimg.cdn.dfyun.com.cn"));
        assert!(policy.matches_host("node7-27.cdn.dfyun.com.cn"));
        assert!(policy.matches_host("img.wangmoyu.com"));
        assert!(!policy.matches_host("cdn.dfyun.com.cn"));
        assert!(!policy.matches_host("example.com"));
        assert!(!policy.matches_host("evil.example.com"));
        // `*.` 和 `.` 是同一种「只匹配子域名」
        assert_eq!(
            policy.patterns(),
            ["img.wangmoyu.com", ".cdn.dfyun.com.cn"].as_slice()
        );
    }

    #[test]
    fn extra_host_comes_from_the_list_not_the_code() {
        let policy = MediaPolicy::new(&["photos.example.org".into()], "https://cdn.example/");
        assert!(policy.matches_host("photos.example.org"));
        assert!(!policy.matches_host("img.wangmoyu.com"));
        assert!(host_matches(".example.org", "a.example.org"));
        assert!(!host_matches(".example.org", "example.org"));
        assert_eq!(
            hotlink_referer("https://photos.example.org/a.png", &policy).unwrap(),
            "https://cdn.example/"
        );
    }

    #[test]
    fn referer_only_for_allowlisted_https() {
        let policy = jandan_policy();
        let url = "https://wangmoyuimg.cdn.dfyun.com.cn/mw600/e4b43cc2b69131bd8e06330d53be8dd9.png";
        assert_eq!(
            hotlink_referer(url, &policy).unwrap(),
            "https://jandan.net/"
        );
        // 校验层仍拒绝明文 http，升级发生在代拉入口
        assert!(hotlink_referer("http://img.wangmoyu.com/a.png", &policy).is_err());
        assert!(hotlink_referer("https://example.com/a.png", &policy).is_err());
        assert!(hotlink_referer("https://user:pass@img.wangmoyu.com/a.png", &policy).is_err());
        assert!(hotlink_referer("https://127.0.0.1/a.png", &policy).is_err());
    }

    #[test]
    fn http_image_is_fetched_over_https() {
        assert_eq!(
            upgrade_http_to_https("http://img.wangmoyu.com/a.png").unwrap(),
            "https://img.wangmoyu.com/a.png"
        );
        assert_eq!(
            upgrade_http_to_https("HTTP://img.wangmoyu.com:80/a.png?q=1").unwrap(),
            "https://img.wangmoyu.com/a.png?q=1"
        );
        assert_eq!(
            upgrade_http_to_https("https://img.wangmoyu.com/a.png").unwrap(),
            "https://img.wangmoyu.com/a.png"
        );
        let policy = jandan_policy();
        let upgraded = upgrade_http_to_https("http://img.wangmoyu.com/a.png").unwrap();
        assert_eq!(
            hotlink_referer(&upgraded, &policy).unwrap(),
            "https://jandan.net/"
        );
    }

    #[test]
    fn http_redirect_is_upgraded_before_allowlist_check() {
        let policy = jandan_policy();
        let current = "https://wangmoyuimg.cdn.dfyun.com.cn/mw600/abc.png";
        let next = resolve_next_url(
            current,
            "http://node7-27.cdn.dfyun.com.cn/f/abc.png",
            &policy,
        )
        .unwrap();
        assert!(
            next.starts_with("https://node7-27.cdn.dfyun.com.cn/"),
            "{next}"
        );
    }

    #[test]
    fn redirect_must_stay_on_allowlist() {
        let policy = jandan_policy();
        let current = "https://wangmoyuimg.cdn.dfyun.com.cn/mw600/abc.png";
        let next = resolve_next_url(
            current,
            "https://node7-27.cdn.dfyun.com.cn/f/token/abc.png?_ex=1",
            &policy,
        )
        .unwrap();
        assert!(next.contains("node7-27.cdn.dfyun.com.cn"), "{next}");
        assert!(resolve_next_url(current, "https://example.com/a.png", &policy).is_err());
    }

    #[tokio::test]
    #[ignore]
    async fn live_dfyun_png_with_referer() {
        let url = "https://wangmoyuimg.cdn.dfyun.com.cn/mw600/e4b43cc2b69131bd8e06330d53be8dd9.png";
        let (ct, bytes) = super::fetch_hotlink_image(url, &jandan_policy())
            .await
            .expect("代拉失败");
        assert!(ct.starts_with("image/"), "{ct}");
        assert!(bytes.starts_with(b"\x89PNG"), "不是 PNG");
        assert!(bytes.len() > 1000, "{}", bytes.len());
    }
}
