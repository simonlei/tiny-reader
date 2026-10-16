//! 服务端自升级。
//!
//! 读 GitHub Release 的 latest，版本更高时下载 Linux musl 包，
//! 用资源上的 sha256 digest 校验，只替换当前可执行文件，然后原地重启。
//! 不碰 config.toml 和数据库。开发构建（路径含 `target`）和非 Linux x86_64 只记录日志。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::config::UpdateConfig;

pub const SERVER_ASSET_NAME: &str = "tiny-reader-server-linux-x86_64-musl.tar.gz";
pub const SERVER_BIN_NAME: &str = "tiny-reader-server";
pub const MIN_INTERVAL_SECS: u64 = 300;

const MAX_ARCHIVE_BYTES: usize = 80 * 1024 * 1024;
const STARTUP_DELAY: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedUpdate {
    pub version: String,
    pub tag: String,
    pub asset_url: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateDecision {
    UpToDate,
    Available(PlannedUpdate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyBlock {
    DevBuild,
    UnsupportedPlatform,
}

impl ApplyBlock {
    pub fn reason(self) -> &'static str {
        match self {
            ApplyBlock::DevBuild => "当前是开发构建（路径含 target），跳过替换",
            ApplyBlock::UnsupportedPlatform => "当前不是 Linux x86_64 发布包，跳过替换",
        }
    }
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

pub fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().strip_prefix('v').unwrap_or(raw.trim());
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(remote), Some(current)) => remote > current,
        _ => false,
    }
}

pub fn normalize_repo(repo: &str) -> Result<String> {
    let repo = repo.trim().trim_matches('/');
    let mut parts = repo.split('/');
    let owner = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or("");
    if parts.next().is_some() || owner.is_empty() || name.is_empty() {
        anyhow::bail!("update.repo 必须是 owner/name");
    }
    let ok = |s: &str| {
        !s.is_empty()
            && !s.starts_with('.')
            && !s.ends_with('.')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    };
    if !ok(owner) || !ok(name) {
        anyhow::bail!("update.repo 含有非法字符");
    }
    Ok(format!("{owner}/{name}"))
}

pub fn latest_release_url(repo: &str) -> Result<String> {
    let repo = normalize_repo(repo)?;
    Ok(format!(
        "https://api.github.com/repos/{repo}/releases/latest"
    ))
}

pub fn decide_update(
    release_json: &str,
    current_version: &str,
    repo: &str,
) -> Result<UpdateDecision> {
    let repo = normalize_repo(repo)?;
    let release: GhRelease =
        serde_json::from_str(release_json).context("解析 GitHub Release 失败")?;
    if release.draft || release.prerelease {
        anyhow::bail!("跳过草稿或预发布版本 {}", release.tag_name);
    }
    let tag = release.tag_name.trim();
    let version = tag.strip_prefix('v').context("发布标签必须以 v 开头")?;
    if parse_semver(version).is_none() {
        anyhow::bail!("无法识别的版本号 {tag}");
    }
    if !is_newer(version, current_version) {
        return Ok(UpdateDecision::UpToDate);
    }
    let asset = release
        .assets
        .into_iter()
        .find(|a| a.name == SERVER_ASSET_NAME)
        .context("最新版本没有 Linux 服务端发布包")?;
    let expected_url =
        format!("https://github.com/{repo}/releases/download/{tag}/{SERVER_ASSET_NAME}");
    if asset.browser_download_url != expected_url {
        anyhow::bail!("发布包地址不可信");
    }
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(parse_sha256_digest)
        .context("发布包缺少 sha256 digest，拒绝安装")?;
    Ok(UpdateDecision::Available(PlannedUpdate {
        version: version.to_string(),
        tag: tag.to_string(),
        asset_url: asset.browser_download_url,
        sha256,
    }))
}

pub fn parse_sha256_digest(digest: &str) -> Option<String> {
    let hex = digest.trim().strip_prefix("sha256:")?;
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex.to_ascii_lowercase())
    } else {
        None
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn verify_sha256(bytes: &[u8], expected_hex: &str) -> Result<()> {
    let actual = sha256_hex(bytes);
    if actual.eq_ignore_ascii_case(expected_hex.trim()) {
        Ok(())
    } else {
        anyhow::bail!("sha256 不匹配");
    }
}

pub fn version_output_matches(text: &str, expected: &str) -> bool {
    !expected.is_empty() && text.split_whitespace().any(|word| word == expected)
}

pub fn is_dev_build(exe: &Path) -> bool {
    exe.components().any(|c| c.as_os_str() == "target")
}

pub fn apply_block(exe: &Path) -> Option<ApplyBlock> {
    if is_dev_build(exe) {
        return Some(ApplyBlock::DevBuild);
    }
    if !cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        return Some(ApplyBlock::UnsupportedPlatform);
    }
    None
}

pub fn effective_interval_secs(secs: u64) -> u64 {
    secs.max(MIN_INTERVAL_SECS)
}

pub fn authorization_header(token: &str) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(format!("Bearer {token}"))
    }
}

pub fn update_disabled_by_env(value: Option<&str>) -> bool {
    matches!(
        value.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("1") | Some("true") | Some("yes")
    )
}

pub fn extract_server_binary(archive: &[u8]) -> Result<Vec<u8>> {
    if archive.len() > MAX_ARCHIVE_BYTES {
        anyhow::bail!("发布包过大（{} 字节）", archive.len());
    }
    let decoder = flate2::read::GzDecoder::new(archive);
    let mut tar = tar::Archive::new(decoder);
    let entries = tar.entries().context("无法读取发布包")?;
    for entry in entries {
        let mut entry = entry.context("发布包条目损坏")?;
        let path = entry.path().context("发布包路径无效")?;
        if path.file_name().and_then(|s| s.to_str()) != Some(SERVER_BIN_NAME) {
            continue;
        }
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).context("读取二进制失败")?;
        if buf.is_empty() {
            anyhow::bail!("发布包里的二进制是空的");
        }
        if buf.len() > MAX_ARCHIVE_BYTES {
            anyhow::bail!("二进制过大");
        }
        return Ok(buf);
    }
    anyhow::bail!("发布包里没有 {SERVER_BIN_NAME}")
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_default();
    name.push(suffix);
    path.with_file_name(name)
}

pub fn write_staged(current_exe: &Path, bytes: &[u8]) -> Result<PathBuf> {
    let staged = sibling(current_exe, ".new");
    if let Some(parent) = staged.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建目录失败: {}", parent.display()))?;
        }
    }
    std::fs::write(&staged, bytes).with_context(|| format!("写入 {} 失败", staged.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))
            .with_context(|| format!("设置可执行权限失败: {}", staged.display()))?;
    }
    Ok(staged)
}

pub fn swap_in(current_exe: &Path, staged: &Path) -> Result<()> {
    if !staged.is_file() {
        anyhow::bail!("暂存文件不存在: {}", staged.display());
    }
    let backup = sibling(current_exe, ".old");
    if backup.exists() {
        std::fs::remove_file(&backup)
            .with_context(|| format!("删除旧备份失败: {}", backup.display()))?;
    }
    std::fs::rename(current_exe, &backup).with_context(|| {
        format!(
            "无法挪走当前可执行文件 {}，请确认对所在目录有写权限",
            current_exe.display()
        )
    })?;
    if let Err(err) = std::fs::rename(staged, current_exe) {
        if let Err(rollback) = std::fs::rename(&backup, current_exe) {
            anyhow::bail!(
                "替换失败 ({err})，回滚也失败 ({rollback})。备份留在 {}",
                backup.display()
            );
        }
        return Err(err).context("替换可执行文件失败，已回滚");
    }
    Ok(())
}

pub fn spawn(cfg: UpdateConfig) {
    tokio::spawn(async move {
        tokio::time::sleep(STARTUP_DELAY).await;
        loop {
            if let Err(err) = check_and_apply(&cfg).await {
                tracing::warn!("自动升级失败: {err:#}");
            }
            let wait = effective_interval_secs(cfg.interval_secs);
            tokio::time::sleep(Duration::from_secs(wait)).await;
        }
    });
}

async fn check_and_apply(cfg: &UpdateConfig) -> Result<()> {
    let repo = normalize_repo(&cfg.repo)?;
    let url = latest_release_url(&repo)?;
    let client = github_client()?;
    let body = github_get(
        &client,
        &url,
        &cfg.github_token,
        "application/vnd.github+json",
    )
    .await?;
    let text = String::from_utf8(body).context("GitHub 响应不是 UTF-8")?;
    match decide_update(&text, env!("CARGO_PKG_VERSION"), &repo)? {
        UpdateDecision::UpToDate => {
            tracing::info!("服务端已是最新版本 {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        UpdateDecision::Available(plan) => apply_plan(&client, cfg, &plan).await,
    }
}

async fn apply_plan(
    client: &reqwest::Client,
    cfg: &UpdateConfig,
    plan: &PlannedUpdate,
) -> Result<()> {
    let exe = std::env::current_exe().context("无法定位当前可执行文件")?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    if let Some(block) = apply_block(&exe) {
        tracing::info!("发现新版本 {}，{}", plan.version, block.reason());
        return Ok(());
    }

    tracing::info!("发现新版本 {}，开始下载", plan.version);
    let archive = github_get(
        client,
        &plan.asset_url,
        &cfg.github_token,
        "application/octet-stream",
    )
    .await?;
    verify_sha256(&archive, &plan.sha256).context("发布包校验失败，已放弃安装")?;
    let binary = extract_server_binary(&archive)?;
    let staged = write_staged(&exe, &binary)?;
    let version_text = match run_version(&staged) {
        Ok(text) => text,
        Err(err) => {
            let _ = std::fs::remove_file(&staged);
            return Err(err);
        }
    };
    if !version_output_matches(&version_text, &plan.version) {
        let _ = std::fs::remove_file(&staged);
        anyhow::bail!("新二进制版本不匹配，--version 输出: {version_text}");
    }
    swap_in(&exe, &staged)?;
    tracing::info!("已替换为 {}，正在重启", plan.version);
    reexec(&exe)
}

fn run_version(path: &Path) -> Result<String> {
    let output = std::process::Command::new(path)
        .arg("--version")
        .output()
        .with_context(|| format!("无法执行 {} --version", path.display()))?;
    if !output.status.success() {
        anyhow::bail!("新二进制 --version 失败: {}", output.status);
    }
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

fn reexec(exe: &Path) -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = std::process::Command::new(exe).args(&args).exec();
        anyhow::bail!("重启失败: {err}。新版本已就位，请手动重启服务端");
    }
    #[cfg(not(unix))]
    {
        let _ = (exe, args);
        anyhow::bail!("当前平台不支持原地重启。新版本已就位，请手动重启服务端");
    }
}

fn github_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::limited(10))
        // 关掉自动解压，保证校验的是 GitHub digest 对应的原始字节。
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .build()
        .context("创建 HTTP 客户端失败")
}

async fn github_get(
    client: &reqwest::Client,
    url: &str,
    token: &str,
    accept: &str,
) -> Result<Vec<u8>> {
    let mut req = client
        .get(url)
        .header(reqwest::header::ACCEPT, accept)
        .header(reqwest::header::USER_AGENT, user_agent())
        // 发布包本身已是 tar.gz。禁止再套一层 Content-Encoding，
        // 否则校验的是解压后的字节，对不上 GitHub 的 digest。
        .header(reqwest::header::ACCEPT_ENCODING, "identity");
    if let Some(auth) = authorization_header(token) {
        req = req.header(reqwest::header::AUTHORIZATION, auth);
    }
    let resp = req
        .send()
        .await
        .with_context(|| format!("请求失败: {url}"))?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let snippet: String = text.chars().take(200).collect();
        anyhow::bail!("GitHub 返回 {status}: {snippet}");
    }
    if let Some(len) = resp.content_length() {
        if len > MAX_ARCHIVE_BYTES as u64 {
            anyhow::bail!("响应过大: {len} 字节");
        }
    }
    let bytes = resp.bytes().await.context("读取响应失败")?;
    if bytes.len() > MAX_ARCHIVE_BYTES {
        anyhow::bail!("响应过大: {} 字节", bytes.len());
    }
    Ok(bytes.to_vec())
}

fn user_agent() -> String {
    format!(
        "TinyReader-Server/{} (+https://github.com/simonlei/tiny-reader)",
        env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn release_json(tag: &str, asset_url: &str, digest: &str) -> String {
        format!(
            r#"{{
                "tag_name": "{tag}",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {{
                        "name": "latest.json",
                        "browser_download_url": "https://github.com/simonlei/tiny-reader/releases/download/{tag}/latest.json",
                        "digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    }},
                    {{
                        "name": "{asset}",
                        "browser_download_url": "{asset_url}",
                        "digest": "{digest}"
                    }}
                ]
            }}"#,
            tag = tag,
            asset = SERVER_ASSET_NAME,
            asset_url = asset_url,
            digest = digest,
        )
    }

    fn asset_url(tag: &str) -> String {
        format!(
            "https://github.com/simonlei/tiny-reader/releases/download/{tag}/{name}",
            tag = tag,
            name = SERVER_ASSET_NAME,
        )
    }

    #[test]
    fn semver_orders_double_digits_numerically() {
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(is_newer("v0.2.0", "0.1.13"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("0.1.13", "0.1.13"));
        assert!(!is_newer("v0.1.13", "0.1.13"));
        assert!(!is_newer("0.1.2", "0.1.10"));
        assert_eq!(parse_semver("v0.1.10"), Some((0, 1, 10)));
        assert_eq!(parse_semver("0.1.10"), Some((0, 1, 10)));
        assert_eq!(parse_semver("0.1.10-beta"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn repo_must_be_owner_slash_name() {
        assert_eq!(
            normalize_repo(" simonlei/tiny-reader ").unwrap(),
            "simonlei/tiny-reader"
        );
        assert!(normalize_repo("simonlei/tiny-reader/extra").is_err());
        assert!(normalize_repo("../evil").is_err());
        assert!(normalize_repo("").is_err());
        assert!(normalize_repo("simonlei").is_err());
        assert_eq!(
            latest_release_url("simonlei/tiny-reader").unwrap(),
            "https://api.github.com/repos/simonlei/tiny-reader/releases/latest"
        );
    }

    #[test]
    fn newer_release_plans_exact_asset() {
        let tag = "v0.2.0";
        let url = asset_url(tag);
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let decision = decide_update(
            &release_json(tag, &url, digest),
            "0.1.13",
            "simonlei/tiny-reader",
        )
        .unwrap();
        assert_eq!(
            decision,
            UpdateDecision::Available(PlannedUpdate {
                version: "0.2.0".into(),
                tag: "v0.2.0".into(),
                asset_url: url,
                sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
            })
        );
    }

    #[test]
    fn same_or_older_release_is_up_to_date() {
        let tag = "v0.1.13";
        let url = asset_url(tag);
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let json = release_json(tag, &url, digest);
        assert_eq!(
            decide_update(&json, "0.1.13", "simonlei/tiny-reader").unwrap(),
            UpdateDecision::UpToDate
        );
        let older = release_json("v0.1.12", &asset_url("v0.1.12"), digest);
        assert_eq!(
            decide_update(&older, "0.1.13", "simonlei/tiny-reader").unwrap(),
            UpdateDecision::UpToDate
        );
    }

    #[test]
    fn rejects_untrusted_or_incomplete_release() {
        let digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let evil = release_json(
            "v0.2.0",
            "https://evil.example/tiny-reader-server-linux-x86_64-musl.tar.gz",
            digest,
        );
        assert!(decide_update(&evil, "0.1.0", "simonlei/tiny-reader").is_err());

        let missing_digest = release_json("v0.2.0", &asset_url("v0.2.0"), "");
        assert!(decide_update(&missing_digest, "0.1.0", "simonlei/tiny-reader").is_err());

        let prerelease = r#"{
            "tag_name": "v0.2.0",
            "draft": false,
            "prerelease": true,
            "assets": []
        }"#;
        assert!(decide_update(prerelease, "0.1.0", "simonlei/tiny-reader").is_err());
        assert!(decide_update("not-json", "0.1.0", "simonlei/tiny-reader").is_err());

        let no_asset = r#"{
            "tag_name": "v0.2.0",
            "draft": false,
            "prerelease": false,
            "assets": []
        }"#;
        assert!(decide_update(no_asset, "0.1.0", "simonlei/tiny-reader").is_err());

        let wrong_tag = release_json("v0.2.0", &asset_url("v0.1.0"), digest);
        assert!(decide_update(&wrong_tag, "0.1.0", "simonlei/tiny-reader").is_err());
    }

    #[test]
    fn sha256_known_vector_and_digest_prefix() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert!(verify_sha256(
            b"abc",
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD"
        )
        .is_ok());
        assert!(verify_sha256(
            b"abc",
            "0000000000000000000000000000000000000000000000000000000000000000"
        )
        .is_err());
        assert_eq!(
            parse_sha256_digest(
                "sha256:dab94837fd06d2905f95149f5f788c3b6962c502e0519a23cc6f96a964ae991f"
            ),
            Some("dab94837fd06d2905f95149f5f788c3b6962c502e0519a23cc6f96a964ae991f".into())
        );
        assert_eq!(parse_sha256_digest("md5:abcd"), None);
        assert_eq!(parse_sha256_digest("sha256:abc"), None);
    }

    #[test]
    fn version_output_matches_clap_line() {
        assert!(version_output_matches(
            "tiny-reader-server 0.2.0\n",
            "0.2.0"
        ));
        assert!(!version_output_matches("tiny-reader-server 0.2.0\n", "0.2"));
        assert!(!version_output_matches(
            "tiny-reader-server 0.2.0\n",
            "0.20.0"
        ));
    }

    #[test]
    fn dev_build_and_foreign_platform_do_not_replace() {
        let dev = Path::new("/opt/tiny-reader/target/release/tiny-reader-server");
        assert!(is_dev_build(dev));
        assert_eq!(apply_block(dev), Some(ApplyBlock::DevBuild));

        let installed = Path::new("/usr/local/bin/tiny-reader-server");
        assert!(!is_dev_build(installed));
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert_eq!(apply_block(installed), None);
        } else {
            assert_eq!(
                apply_block(installed),
                Some(ApplyBlock::UnsupportedPlatform)
            );
        }
    }

    #[test]
    fn interval_never_below_five_minutes() {
        assert_eq!(effective_interval_secs(1), 300);
        assert_eq!(effective_interval_secs(0), 300);
        assert_eq!(effective_interval_secs(21_600), 21_600);
    }

    #[test]
    fn token_header_omitted_when_empty() {
        assert_eq!(authorization_header("  "), None);
        assert_eq!(
            authorization_header("ghp_example").as_deref(),
            Some("Bearer ghp_example")
        );
    }

    #[test]
    fn env_disable_accepts_common_truthy_values() {
        assert!(update_disabled_by_env(Some("1")));
        assert!(update_disabled_by_env(Some("true")));
        assert!(update_disabled_by_env(Some(" YES ")));
        assert!(!update_disabled_by_env(Some("0")));
        assert!(!update_disabled_by_env(None));
    }

    #[test]
    fn missing_update_section_defaults_on() {
        let cfg: crate::config::Config =
            toml::from_str("[server]\nhost = \"127.0.0.1\"\n").unwrap();
        assert!(cfg.update.enabled);
        assert_eq!(cfg.update.interval_secs, 6 * 60 * 60);
        assert_eq!(cfg.update.repo, "simonlei/tiny-reader");
        assert!(cfg.update.github_token.is_empty());

        let partial: crate::config::Config = toml::from_str("[update]\nenabled = false\n").unwrap();
        assert!(!partial.update.enabled);
        assert_eq!(partial.update.interval_secs, 6 * 60 * 60);
        assert_eq!(partial.update.repo, "simonlei/tiny-reader");
    }

    fn tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut raw = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut raw);
            for (name, bytes) in files {
                let mut header = tar::Header::new_gnu();
                header.set_path(name).unwrap();
                header.set_size(bytes.len() as u64);
                header.set_mode(0o755);
                header.set_cksum();
                builder.append(&header, *bytes).unwrap();
            }
            builder.finish().unwrap();
        }
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        enc.write_all(&raw).unwrap();
        enc.finish().unwrap()
    }

    #[test]
    fn extract_only_the_server_binary() {
        let archive = tar_gz(&[
            ("config.example.toml", b"token = \"do-not-touch\""),
            ("tiny-reader-server", b"binary-bytes"),
        ]);
        assert_eq!(extract_server_binary(&archive).unwrap(), b"binary-bytes");

        let nested = tar_gz(&[("./dist/tiny-reader-server", b"nested")]);
        assert_eq!(extract_server_binary(&nested).unwrap(), b"nested");

        let empty = tar_gz(&[("config.example.toml", b"only-config")]);
        assert!(extract_server_binary(&empty).is_err());
        assert!(extract_server_binary(b"not-a-gzip").is_err());
    }

    #[test]
    fn swap_keeps_backup_and_refuses_missing_staged_file() {
        let dir = std::env::temp_dir().join(format!(
            "tiny-reader-upd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _cleanup = TempDir(dir.clone());
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("tiny-reader-server");
        std::fs::write(&exe, b"old-binary").unwrap();

        let missing = dir.join("missing");
        assert!(swap_in(&exe, &missing).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old-binary");

        let staged = write_staged(&exe, b"new-binary").unwrap();
        assert_eq!(std::fs::read(&staged).unwrap(), b"new-binary");
        swap_in(&exe, &staged).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new-binary");
        assert_eq!(
            std::fs::read(dir.join("tiny-reader-server.old")).unwrap(),
            b"old-binary"
        );
        assert!(!staged.exists());
    }

    struct TempDir(std::path::PathBuf);
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
