use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{header, Method, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::config::Config;
use crate::db::Db;
use crate::models::{Article, ArticleQuery, Feed, FeedError, NewFeed, RefreshStatus, UpdateFeed};
use crate::opml::{self, OpmlEntry};
use crate::rss::Fetcher;

// ------------------------------------------------------------------ 共享状态

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Db>,
    pub config: Arc<Config>,
    pub fetcher: Arc<Fetcher>,
    pub refresh: Arc<Mutex<RefreshInner>>,
    pub token: Arc<String>,
}

#[derive(Debug, Default)]
pub struct RefreshInner {
    pub running: bool,
    pub total: usize,
    pub done: usize,
    pub new_articles: usize,
    pub errors: Vec<FeedError>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl RefreshInner {
    fn snapshot(&self) -> RefreshStatus {
        RefreshStatus {
            running: self.running,
            total: self.total,
            done: self.done,
            new_articles: self.new_articles,
            errors: self.errors.clone(),
            started_at: self.started_at,
            finished_at: self.finished_at,
        }
    }
}

// -------------------------------------------------------------------- 错误类型

pub struct AppError {
    status: StatusCode,
    message: String,
}

impl AppError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: msg.into(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

// ---------------------------------------------------------------------- 鉴权

/// 单用户模式下只认一个 token。
/// 支持 `Authorization: Bearer <token>` 和 `X-Auth-Token: <token>` 两种写法。
async fn auth_middleware(
    State(state): State<AppState>,
    request: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let provided = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.strip_prefix("Bearer ")
                .or_else(|| v.strip_prefix("bearer "))
        })
        .map(|s| s.trim())
        .or_else(|| {
            request
                .headers()
                .get("X-Auth-Token")
                .and_then(|v| v.to_str().ok())
        })
        .unwrap_or("");

    if provided == state.token.as_str() {
        return next.run(request).await;
    }

    tracing::warn!("拒绝未授权请求 ({} {})", request.method(), request.uri());
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({ "error": "invalid token" })),
    )
        .into_response()
}

// ---------------------------------------------------------------------- 路由

pub fn build_router(state: AppState) -> Router {
    let api = Router::new()
        // 订阅源
        .route("/feeds", get(list_feeds).post(add_feed))
        .route("/feeds/export", get(export_opml))
        .route("/feeds/import", post(import_opml))
        .route(
            "/feeds/{id}",
            get(get_feed).put(update_feed).delete(delete_feed),
        )
        .route("/feeds/{id}/refresh", post(refresh_feed))
        // 文章
        .route("/articles", get(list_articles))
        .route("/articles/mark-read", post(mark_read))
        .route("/articles/{id}", get(get_article))
        .route("/articles/{id}/read", post(set_read))
        .route("/articles/{id}/star", post(set_starred))
        // 刷新
        .route("/refresh", post(refresh_all))
        .route("/refresh/status", get(refresh_status))
        // 统计
        .route("/stats", get(stats))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .nest("/api", api)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT])
                .allow_private_network(true),
        )
}

async fn root() -> &'static str {
    "Tiny Reader server. See /health and /api/*"
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "ok": true, "service": "tiny-reader-server" }))
}

// ------------------------------------------------------------------ 订阅源 CRUD

async fn list_feeds(State(st): State<AppState>) -> Result<Json<Vec<Feed>>, AppError> {
    let feeds = st
        .db
        .list_feeds()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(feeds))
}

async fn get_feed(State(st): State<AppState>, Path(id): Path<i64>) -> Result<Json<Feed>, AppError> {
    st.db
        .get_feed(id)
        .map_err(|e| AppError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| AppError::not_found("订阅源不存在"))
}

#[derive(Debug, Deserialize)]
struct AddFeedBody {
    url: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    category: Option<String>,
}

/// 新增订阅源：先探测拉取一次，拿到真实标题并灌入首批文章
async fn add_feed(
    State(st): State<AppState>,
    Json(body): Json<AddFeedBody>,
) -> Result<Json<Feed>, AppError> {
    let url = normalize_url(&body.url).ok_or_else(|| AppError::bad_request("URL 不合法"))?;

    let fetched = st
        .fetcher
        .discover(&url)
        .await
        .map_err(|e| AppError::bad_request(format!("无法读取该订阅源：{e}")))?;

    let title = body
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .or_else(|| fetched.title.clone())
        .unwrap_or_else(|| url.clone());

    let feed = st
        .db
        .insert_feed(&NewFeed {
            url: url.clone(),
            title: Some(title),
            category: body.category.clone(),
        })
        .map_err(|e| AppError::bad_request(format!("添加失败：{e}")))?;

    let new_count = st
        .db
        .insert_articles(feed.id, &fetched.entries)
        .map_err(|e| AppError::internal(e.to_string()))?;

    let _ = st.db.update_feed_after_fetch(
        feed.id,
        fetched.title.as_deref(),
        fetched.site_url.as_deref(),
        fetched.description.as_deref(),
        fetched.etag.as_deref(),
        fetched.last_modified.as_deref(),
        None,
    );

    tracing::info!("新增订阅源 #{} {}（{} 篇）", feed.id, url, new_count);

    st.db
        .get_feed(feed.id)
        .map_err(|e| AppError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| AppError::internal("新增后读取失败"))
}

async fn update_feed(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    Json(patch): Json<UpdateFeed>,
) -> Result<Json<Feed>, AppError> {
    if let Some(u) = &patch.url {
        if normalize_url(u).is_none() {
            return Err(AppError::bad_request("URL 不合法"));
        }
    }
    st.db
        .update_feed(id, &patch)
        .map_err(|e| AppError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| AppError::not_found("订阅源不存在"))
}

async fn delete_feed(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let ok = st
        .db
        .delete_feed(id)
        .map_err(|e| AppError::internal(e.to_string()))?;
    if !ok {
        return Err(AppError::not_found("订阅源不存在"));
    }
    Ok(Json(json!({ "ok": true })))
}

/// 只刷新单个源
async fn refresh_feed(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, AppError> {
    let target = st
        .db
        .list_feed_targets()
        .map_err(|e| AppError::internal(e.to_string()))?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| AppError::not_found("订阅源不存在"))?;

    let max = st.config.storage.max_articles_per_feed;
    match crate::rss::refresh_one(&st.db, &st.fetcher, &target, max).await {
        Ok(n) => Ok(Json(json!({ "ok": true, "new_articles": n }))),
        Err(e) => {
            let _ = st
                .db
                .update_feed_after_fetch(id, None, None, None, None, None, Some(&e));
            Err(AppError::bad_request(e))
        }
    }
}

// ---------------------------------------------------------------------- OPML

async fn export_opml(State(st): State<AppState>) -> Result<Response, AppError> {
    let feeds = st
        .db
        .list_feeds()
        .map_err(|e| AppError::internal(e.to_string()))?;
    let entries: Vec<OpmlEntry> = feeds
        .into_iter()
        .map(|f| OpmlEntry {
            title: f.title,
            url: f.url,
            category: f.category,
        })
        .collect();
    let xml = opml::generate(&entries, "Tiny Reader Subscriptions");
    Response::builder()
        .header(header::CONTENT_TYPE, "text/xml; charset=utf-8")
        .header(
            header::CONTENT_DISPOSITION,
            "attachment; filename=\"tiny-reader-subscriptions.opml\"",
        )
        .body(axum::body::Body::from(xml))
        .map_err(|e| AppError::internal(e.to_string()))
}

#[derive(Debug, Deserialize)]
struct ImportBody {
    opml: Option<String>,
    /// 是否立刻在后台拉取每个新源（会补全标题并灌入文章）
    #[serde(default = "default_true")]
    fetch_now: bool,
}

fn default_true() -> bool {
    true
}

/// 导入 OPML。正文可以是 JSON {"opml": "..."} 也可以是原始 XML 文本。
async fn import_opml(
    State(st): State<AppState>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<serde_json::Value>, AppError> {
    let text = String::from_utf8_lossy(&body).to_string();

    let (xml, fetch_now) = if is_json(&headers) {
        let parsed: ImportBody = serde_json::from_str(&text)
            .map_err(|e| AppError::bad_request(format!("JSON 解析失败：{e}")))?;
        (parsed.opml.clone().unwrap_or_default(), parsed.fetch_now)
    } else {
        (text, true)
    };

    if xml.trim().is_empty() {
        return Err(AppError::bad_request("OPML 内容为空"));
    }

    let entries = opml::parse(&xml).map_err(|e| AppError::bad_request(e.to_string()))?;
    if entries.is_empty() {
        return Err(AppError::bad_request("OPML 里没有找到任何订阅源"));
    }

    let existing: std::collections::HashSet<String> = st
        .db
        .list_feeds()
        .map_err(|e| AppError::internal(e.to_string()))?
        .into_iter()
        .map(|f| f.url.to_lowercase())
        .collect();

    let mut added = 0usize;
    let mut skipped = 0usize;
    let mut new_ids: Vec<i64> = Vec::new();

    for e in entries {
        let url = match normalize_url(&e.url) {
            Some(u) => u,
            None => {
                skipped += 1;
                continue;
            }
        };
        if existing.contains(&url.to_lowercase()) {
            skipped += 1;
            continue;
        }
        let title = if e.title.trim().is_empty() {
            url.clone()
        } else {
            e.title
        };
        match st.db.insert_feed(&NewFeed {
            url: url.clone(),
            title: Some(title),
            category: Some(e.category),
        }) {
            Ok(f) => {
                added += 1;
                new_ids.push(f.id);
            }
            Err(err) => {
                tracing::warn!("导入 {url} 失败：{err}");
                skipped += 1;
            }
        }
    }

    // 后台补全真实标题 + 首批文章，避免导入接口卡太久
    if fetch_now && !new_ids.is_empty() {
        let st2 = st.clone();
        tokio::spawn(async move {
            for id in new_ids {
                let target = match st2
                    .db
                    .list_feed_targets()
                    .map(|v| v.into_iter().find(|t| t.id == id))
                {
                    Ok(Some(t)) => t,
                    _ => continue,
                };
                let max = st2.config.storage.max_articles_per_feed;
                if let Err(e) = crate::rss::refresh_one(&st2.db, &st2.fetcher, &target, max).await {
                    tracing::warn!("导入后拉取 #{} 失败：{e}", id);
                    let _ =
                        st2.db
                            .update_feed_after_fetch(id, None, None, None, None, None, Some(&e));
                }
            }
            tracing::info!("OPML 导入后的后台拉取完成");
        });
    }

    Ok(Json(
        json!({ "ok": true, "added": added, "skipped": skipped }),
    ))
}

fn is_json(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("json"))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------- 文章

#[derive(Debug, Deserialize, Default)]
struct ArticleParams {
    #[serde(default)]
    feed_id: Option<i64>,
    #[serde(default)]
    unread_only: bool,
    #[serde(default)]
    starred_only: bool,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    offset: i64,
    #[serde(default)]
    q: Option<String>,
    /// 游标分页：上一页最后一条的排序时间
    #[serde(default)]
    before_time: Option<String>,
    /// 游标分页：上一页最后一条的 id
    #[serde(default)]
    before_id: Option<i64>,
}

fn default_limit() -> i64 {
    50
}

async fn list_articles(
    State(st): State<AppState>,
    Query(p): Query<ArticleParams>,
) -> Result<Json<ArticlePage>, AppError> {
    let limit = p.limit.clamp(1, 200);
    let q = ArticleQuery {
        feed_id: p.feed_id,
        unread_only: p.unread_only,
        starred_only: p.starred_only,
        limit,
        offset: p.offset.max(0),
        keyword: p.q,
        before_time: p.before_time,
        before_id: p.before_id,
    };
    let items = st
        .db
        .list_articles(&q)
        .map_err(|e| AppError::internal(e.to_string()))?;
    let total = st
        .db
        .count_articles(&q)
        .map_err(|e| AppError::internal(e.to_string()))?;
    // 把本页最后一条的排序键回传，客户端下一页原样带回即可，无需自己算 offset
    let next_cursor = items.last().map(|a| Cursor {
        before_time: a
            .published_at
            .or(Some(a.fetched_at))
            .map(|t| t.to_rfc3339())
            .unwrap_or_default(),
        before_id: a.id,
    });
    Ok(Json(ArticlePage {
        items,
        total,
        offset: q.offset,
        limit,
        next_cursor,
    }))
}

#[derive(Debug, Serialize)]
struct Cursor {
    before_time: String,
    before_id: i64,
}

#[derive(Debug, Serialize)]
struct ArticlePage {
    items: Vec<Article>,
    total: i64,
    offset: i64,
    limit: i64,
    /// 下一页游标；为 null 表示已经没有更多数据
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<Cursor>,
}

async fn get_article(
    State(st): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Article>, AppError> {
    st.db
        .get_article(id)
        .map_err(|e| AppError::internal(e.to_string()))?
        .map(Json)
        .ok_or_else(|| AppError::not_found("文章不存在"))
}

#[derive(Debug, Deserialize)]
struct BoolBody {
    #[serde(default = "default_true")]
    value: bool,
}

async fn set_read(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<BoolBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let value = body.map(|b| b.value).unwrap_or(true);
    let ok = st
        .db
        .set_read(id, value)
        .map_err(|e| AppError::internal(e.to_string()))?;
    if !ok {
        return Err(AppError::not_found("文章不存在"));
    }
    Ok(Json(json!({ "ok": true, "is_read": value })))
}

async fn set_starred(
    State(st): State<AppState>,
    Path(id): Path<i64>,
    body: Option<Json<BoolBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let value = body.map(|b| b.value).unwrap_or(true);
    let ok = st
        .db
        .set_starred(id, value)
        .map_err(|e| AppError::internal(e.to_string()))?;
    if !ok {
        return Err(AppError::not_found("文章不存在"));
    }
    Ok(Json(json!({ "ok": true, "is_starred": value })))
}

#[derive(Debug, Deserialize)]
struct MarkReadBody {
    #[serde(default)]
    feed_id: Option<i64>,
    #[serde(default)]
    ids: Option<Vec<i64>>,
}

async fn mark_read(
    State(st): State<AppState>,
    body: Option<Json<MarkReadBody>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let body = match body {
        Some(Json(b)) => b,
        None => MarkReadBody {
            feed_id: None,
            ids: None,
        },
    };
    let n = match body.ids {
        Some(ids) if !ids.is_empty() => {
            let mut n = 0usize;
            for id in ids {
                if st.db.set_read(id, true).unwrap_or(false) {
                    n += 1;
                }
            }
            n
        }
        _ => st
            .db
            .mark_all_read(body.feed_id)
            .map_err(|e| AppError::internal(e.to_string()))?,
    };
    Ok(Json(json!({ "ok": true, "updated": n })))
}

async fn stats(State(st): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    let (total, unread, starred) = st
        .db
        .stats()
        .map_err(|e| AppError::internal(e.to_string()))?;
    Ok(Json(json!({
        "total": total,
        "unread": unread,
        "starred": starred,
    })))
}

// ---------------------------------------------------------------------- 刷新

async fn refresh_status(State(st): State<AppState>) -> Json<RefreshStatus> {
    let inner = st.refresh.lock().unwrap();
    Json(inner.snapshot())
}

async fn refresh_all(State(st): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    {
        let mut inner = st.refresh.lock().unwrap();
        if inner.running {
            return Err(AppError::bad_request("刷新任务正在运行中"));
        }
        *inner = RefreshInner {
            running: true,
            started_at: Some(Utc::now()),
            ..Default::default()
        };
    }

    let targets = st
        .db
        .list_feed_targets()
        .map_err(|e| AppError::internal(e.to_string()))?;

    {
        let mut inner = st.refresh.lock().unwrap();
        inner.total = targets.len();
    }

    if targets.is_empty() {
        let mut inner = st.refresh.lock().unwrap();
        inner.running = false;
        inner.finished_at = Some(Utc::now());
        return Ok(Json(json!({ "ok": true, "total": 0 })));
    }

    tokio::spawn(async move {
        run_refresh(st).await;
    });

    Ok(Json(json!({ "ok": true, "total": targets.len() })))
}

async fn run_refresh(st: AppState) {
    let targets = match st.db.list_feed_targets() {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("读取订阅源列表失败：{e}");
            let mut inner = st.refresh.lock().unwrap();
            inner.running = false;
            inner.finished_at = Some(Utc::now());
            return;
        }
    };

    let concurrency = st.config.refresh.concurrency.max(1);
    let max = st.config.storage.max_articles_per_feed;
    let sem = Arc::new(tokio::sync::Semaphore::new(concurrency));

    let mut handles = Vec::with_capacity(targets.len());
    for t in targets {
        let st = st.clone();
        let sem = sem.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await;
            let result = crate::rss::refresh_one(&st.db, &st.fetcher, &t, max).await;

            let mut inner = st.refresh.lock().unwrap();
            inner.done += 1;
            match result {
                Ok(n) => inner.new_articles += n,
                Err(e) => {
                    tracing::warn!("刷新 #{} ({}) 失败：{e}", t.id, t.url);
                    let _ =
                        st.db
                            .update_feed_after_fetch(t.id, None, None, None, None, None, Some(&e));
                    inner.errors.push(FeedError {
                        feed_id: t.id,
                        title: t.title,
                        message: e,
                    });
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let mut inner = st.refresh.lock().unwrap();
    inner.running = false;
    inner.finished_at = Some(Utc::now());
    tracing::info!(
        "刷新完成：共 {} 个源，新增 {} 篇，失败 {} 个",
        inner.total,
        inner.new_articles,
        inner.errors.len()
    );
}

/// 定时自动刷新（配置里 auto_interval_secs > 0 时启用）
pub fn spawn_auto_refresh(state: AppState, interval: Duration) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // 第一次 tick 立即返回，跳过它，避免刚启动就打满
        ticker.tick().await;
        loop {
            ticker.tick().await;
            {
                let inner = state.refresh.lock().unwrap();
                if inner.running {
                    continue;
                }
            }
            let targets = match state.db.list_feed_targets() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if targets.is_empty() {
                continue;
            }
            {
                let mut inner = state.refresh.lock().unwrap();
                *inner = RefreshInner {
                    running: true,
                    total: targets.len(),
                    started_at: Some(Utc::now()),
                    ..Default::default()
                };
            }
            tracing::info!("定时刷新开始（{} 个源）", targets.len());
            run_refresh(state.clone()).await;
        }
    });
}

// ---------------------------------------------------------------------- 工具

pub fn normalize_url(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let s = if s.starts_with("http://") || s.starts_with("https://") {
        s.to_string()
    } else if s.starts_with("feed://") {
        s.replacen("feed://", "http://", 1)
    } else if looks_like_host_port(s) {
        // 形如 192.168.1.10:8787/rss，这种裸地址几乎都是局域网 http 服务
        format!("http://{s}")
    } else {
        format!("https://{s}")
    };

    match reqwest::Url::parse(&s) {
        Ok(u) if !u.host_str().unwrap_or("").is_empty() => Some(u.to_string()),
        _ => None,
    }
}

/// 判断是不是「主机:端口」开头的地址（主机名里不会出现冒号，所以这个判断是安全的）
fn looks_like_host_port(s: &str) -> bool {
    let (host, rest) = match s.split_once(':') {
        Some((h, r)) => (h, r),
        None => return false,
    };
    if host.is_empty() || host.contains('/') || host.contains('?') {
        return false;
    }
    // 端口必须是纯数字，且后面要么是路径结束要么接着 / ? #
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let tail = &rest[digits.len()..];
    tail.is_empty() || tail.starts_with('/') || tail.starts_with('?') || tail.starts_with('#')
}

pub fn request_timeout(cfg: &Config) -> Duration {
    Duration::from_secs(cfg.refresh.timeout_secs.max(1))
}
