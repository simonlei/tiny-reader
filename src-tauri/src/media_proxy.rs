//! 本机图片代理。只监听 127.0.0.1 的随机端口，给白名单图床补上 Referer。
//!
//! 页面里的 `<img>` 改不了 Referer。若仍把图绕回数据服务端，字节要多走一趟公网。
//! 这里在阅读器进程里代拉，白名单之外直接拒绝，避免变成开放代理。

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE, LOCATION, REFERER};
use reqwest::redirect::Policy;

const MAX_BYTES: usize = 15 * 1024 * 1024;
const MAX_HOPS: usize = 4;
const IMAGE_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36";

type Exchange = Arc<dyn Fn(&str, &str) -> Result<Upstream, String> + Send + Sync>;

struct Upstream {
    status: u16,
    location: Option<String>,
    content_type: String,
    body: Vec<u8>,
}

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

    fn matches_host(&self, host: &str) -> bool {
        let host = normalize_host(host);
        if host.is_empty() {
            return false;
        }
        self.patterns.iter().any(|p| host_matches(p, &host))
    }
}

pub struct LocalProxy {
    base: String,
    policy: Arc<RwLock<MediaPolicy>>,
    running: Arc<AtomicBool>,
    accept_thread: Option<JoinHandle<()>>,
}

pub struct MediaProxyState {
    inner: Mutex<Option<LocalProxy>>,
}

impl MediaProxyState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub fn configure(&self, hosts: Vec<String>, referer: String) -> Result<String, String> {
        let mut slot = self.inner.lock().map_err(|_| "代理锁失败".to_string())?;
        if slot.is_none() {
            *slot = Some(LocalProxy::start()?);
        }
        let proxy = slot.as_ref().ok_or("本机图片代理没有启动")?;
        proxy.set_policy(&hosts, &referer);
        Ok(proxy.base_url())
    }
}

impl LocalProxy {
    pub fn start() -> Result<Self, String> {
        Self::start_with(Arc::new(exchange_reqwest))
    }

    fn start_with(exchange: Exchange) -> Result<Self, String> {
        let listener =
            TcpListener::bind("127.0.0.1:0").map_err(|e| format!("本机图片代理启动失败: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("本机图片代理启动失败: {e}"))?;
        let addr: SocketAddr = listener
            .local_addr()
            .map_err(|e| format!("本机图片代理启动失败: {e}"))?;
        if !addr.ip().is_loopback() {
            return Err("本机图片代理必须只监听回环地址".into());
        }
        let policy = Arc::new(RwLock::new(MediaPolicy::new(&[], "")));
        let running = Arc::new(AtomicBool::new(true));
        let policy_thread = policy.clone();
        let running_thread = running.clone();
        let accept_thread =
            thread::spawn(move || accept_loop(listener, policy_thread, exchange, running_thread));
        Ok(Self {
            base: format!("http://127.0.0.1:{}", addr.port()),
            policy,
            running,
            accept_thread: Some(accept_thread),
        })
    }

    pub fn base_url(&self) -> String {
        self.base.clone()
    }

    pub fn set_policy(&self, hosts: &[String], referer: &str) {
        let next = MediaPolicy::new(hosts, referer);
        *self.policy.write().unwrap_or_else(|e| e.into_inner()) = next;
    }
}

impl Drop for LocalProxy {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.accept_thread.take() {
            let _ = handle.join();
        }
    }
}

fn accept_loop(
    listener: TcpListener,
    policy: Arc<RwLock<MediaPolicy>>,
    exchange: Exchange,
    running: Arc<AtomicBool>,
) {
    while running.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((mut sock, _)) => {
                let policy = policy.clone();
                let exchange = exchange.clone();
                thread::spawn(move || handle_client(&mut sock, &policy, &exchange));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
}

fn handle_client(sock: &mut TcpStream, policy: &RwLock<MediaPolicy>, exchange: &Exchange) {
    let _ = sock.set_read_timeout(Some(Duration::from_secs(20)));
    let _ = sock.set_write_timeout(Some(Duration::from_secs(20)));
    let head = match read_headers(sock) {
        Ok(h) => h,
        Err(_) => return,
    };
    let request_line = head.split("\r\n").next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    if method.eq_ignore_ascii_case("OPTIONS") {
        write_response(sock, 204, "No Content", "text/plain", b"", false);
        return;
    }
    if !method.eq_ignore_ascii_case("GET") {
        write_response(
            sock,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            "只接受 GET".as_bytes(),
            false,
        );
        return;
    }
    let image_url = match image_url_from_target(target) {
        Ok(url) => url,
        Err((status, msg)) => {
            let reason = if status == 404 {
                "Not Found"
            } else {
                "Bad Request"
            };
            write_response(
                sock,
                status,
                reason,
                "text/plain; charset=utf-8",
                msg.as_bytes(),
                false,
            );
            return;
        }
    };
    let policy = policy.read().unwrap_or_else(|e| e.into_inner()).clone();
    match pull_image(&image_url, &policy, |url, referer| exchange(url, referer)) {
        Ok((ct, bytes)) => write_response(sock, 200, "OK", &safe_content_type(&ct), &bytes, true),
        Err(e) => write_response(
            sock,
            400,
            "Bad Request",
            "text/plain; charset=utf-8",
            e.as_bytes(),
            false,
        ),
    }
}

fn image_url_from_target(target: &str) -> Result<String, (u16, String)> {
    if !target.starts_with('/') {
        return Err((404, "不是图片代理路径".into()));
    }
    let abs = format!("http://127.0.0.1{target}");
    let parsed = reqwest::Url::parse(&abs).map_err(|_| (400, "请求路径不合法".to_string()))?;
    if parsed.path() != "/api/media" {
        return Err((404, "不是图片代理路径".into()));
    }
    parsed
        .query_pairs()
        .find(|(k, _)| k == "url")
        .map(|(_, v)| v.into_owned())
        .filter(|v| !v.trim().is_empty())
        .ok_or((400, "缺少图片地址".into()))
}

fn read_headers(stream: &mut TcpStream) -> Result<String, String> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    let mut seen = 0u8;
    while buf.len() < 16 * 1024 {
        let n = stream.read(&mut byte).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        buf.push(byte[0]);
        if byte[0] == b'\r' && seen % 2 == 0 {
            seen += 1;
        } else if byte[0] == b'\n' && seen % 2 == 1 {
            seen += 1;
        } else if byte[0] == b'\r' {
            seen = 1;
        } else {
            seen = 0;
        }
        if seen == 4 {
            break;
        }
    }
    String::from_utf8(buf).map_err(|_| "请求头不是 UTF-8".to_string())
}

fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
    cache: bool,
) {
    let cache_line = if cache {
        "Cache-Control: private, max-age=86400\r\n"
    } else {
        ""
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{cache_line}Connection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Private-Network: true\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
    let _ = stream.flush();
}

fn safe_content_type(ct: &str) -> String {
    let token = ct.split(';').next().unwrap_or("").trim();
    if token.starts_with("image/") && token.bytes().all(|b| b.is_ascii_graphic()) {
        token.to_string()
    } else {
        "application/octet-stream".into()
    }
}

fn host_matches(pattern: &str, host: &str) -> bool {
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

fn upgrade_http_to_https(url: &str) -> Result<String, String> {
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

fn hotlink_referer(url: &str, policy: &MediaPolicy) -> Result<String, String> {
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
    if policy.referer.contains(['\r', '\n']) {
        return Err("Referer 不合法".into());
    }
    Ok(policy.referer.clone())
}

fn resolve_next_url(current: &str, location: &str, policy: &MediaPolicy) -> Result<String, String> {
    let base = reqwest::Url::parse(current).map_err(|_| "图片地址不合法".to_string())?;
    let next = base
        .join(location.trim())
        .map_err(|_| "图片重定向地址不合法".to_string())?;
    let next = upgrade_http_to_https(next.as_str())?;
    hotlink_referer(&next, policy)?;
    Ok(next)
}

fn pull_image<F>(
    url: &str,
    policy: &MediaPolicy,
    mut exchange: F,
) -> Result<(String, Vec<u8>), String>
where
    F: FnMut(&str, &str) -> Result<Upstream, String>,
{
    let mut current = upgrade_http_to_https(url)?;
    hotlink_referer(&current, policy)?;
    for _ in 0..MAX_HOPS {
        let referer = hotlink_referer(&current, policy)?;
        let resp = exchange(&current, &referer)?;
        if (300..400).contains(&resp.status) {
            let loc = resp.location.unwrap_or_default();
            if loc.trim().is_empty() {
                return Err(format!("图片重定向缺少地址（HTTP {}）", resp.status));
            }
            current = resolve_next_url(&current, &loc, policy)?;
            continue;
        }
        if !(200..300).contains(&resp.status) {
            return Err(format!("图片 HTTP {}", resp.status));
        }
        if resp.body.len() > MAX_BYTES {
            return Err("图片过大".into());
        }
        if !is_image(&resp.content_type, &resp.body) {
            return Err("返回的不是图片".into());
        }
        return Ok((
            normalize_content_type(&resp.content_type, &resp.body),
            resp.body,
        ));
    }
    Err("图片重定向次数过多".into())
}

fn exchange_reqwest(url: &str, referer: &str) -> Result<Upstream, String> {
    let mut req = image_client()
        .get(url)
        .header(ACCEPT, "image/avif,image/webp,image/apng,image/*,*/*;q=0.8");
    if !referer.is_empty() {
        let value = HeaderValue::from_str(referer).map_err(|_| "Referer 不合法".to_string())?;
        req = req.header(REFERER, value);
    }
    let resp = req.send().map_err(|e| format!("图片请求失败: {e}"))?;
    let status = resp.status().as_u16();
    let location = resp
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let content_type = resp
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
    let body = resp
        .bytes()
        .map_err(|e| format!("读取图片失败: {e}"))?
        .to_vec();
    if body.len() > MAX_BYTES {
        return Err("图片过大".into());
    }
    Ok(Upstream {
        status,
        location,
        content_type,
        body,
    })
}

fn image_client() -> &'static Client {
    static CLIENT: std::sync::OnceLock<Client> = std::sync::OnceLock::new();
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
    ct.starts_with("image/") || looks_like_image_magic(body)
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
    use super::*;
    use std::sync::atomic::AtomicUsize;

    fn jandan() -> MediaPolicy {
        MediaPolicy::new(
            &["img.wangmoyu.com".into(), "*.cdn.dfyun.com.cn".into()],
            "https://jandan.net/",
        )
    }

    fn png() -> Vec<u8> {
        b"\x89PNG\r\n\x1a\nmore".to_vec()
    }

    fn ok_png() -> Upstream {
        Upstream {
            status: 200,
            location: None,
            content_type: "image/png".into(),
            body: png(),
        }
    }

    #[test]
    fn http_image_is_requested_as_https_with_referer() {
        let policy = jandan();
        let mut hits = Vec::new();
        let (ct, body) = pull_image(
            "http://img.wangmoyu.com:80/a.png?q=1",
            &policy,
            |url, referer| {
                hits.push((url.to_string(), referer.to_string()));
                Ok(ok_png())
            },
        )
        .unwrap();
        assert_eq!(
            hits,
            vec![(
                "https://img.wangmoyu.com/a.png?q=1".into(),
                "https://jandan.net/".into()
            )]
        );
        assert_eq!(ct, "image/png");
        assert!(body.starts_with(b"\x89PNG"));
    }

    #[test]
    fn redirect_must_stay_on_allowlist() {
        let policy = jandan();
        let mut hits = Vec::new();
        let (ct, _) = pull_image("https://img.wangmoyu.com/a.png", &policy, |url, referer| {
            hits.push((url.to_string(), referer.to_string()));
            if hits.len() == 1 {
                Ok(Upstream {
                    status: 302,
                    location: Some("http://node7-27.cdn.dfyun.com.cn/f/a.png".into()),
                    content_type: String::new(),
                    body: Vec::new(),
                })
            } else {
                Ok(ok_png())
            }
        })
        .unwrap();
        assert_eq!(ct, "image/png");
        assert_eq!(hits[1].0, "https://node7-27.cdn.dfyun.com.cn/f/a.png");
        assert_eq!(hits[1].1, "https://jandan.net/");

        let err = pull_image(
            "https://img.wangmoyu.com/a.png",
            &policy,
            |_url, _referer| {
                Ok(Upstream {
                    status: 302,
                    location: Some("https://example.com/a.png".into()),
                    content_type: String::new(),
                    body: Vec::new(),
                })
            },
        );
        assert!(err.is_err(), "{err:?}");
    }

    #[test]
    fn rejects_non_image_and_off_list_before_any_request() {
        let policy = jandan();
        let calls = AtomicUsize::new(0);
        let err = pull_image("https://example.com/a.png", &policy, |_url, _referer| {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(ok_png())
        });
        assert!(err.is_err());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(hotlink_referer("https://user:pass@img.wangmoyu.com/a.png", &policy).is_err());
        assert!(hotlink_referer("https://127.0.0.1/a.png", &policy).is_err());

        let err = pull_image(
            "https://img.wangmoyu.com/a.png",
            &policy,
            |_url, _referer| {
                Ok(Upstream {
                    status: 200,
                    location: None,
                    content_type: "text/html".into(),
                    body: b"<html></html>".to_vec(),
                })
            },
        );
        assert!(err.unwrap_err().contains("不是图片"));
    }

    #[test]
    fn local_server_adds_referer_and_stays_on_loopback() {
        let calls = Arc::new(AtomicUsize::new(0));
        let calls2 = calls.clone();
        let proxy = LocalProxy::start_with(Arc::new(move |url, referer| {
            calls2.fetch_add(1, Ordering::SeqCst);
            assert_eq!(url, "https://img.wangmoyu.com/a.png");
            assert_eq!(referer, "https://jandan.net/");
            Ok(ok_png())
        }))
        .unwrap();
        assert!(proxy.base_url().starts_with("http://127.0.0.1:"));
        proxy.set_policy(
            &["img.wangmoyu.com".into(), "*.cdn.dfyun.com.cn".into()],
            "https://jandan.net/",
        );

        let image = proxy_url(&proxy.base_url(), "http://img.wangmoyu.com/a.png");
        let (status, ct, body) = http_get(&image);
        assert_eq!(status, 200, "{}", String::from_utf8_lossy(&body));
        assert_eq!(ct, "image/png");
        assert!(body.starts_with(b"\x89PNG"));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let blocked = proxy_url(&proxy.base_url(), "https://example.com/secret.png");
        let (status, _, _) = http_get(&blocked);
        assert_eq!(status, 400);
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        let (status, _, _) = http_get(&format!("{}/other", proxy.base_url()));
        assert_eq!(status, 404);
    }

    fn proxy_url(base: &str, image: &str) -> String {
        let mut u = reqwest::Url::parse(&format!("{base}/api/media")).unwrap();
        u.query_pairs_mut().append_pair("url", image);
        u.to_string()
    }

    fn http_get(url: &str) -> (u16, String, Vec<u8>) {
        let resp = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap()
            .get(url)
            .send()
            .unwrap();
        let status = resp.status().as_u16();
        let ct = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let body = resp.bytes().unwrap().to_vec();
        (status, ct, body)
    }
}
