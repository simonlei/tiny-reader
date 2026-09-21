use std::time::Duration;

use chrono::Utc;
use feed_rs::parser;
use reqwest::header::{HeaderMap, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH, USER_AGENT};
use reqwest::Client;

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
}

impl Fetcher {
    pub fn new(
        user_agent: String,
        timeout: Duration,
        rsshub_base: String,
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
        // 逻辑 URL -> 实际请求地址。放在最前面，add_feed / refresh 两条路径都会经过这里。
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
            return Err(format!("HTTP {status}"));
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

        // feed-rs 2.x：用 Builder 指定 base_uri，相对链接会被补全成绝对地址。
        // sanitize_content 默认 true，会顺手清掉正文里的危险 HTML。
        let parsed = parser::Builder::new()
            .base_uri(Some(url))
            .build()
            .parse(&body[..])
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

        Ok(FetchOutcome::Updated(FetchedFeed {
            title,
            site_url,
            description,
            etag,
            last_modified,
            entries,
        }))
    }
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
    use super::resolve_rsshub_url;

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
}
