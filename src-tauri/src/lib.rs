mod api_http;
mod media_proxy;

use media_proxy::MediaProxyState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 用系统默认浏览器打开一个外部链接。
///
/// 之所以不直接用 `<a target="_blank">`：Tauri 的 WebView 默认会在应用内部导航，
/// 而不是交给系统浏览器。
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err("empty url".into());
    }
    // 仅允许 http/https，避免被拿去执行本地程序
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(format!("unsupported url scheme: {url}"));
    }
    std::thread::spawn(move || {
        if let Err(e) = open::that(&url) {
            eprintln!("[tiny-reader] failed to open {url}: {e}");
        }
    });
    Ok(())
}

#[derive(Serialize)]
struct AppInfo {
    version: String,
    platform: String,
    arch: String,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
    }
}

#[derive(Deserialize)]
struct HttpRequestArgs {
    url: String,
    method: String,
    #[serde(default)]
    headers: HashMap<String, String>,
    body: Option<String>,
}

#[derive(Serialize)]
struct HttpRequestResult {
    status: u16,
    body: String,
}

/// macOS WebView 发不出明文 HTTP，业务请求改由本进程发出。
#[tauri::command]
async fn http_request(args: HttpRequestArgs) -> Result<HttpRequestResult, String> {
    let response = tauri::async_runtime::spawn_blocking(move || {
        api_http::execute(api_http::ApiCall {
            url: args.url,
            method: args.method,
            headers: args.headers,
            body: args.body,
        })
    })
    .await
    .map_err(|err| format!("请求任务失败：{err}"))??;

    Ok(HttpRequestResult {
        status: response.status,
        body: response.body,
    })
}

/// 按服务端下发的域名和 Referer 启动（或更新）本机图片代理，返回 `http://127.0.0.1:端口`。
#[tauri::command]
fn set_media_proxy(
    hosts: Vec<String>,
    referer: String,
    proxy: tauri::State<MediaProxyState>,
) -> Result<String, String> {
    proxy.configure(hosts, referer)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(MediaProxyState::new())
        .invoke_handler(tauri::generate_handler![
            open_url,
            app_info,
            http_request,
            set_media_proxy
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch Tiny Reader");
}
