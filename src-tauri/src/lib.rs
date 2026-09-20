use serde::Serialize;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![open_url, app_info])
        .run(tauri::generate_context!())
        .expect("failed to launch Tiny Reader");
}
