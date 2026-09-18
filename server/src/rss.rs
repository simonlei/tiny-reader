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
}

impl Fetcher {
    pub fn new(user_agent: String, timeout: Duration) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .timeout(timeout)
            // 部分源会做重定向，允许跟随
            .redirect(reqwest::redirect::Policy::limited(5))
            .user_agent(&user_agent)
            .build()?;
        Ok(Self { client, user_agent })
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
            .get(url)
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
