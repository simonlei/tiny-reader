//! 桌面端访问数据服务端的 HTTP。
//!
//! macOS 的 WKWebView 从 `tauri://` 页面发 `fetch` 时，明文 HTTP 会被
//! App Transport Security 直接丢掉，页面只能看到一次失败的连接。
//! 这里改用本进程的 HTTP 客户端，和安卓端允许明文流量是同一件事。

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use reqwest::blocking::Client;
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;

#[derive(Debug)]
pub struct ApiCall {
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
}

#[derive(Debug)]
pub struct ApiResponse {
    pub status: u16,
    pub body: String,
}

const MAX_BODY: usize = 8 * 1024 * 1024;

pub fn execute(call: ApiCall) -> Result<ApiResponse, String> {
    execute_with(call, shared_client())
}

pub fn execute_timeout(call: ApiCall, timeout: Duration) -> Result<ApiResponse, String> {
    let client = Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| format!("连接失败：{err}"))?;
    execute_with(call, &client)
}

fn shared_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(Duration::from_secs(20))
            .build()
            .expect("api http client")
    })
}

fn execute_with(call: ApiCall, client: &Client) -> Result<ApiResponse, String> {
    let url = call.url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("只接受 http 或 https 地址".into());
    }
    if url.chars().any(|c| c.is_control()) {
        return Err("地址无效".into());
    }

    let method = Method::from_bytes(call.method.trim().as_bytes())
        .map_err(|_| format!("不支持的方法 {}", call.method))?;
    let mut builder = client.request(method, url);
    for (name, value) in &call.headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| format!("请求头无效：{name}"))?;
        let value = HeaderValue::from_str(value).map_err(|_| format!("请求头无效：{name}"))?;
        builder = builder.header(name, value);
    }
    if let Some(body) = call.body {
        builder = builder.body(body);
    }

    let response = builder.send().map_err(|err| format!("连接失败：{err}"))?;
    let status = response.status().as_u16();
    let bytes = response.bytes().map_err(|err| format!("连接失败：{err}"))?;
    if bytes.len() > MAX_BODY {
        return Err("响应过大".into());
    }
    Ok(ApiResponse {
        status,
        body: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn call(url: &str, method: &str, body: Option<&str>) -> ApiCall {
        let mut headers = HashMap::new();
        headers.insert("Authorization".into(), "Bearer secret".into());
        ApiCall {
            url: url.into(),
            method: method.into(),
            headers,
            body: body.map(|s| s.to_string()),
        }
    }

    /// 读完请求头和 Content-Length 指定的正文，再回一段固定响应。
    fn serve(status: u16, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let body = body.to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            let mut header_end = None;
            loop {
                match sock.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        if header_end.is_none() {
                            header_end =
                                buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
                        }
                        if let Some(end) = header_end {
                            let head = String::from_utf8_lossy(&buf[..end]);
                            let need = head.lines().find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                if name.eq_ignore_ascii_case("content-length") {
                                    value.trim().parse::<usize>().ok()
                                } else {
                                    None
                                }
                            });
                            if buf.len() >= end + need.unwrap_or(0) {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
            let resp = format!(
                "HTTP/1.1 {status} TEST\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        });
        (format!("http://{addr}"), rx)
    }

    #[test]
    fn rejects_non_http_urls() {
        let err = execute(call("file:///etc/passwd", "GET", None)).unwrap_err();
        assert!(err.contains("http"), "应拒绝非 http(s) 地址，实际：{err}");
    }

    #[test]
    fn get_returns_status_and_body() {
        let (url, rx) = serve(200, "{\"ok\":true}");
        let resp = execute(call(&format!("{url}/health"), "GET", None)).unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "{\"ok\":true}");
        let raw = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(raw.starts_with("GET /health "), "实际请求：{raw}");
        assert!(
            raw.to_ascii_lowercase()
                .contains("authorization: bearer secret"),
            "应带上 token，实际：{raw}"
        );
    }

    #[test]
    fn post_sends_body_and_keeps_error_status() {
        let (url, rx) = serve(401, "{\"error\":\"bad token\"}");
        let resp = execute(call(&format!("{url}/api/feeds"), "POST", Some("{\"a\":1}"))).unwrap();
        assert_eq!(resp.status, 401);
        assert_eq!(resp.body, "{\"error\":\"bad token\"}");
        let raw = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(raw.starts_with("POST /api/feeds "), "实际请求：{raw}");
        assert!(raw.contains("{\"a\":1}"), "应带上正文，实际：{raw}");
    }

    #[test]
    fn connection_failure_is_an_error() {
        let err = execute_timeout(
            call("http://127.0.0.1:1/health", "GET", None),
            Duration::from_millis(500),
        )
        .unwrap_err();
        assert!(err.contains("连接失败"), "失败原因应可读，实际：{err}");
    }

    #[test]
    fn macos_bundle_allows_http_and_local_network() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Info.plist");
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            text.contains("<key>NSAllowsArbitraryLoads</key>"),
            "macOS 需要允许明文 HTTP，缺少 NSAllowsArbitraryLoads：{}",
            path.display()
        );
        assert!(
            text.contains("<key>NSAllowsLocalNetworking</key>"),
            "缺少 NSAllowsLocalNetworking"
        );
        assert!(
            text.contains("<key>NSLocalNetworkUsageDescription</key>"),
            "macOS 15 访问局域网前需要用途说明"
        );
    }
}
