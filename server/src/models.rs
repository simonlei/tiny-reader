use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 一个 RSS 订阅源
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feed {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub site_url: Option<String>,
    pub description: Option<String>,
    /// 分组/目录名（OPML 导入时会带），空字符串表示未分组
    pub category: String,
    pub last_fetched_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    /// 未读数（列表接口才会填充）
    #[serde(default)]
    pub unread_count: i64,
    /// 文章总数（列表接口才会填充）
    #[serde(default)]
    pub total_count: i64,
}

/// 一篇文章
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub id: i64,
    pub feed_id: i64,
    pub guid: String,
    pub title: String,
    pub author: Option<String>,
    pub url: Option<String>,
    /// 摘要（HTML）
    pub summary: Option<String>,
    /// 正文（HTML），可能为 None，此时前端退回到 summary
    pub content: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
    #[serde(default)]
    pub is_read: bool,
    #[serde(default)]
    pub is_starred: bool,
    /// 所属源的标题（列表接口附带，省一次请求）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed_site_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NewFeed {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateFeed {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

/// 文章列表的查询条件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArticleQuery {
    pub feed_id: Option<i64>,
    pub unread_only: bool,
    pub starred_only: bool,
    pub limit: i64,
    pub offset: i64,
    pub keyword: Option<String>,
    /// 游标分页：上一页最后一条的排序时间（COALESCE(published_at, fetched_at)）。
    /// 传了它就用 keyset 分页，忽略 offset。未读模式下结果集会随阅读收缩，
    /// offset 分页会跳过文章，必须靠游标保证稳定。
    pub before_time: Option<String>,
    /// 游标分页：上一页最后一条的 id，与 before_time 配对用于打破同时间平局
    pub before_id: Option<i64>,
}

impl Default for ArticleQuery {
    fn default() -> Self {
        Self {
            feed_id: None,
            unread_only: false,
            starred_only: false,
            limit: 50,
            offset: 0,
            keyword: None,
            before_time: None,
            before_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct RefreshStatus {
    pub running: bool,
    pub total: usize,
    pub done: usize,
    pub new_articles: usize,
    pub errors: Vec<FeedError>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FeedError {
    pub feed_id: i64,
    pub title: String,
    pub message: String,
}
