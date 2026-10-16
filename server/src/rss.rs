use std::time::Duration;

use chrono::Utc;
use feed_rs::parser;
use reqwest::header::{
    HeaderMap, HeaderValue, COOKIE, IF_MODIFIED_SINCE, IF_NONE_MATCH, REFERER, USER_AGENT,
};
use reqwest::Client;

use crate::daily::{self, DailyEdition, DailyRequest};
use crate::db::{FeedTarget, NewArticle};

/// 拉取结果
pub enum FetchOutcome {
    /// 有更新，返回解析后的内容
    Updated(FetchedFeed),
    /// 服务端返回 304，内容没变
    NotModified,
}

#[derive(Debug)]
pub struct FetchedFeed {
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub description: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub entries: Vec<NewArticle>,
}

pub struct Fetcher {
    client: Client,
    user_agent: String,
    /// 自部署 RSSHub 实例地址（去尾斜杠），为空表示未启用
    rsshub_base: String,
    /// 煎蛋登录 Cookie。只发给 jandan.net，用来翻页；空字符串表示不登录。
    jandan_cookie: String,
}

impl Fetcher {
    pub fn new(
        user_agent: String,
        timeout: Duration,
        rsshub_base: String,
        jandan_cookie: String,
    ) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(timeout)
            // 部分源会做重定向，允许跟随
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(&user_agent)
            .build()?;
        Ok(Self {
            client,
            user_agent,
            rsshub_base,
            jandan_cookie,
        })
    }

    /// 探测并解析一个源（新增源时用，不走条件请求）
    pub async fn discover(&self, url: &str) -> Result<FetchedFeed, String> {
        self.fetch(url, None, None).await.and_then(|o| match o {
            FetchOutcome::Updated(f) => Ok(f),
            FetchOutcome::NotModified => Err("服务器返回 304，无法获取内容".into()),
        })
    }

    /// 按源上一次的 etag / last_modified 做条件 GET
    pub async fn fetch(
        &self,
        url: &str,
        etag: Option<&str>,
        last_modified: Option<&str>,
    ) -> Result<FetchOutcome, String> {
        // 日报不走 RSS 解析，也不用上次的 ETag：每天请求的地址都不一样。
        if daily::is_stored_daily(url) {
            return self.fetch_daily(url).await;
        }

        // 逻辑 URL -> 实际请求地址。放在最前面，add_feed / refresh 两条路径都会经过这里。
        // 记住「原本是不是 rsshub://」，出错时才能把路由回显给用户。
        let logical_route = url.strip_prefix("rsshub://");
        let url = resolve_rsshub_url(url, &self.rsshub_base)?;

        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&self.user_agent).map_err(|e| e.to_string())?,
        );
        if let Some(e) = etag.filter(|s| !s.trim().is_empty()) {
            headers.insert(
                IF_NONE_MATCH,
                HeaderValue::from_str(e).map_err(|_| "ETag 含非法字符".to_string())?,
            );
        }
        if let Some(lm) = last_modified.filter(|s| !s.trim().is_empty()) {
            headers.insert(
                IF_MODIFIED_SINCE,
                HeaderValue::from_str(lm).map_err(|_| "Last-Modified 含非法字符".to_string())?,
            );
        }

        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("请求失败: {e}"))?;

        let status = resp.status();
        if status == reqwest::StatusCode::NOT_MODIFIED {
            return Ok(FetchOutcome::NotModified);
        }
        if !status.is_success() {
            // RSSHub 出错时返回的是 HTML 错误页，只看状态码完全看不出原因，
            // 把页面里的 Error Message 挖出来一起返回。
            let body = resp.bytes().await.unwrap_or_default();
            let detail = describe_http_error(&body);
            let route_hint = logical_route
                .map(|r| format!("（RSSHub 路由 /{}）", r.trim_start_matches('/')))
                .unwrap_or_default();
            return Err(match detail {
                Some(d) => format!("HTTP {status}：{d}{route_hint}"),
                None => format!("HTTP {status}{route_hint}"),
            });
        }

        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let last_modified = resp
            .headers()
            .get(reqwest::header::LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let body = resp
            .bytes()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;

        parse_feed_body(&url, &body, etag, last_modified).map(FetchOutcome::Updated)
    }

    /// 日报：把 `{date}` 换成东八区的昨天再抓。
    /// 煎蛋的页面是空壳，正文在热评接口里，而且不带 Referer 会返回空数据。
    async fn fetch_daily(&self, stored: &str) -> Result<FetchOutcome, String> {
        let plan = daily::plan_daily_request(stored, Utc::now())?;
        let edition = match plan {
            DailyRequest::Jandan { page_url, .. } => {
                let day = iso_date_in(&page_url)
                    .ok_or_else(|| "日报地址里没有可识别的日期".to_string())?;
                let mut entries = Vec::new();
                let mut seen = std::collections::HashSet::new();
                let mut total = 0i64;
                let mut logged_in = false;
                for requested in 1u32..=20 {
                    let url = daily::jandan_api_url(day, requested);
                    let cookie = jandan_cookie_for(&self.jandan_cookie, &url);
                    let body = self.get_text(&url, Some(&page_url), cookie).await?;
                    let page = daily::parse_jandan_page(&body)?;
                    if requested > 1 && page.page != requested as i64 {
                        // 未登录时第 2 页会退回第 1 页，丢掉这次结果
                        break;
                    }
                    if requested == 1 && page.entries.is_empty() {
                        return Err("这一天还没有日报内容".into());
                    }
                    total = page.total;
                    logged_in = page.is_login;
                    let page_len = page.entries.len();
                    let page_size = page.page_size;
                    let response_page = page.page;
                    let mut added = 0usize;
                    for entry in page.entries {
                        if seen.insert(entry.guid.clone()) {
                            entries.push(entry);
                            added += 1;
                        }
                    }
                    if added == 0
                        || !daily::jandan_has_more(
                            requested as i64,
                            response_page,
                            page_len,
                            page_size,
                            total,
                            entries.len(),
                        )
                    {
                        break;
                    }
                }
                if entries.is_empty() {
                    return Err("这一天还没有日报内容".into());
                }
                daily::jandan_edition(entries, total, logged_in)
            }
            DailyRequest::Page { page_url } => {
                let body = self.get_text(&page_url, Some(&page_url), None).await?;
                if looks_like_feed(&body) {
                    if let Ok(feed) = parse_feed_body(&page_url, body.as_bytes(), None, None) {
                        return Ok(FetchOutcome::Updated(feed));
                    }
                }
                let day = iso_date_in(&page_url)
                    .ok_or_else(|| "日报地址里没有可识别的日期".to_string())?;
                daily::parse_html_daily(&page_url, &body, &daily::published_stamp(day))?
            }
        };
        Ok(FetchOutcome::Updated(edition_to_fetched(edition)))
    }

    async fn get_text(
        &self,
        url: &str,
        referer: Option<&str>,
        cookie: Option<&str>,
    ) -> Result<String, String> {
        let mut req = self.client.get(url).header(
            reqwest::header::ACCEPT,
            "application/json, text/html, application/xhtml+xml, */*",
        );
        if let Some(r) = referer {
            req = req.header(REFERER, r);
        }
        if let Some(c) = cookie.filter(|s| !s.trim().is_empty()) {
            req = req.header(COOKIE, c);
        }
        let resp = req.send().await.map_err(|e| format!("请求失败: {e}"))?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.bytes().await.unwrap_or_default();
            let detail = describe_http_error(&body);
            return Err(match detail {
                Some(d) => format!("HTTP {status}：{d}"),
                None => format!("HTTP {status}"),
            });
        }
        resp.text().await.map_err(|e| format!("读取响应失败: {e}"))
    }
}

fn jandan_cookie_for<'a>(cookie: &'a str, url: &str) -> Option<&'a str> {
    let cookie = cookie.trim();
    if cookie.is_empty() {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?;
    if host == "jandan.net" || host.ends_with(".jandan.net") {
        Some(cookie)
    } else {
        None
    }
}

fn edition_to_fetched(edition: DailyEdition) -> FetchedFeed {
    FetchedFeed {
        title: edition.title,
        site_url: edition.site_url,
        description: edition.description,
        etag: None,
        last_modified: None,
        entries: edition.entries,
    }
}

fn looks_like_feed(body: &str) -> bool {
    let t = body.trim_start_matches('\u{feff}').trim_start();
    let head: String = t.chars().take(64).collect();
    let head = head.to_ascii_lowercase();
    head.starts_with("<?xml")
        || head.starts_with("<rss")
        || head.starts_with("<feed")
        || head.starts_with('{')
}

fn iso_date_in(url: &str) -> Option<chrono::NaiveDate> {
    let b = url.as_bytes();
    if b.len() < 10 {
        return None;
    }
    for i in 0..=b.len() - 10 {
        if !url.is_char_boundary(i) {
            continue;
        }
        let Some(slice) = url.get(i..i + 10) else {
            continue;
        };
        if let Ok(d) = chrono::NaiveDate::parse_from_str(slice, "%Y-%m-%d") {
            return Some(d);
        }
    }
    None
}

fn parse_feed_body(
    url: &str,
    body: &[u8],
    etag: Option<String>,
    last_modified: Option<String>,
) -> Result<FetchedFeed, String> {
    // feed-rs 2.x：用 Builder 指定 base_uri，相对链接会被补全成绝对地址。
    // sanitize_content 默认 true，会顺手清掉正文里的危险 HTML。
    let parsed = parser::Builder::new()
        .base_uri(Some(url))
        .build()
        .parse(body)
        .map_err(|e| format!("解析 feed 失败: {e}"))?;

    let title = parsed.title.as_ref().map(|t| t.content.trim().to_string());
    let description = parsed
        .description
        .as_ref()
        .map(|t| t.content.trim().to_string())
        .filter(|s| !s.is_empty());
    let site_url = parsed
        .links
        .iter()
        .find(|l| l.rel.as_deref() == Some("alternate"))
        .or_else(|| parsed.links.first())
        .map(|l| l.href.clone());
    let entries = parsed.entries.iter().map(entry_to_article).collect();

    Ok(FetchedFeed {
        title,
        site_url,
        description,
        etag,
        last_modified,
        entries,
    })
}

fn entry_to_article(e: &feed_rs::model::Entry) -> NewArticle {
    // guid：优先用 id，其次用链接，最后退回标题+时间
    let guid = if !e.id.trim().is_empty() {
        e.id.trim().to_string()
    } else if let Some(link) = first_link(e) {
        link.clone()
    } else {
        let title = e.title.as_ref().map(|t| t.content.as_str()).unwrap_or("");
        format!(
            "{title}#{}",
            e.updated.map(|d| d.to_rfc3339()).unwrap_or_default()
        )
    };

    let title = e
        .title
        .as_ref()
        .map(|t| t.content.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(无标题)".to_string());

    // 注意：RSS 的 <author> 在 feed-rs 里文本被放进 Person.email，
    // Person.name 只是角色名（字面量 "author"），别把它当作者名显示出来。
    let author = e.authors.iter().find_map(|p| {
        let name = p.name.trim();
        let by_name = (!name.is_empty() && name != "author").then(|| name.to_string());
        by_name.or_else(|| {
            p.email
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
    });

    let url = first_link(e).cloned();

    let summary = e
        .summary
        .as_ref()
        .map(|t| t.content.clone())
        .filter(|s| !s.trim().is_empty());

    let content = e
        .content
        .as_ref()
        .and_then(|c| c.body.clone())
        .filter(|s| !s.trim().is_empty());

    let published_at = e
        .published
        .or(e.updated)
        .map(|d| d.with_timezone(&Utc).to_rfc3339());

    NewArticle {
        guid,
        title,
        author,
        url,
        summary,
        content,
        published_at,
    }
}

fn first_link(e: &feed_rs::model::Entry) -> Option<&String> {
    let links = &e.links;
    links
        .iter()
        .find(|l| l.rel.as_deref() == Some("alternate"))
        .or_else(|| links.first())
        .map(|l| &l.href)
}

/// 把 feeds.url 里存的逻辑地址解析成真正要请求的地址。
///
/// - `rsshub://zhihu/daily` + base `http://127.0.0.1:1200` -> `http://127.0.0.1:1200/zhihu/daily`
/// - 其它地址原样返回
pub fn resolve_rsshub_url(url: &str, base: &str) -> Result<String, String> {
    let route = match url.strip_prefix("rsshub://") {
        Some(r) => r,
        None => return Ok(url.to_string()),
    };
    if base.trim().is_empty() {
        return Err("该订阅源依赖 RSSHub，但服务端未配置 rsshub.base_url".to_string());
    }
    let route = route.trim_start_matches('/');
    if route.is_empty() {
        return Err("RSSHub 路由为空".to_string());
    }
    Ok(format!("{}/{}", base, route))
}

/// 非 2xx 响应里尽量挖出一句人话，别只报 "HTTP 503"。
fn describe_http_error(body: &[u8]) -> Option<String> {
    const MAX_SCAN: usize = 64 * 1024;
    let text = String::from_utf8_lossy(&body[..body.len().min(MAX_SCAN)]);
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // 不是 HTML（多半是网关/反爬返回的纯文本），原样摘一段
    if !text.starts_with('<') {
        return Some(truncate_chars(text, 120));
    }
    // RSSHub 错误页：<p class="message">Error Message:<br/><code ...>分类不存在</code></p>
    if let Some(i) = text.find("Error Message") {
        if let Some(msg) = extract_tag_text(&text[i..], "code") {
            // RSSHub 的消息形如 "Error: 分类不存在"，前缀留着是废话
            let msg = unescape_html(&msg).trim().to_string();
            let msg = msg
                .strip_prefix("Error:")
                .unwrap_or(&msg)
                .trim()
                .to_string();
            if !msg.is_empty() {
                return Some(truncate_chars(&msg, 120));
            }
        }
    }
    // 退化：拿 <title> 当一句描述
    text.find("<title")
        .and_then(|i| extract_tag_text(&text[i..], "title"))
        .map(|t| unescape_html(&t).trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|t| truncate_chars(&t, 120))
}

/// 从 `html`（开头处应为目标标签）里取出标签内的文本
fn extract_tag_text(html: &str, tag: &str) -> Option<String> {
    let open_start = html.find(&format!("<{tag}"))?;
    let open_end = html[open_start..].find('>')? + open_start + 1;
    let close_start = html[open_end..].find(&format!("</{tag}>"))? + open_end;
    Some(html[open_end..close_start].to_string())
}

fn unescape_html(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

fn truncate_chars(s: &str, max: usize) -> String {
    match s.char_indices().nth(max) {
        Some((i, _)) => format!("{}…", s[..i].trim_end()),
        None => s.to_string(),
    }
}

/// 刷新一个源：拉取 -> 入库 -> 写回元信息
/// 返回新增文章数
pub async fn refresh_one(
    db: &crate::db::Db,
    fetcher: &Fetcher,
    target: &FeedTarget,
    max_articles_per_feed: usize,
) -> Result<usize, String> {
    let outcome = fetcher
        .fetch(
            &target.url,
            target.etag.as_deref(),
            target.last_modified.as_deref(),
        )
        .await?;

    let fetched = match outcome {
        FetchOutcome::NotModified => {
            db.update_feed_after_fetch(target.id, None, None, None, None, None, None)
                .map_err(|e| e.to_string())?;
            return Ok(0);
        }
        FetchOutcome::Updated(f) => f,
    };

    let new_count = db
        .insert_articles(target.id, &fetched.entries)
        .map_err(|e| e.to_string())?;

    db.update_feed_after_fetch(
        target.id,
        fetched.title.as_deref(),
        fetched.site_url.as_deref(),
        fetched.description.as_deref(),
        fetched.etag.as_deref(),
        fetched.last_modified.as_deref(),
        None,
    )
    .map_err(|e| e.to_string())?;

    if max_articles_per_feed > 0 {
        let _ = db.trim_feed(target.id, max_articles_per_feed);
    }

    Ok(new_count)
}

#[cfg(test)]
mod tests {
    use super::{describe_http_error, jandan_cookie_for, resolve_rsshub_url};

    #[test]
    fn resolve_expands_rsshub_route() {
        assert_eq!(
            resolve_rsshub_url("rsshub://zhihu/daily", "http://127.0.0.1:1200").unwrap(),
            "http://127.0.0.1:1200/zhihu/daily"
        );
    }

    /// 库里只存路由，换实例地址后同一个源自动指向新实例
    #[test]
    fn resolve_follows_base_change() {
        let stored = "rsshub://zhihu/daily";
        assert_eq!(
            resolve_rsshub_url(stored, "https://rsshub.app").unwrap(),
            "https://rsshub.app/zhihu/daily"
        );
        assert_eq!(
            resolve_rsshub_url(stored, "https://rss.example.com").unwrap(),
            "https://rss.example.com/zhihu/daily"
        );
    }

    #[test]
    fn resolve_keeps_plain_url() {
        assert_eq!(
            resolve_rsshub_url("https://example.com/feed.xml", "http://127.0.0.1:1200").unwrap(),
            "https://example.com/feed.xml"
        );
    }

    #[test]
    fn resolve_requires_base() {
        assert!(resolve_rsshub_url("rsshub://zhihu/daily", "").is_err());
    }

    /// RSSHub 出错返回的是 HTML 错误页，状态码看不出原因，得把 Error Message 挖出来
    #[test]
    fn describe_extracts_rsshub_error_message() {
        let html = r#"<html><head><title>Welcome to RSSHub!</title></head><body>
<p class="message">Error Message:<br/><code class="mt-2 block">Error: 分类不存在</code></p>
<p class="message">Route: <code>/juejin/category/:category</code></p></body></html>"#;
        assert_eq!(
            describe_http_error(html.as_bytes()).as_deref(),
            Some("分类不存在")
        );
    }

    /// 没有 Error Message 时退化到 <title>
    #[test]
    fn describe_falls_back_to_title() {
        let html = b"<html><head><title>503 Service Unavailable</title></head><body></body></html>";
        assert_eq!(
            describe_http_error(html).as_deref(),
            Some("503 Service Unavailable")
        );
    }

    #[test]
    fn describe_handles_plain_text_and_empty() {
        assert_eq!(
            describe_http_error(b"rate limited").as_deref(),
            Some("rate limited")
        );
        assert_eq!(describe_http_error(b"   ").as_deref(), None);
    }

    #[test]
    fn jandan_cookie_stays_on_jandan() {
        assert_eq!(
            jandan_cookie_for(
                "egg_login_uid=1",
                "https://jandan.net/api/v1/daily-hot/comments"
            ),
            Some("egg_login_uid=1")
        );
        assert_eq!(
            jandan_cookie_for("egg_login_uid=1", "https://img.jandan.net/a.jpg"),
            Some("egg_login_uid=1")
        );
        assert_eq!(
            jandan_cookie_for("egg_login_uid=1", "https://example.com/digest/2026-10-15"),
            None
        );
        assert_eq!(
            jandan_cookie_for("  ", "https://jandan.net/api/v1/daily-hot/comments"),
            None
        );
    }

    /// 需要环境变量 TINY_READER_JANDAN_COOKIE。平时不跑。
    #[tokio::test]
    #[ignore]
    async fn live_jandan_paginates_with_cookie() {
        let cookie = std::env::var("TINY_READER_JANDAN_COOKIE").unwrap_or_default();
        assert!(
            !cookie.trim().is_empty(),
            "需要环境变量 TINY_READER_JANDAN_COOKIE"
        );
        let fetcher = super::Fetcher::new(
            "TinyReader/0.1".into(),
            std::time::Duration::from_secs(30),
            String::new(),
            cookie,
        )
        .unwrap();
        let feed = fetcher
            .discover("daily://https://jandan.net/new/daily/date/{date}")
            .await
            .expect("拉取失败");
        let n = feed.entries.len();
        eprintln!("登录翻页 {} 篇 {}", n, feed.description.unwrap_or_default());
        assert!(n > 10, "登录后应该超过未登录的 10 条，实际 {n}");
    }
}
