use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{Article, ArticleQuery, Feed, NewFeed, UpdateFeed};

/// 整个服务端共用一个 SQLite 连接。
/// rusqlite 的 Connection 不是 Send，所以用 Mutex 包一层放进 Arc 共享。
pub struct Db(pub Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建数据目录失败: {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开数据库失败: {}", path.display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .context("设置 WAL 模式失败")?;
        conn.pragma_update(None, "foreign_keys", "ON")
            .context("开启外键约束失败")?;
        conn.pragma_update(None, "busy_timeout", "5000")
            .context("设置 busy_timeout 失败")?;
        let db = Db(Mutex::new(conn));
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.0.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS feeds (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                url             TEXT    NOT NULL UNIQUE,
                title           TEXT    NOT NULL,
                site_url        TEXT,
                description     TEXT,
                category        TEXT    NOT NULL DEFAULT '',
                etag            TEXT,
                last_modified   TEXT,
                last_fetched_at TEXT,
                last_error      TEXT,
                created_at      TEXT    NOT NULL
            );

            CREATE TABLE IF NOT EXISTS articles (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                feed_id       INTEGER NOT NULL REFERENCES feeds(id) ON DELETE CASCADE,
                guid          TEXT    NOT NULL,
                title         TEXT    NOT NULL,
                author        TEXT,
                url           TEXT,
                summary       TEXT,
                content       TEXT,
                published_at  TEXT,
                fetched_at    TEXT    NOT NULL,
                is_read       INTEGER NOT NULL DEFAULT 0,
                is_starred    INTEGER NOT NULL DEFAULT 0,
                UNIQUE (feed_id, guid)
            );

            CREATE INDEX IF NOT EXISTS idx_articles_feed_pub
                ON articles (feed_id, published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_articles_pub
                ON articles (published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_articles_read
                ON articles (is_read);
            CREATE INDEX IF NOT EXISTS idx_articles_star
                ON articles (is_starred);
            "#,
        )
        .context("执行数据库迁移失败")?;
        Ok(())
    }

    // ---------------------------------------------------------------- feeds

    pub fn list_feeds(&self) -> Result<Vec<Feed>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT f.id, f.url, f.title, f.site_url, f.description, f.category,
                   f.last_fetched_at, f.last_error, f.created_at,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.is_read = 0) AS unread_count,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id)                    AS total_count,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.is_starred = 1) AS starred_count
            FROM feeds f
            ORDER BY f.category ASC, lower(f.title) ASC
            "#,
        )?;
        let rows = stmt
            .query_map([], |row| Ok(feed_from_row(row)?))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_feed(&self, id: i64) -> Result<Option<Feed>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT f.id, f.url, f.title, f.site_url, f.description, f.category,
                   f.last_fetched_at, f.last_error, f.created_at,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.is_read = 0) AS unread_count,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id)                    AS total_count,
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id AND a.is_starred = 1) AS starred_count
            FROM feeds f WHERE f.id = ?1
            "#,
        )?;
        let feed = stmt
            .query_row(params![id], |row| Ok(feed_from_row(row)?))
            .optional()?;
        Ok(feed)
    }

    /// 只取刷新任务需要的字段（含 etag / last_modified）
    pub fn list_feed_targets(&self) -> Result<Vec<FeedTarget>> {
        let conn = self.0.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT id, url, title, etag, last_modified FROM feeds ORDER BY id")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(FeedTarget {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    etag: row.get(3)?,
                    last_modified: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn insert_feed(&self, new: &NewFeed) -> Result<Feed> {
        let now = Utc::now().to_rfc3339();
        let title = new.title.clone().unwrap_or_else(|| new.url.clone());
        let category = new.category.clone().unwrap_or_default();
        let conn = self.0.lock().unwrap();
        conn.execute(
            "INSERT INTO feeds (url, title, category, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![new.url.trim(), title.trim(), category.trim(), now],
        )
        .context("新增订阅源失败（可能已存在同名 URL）")?;
        let id = conn.last_insert_rowid();
        drop(conn);
        self.get_feed(id)?.context("刚插入的订阅源不见了")
    }

    pub fn update_feed(&self, id: i64, patch: &UpdateFeed) -> Result<Option<Feed>> {
        if self.get_feed(id)?.is_none() {
            return Ok(None);
        }
        let conn = self.0.lock().unwrap();
        if let Some(url) = &patch.url {
            conn.execute(
                "UPDATE feeds SET url = ?1 WHERE id = ?2",
                params![url.trim(), id],
            )?;
        }
        if let Some(title) = &patch.title {
            conn.execute(
                "UPDATE feeds SET title = ?1 WHERE id = ?2",
                params![title.trim(), id],
            )?;
        }
        if let Some(category) = &patch.category {
            conn.execute(
                "UPDATE feeds SET category = ?1 WHERE id = ?2",
                params![category.trim(), id],
            )?;
        }
        drop(conn);
        self.get_feed(id)
    }

    pub fn delete_feed(&self, id: i64) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let n = conn.execute("DELETE FROM feeds WHERE id = ?1", params![id])?;
        Ok(n > 0)
    }

    /// 刷新完成后写回元信息
    pub fn update_feed_after_fetch(
        &self,
        id: i64,
        title: Option<&str>,
        site_url: Option<&str>,
        description: Option<&str>,
        etag: Option<&str>,
        last_modified: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self.0.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        if let Some(t) = title {
            conn.execute("UPDATE feeds SET title = ?1 WHERE id = ?2", params![t, id])?;
        }
        if let Some(s) = site_url {
            conn.execute(
                "UPDATE feeds SET site_url = ?1 WHERE id = ?2",
                params![s, id],
            )?;
        }
        if let Some(d) = description {
            conn.execute(
                "UPDATE feeds SET description = ?1 WHERE id = ?2",
                params![d, id],
            )?;
        }
        conn.execute(
            "UPDATE feeds SET etag = ?1, last_modified = ?2, last_fetched_at = ?3, last_error = ?4 WHERE id = ?5",
            params![etag, last_modified, now, error, id],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------- articles

    /// 写入文章。同一 `(feed_id, guid)` 已存在则跳过。
    ///
    /// 同一源里目标地址已经出现过的，在标题比较之前就丢掉，不算标题去重，也不打标题去重日志。
    /// 同一批里更早留下的那篇会占住这个地址，后面的同地址条目同样直接忽略。
    ///
    /// 同一源在 6 小时内（按发布时间，没有则按抓取时间）若已有相同或接近的标题，
    /// 后到的那一篇也会被忽略。接近的判定见 [`titles_near_duplicate`]。
    /// Google 新闻标题末尾的「 - 来源」会先去掉再比较。
    pub fn insert_articles(&self, feed_id: i64, items: &[NewArticle]) -> Result<usize> {
        if items.is_empty() {
            return Ok(0);
        }
        let now = Utc::now();
        let now_s = now.to_rfc3339();
        let mut conn = self.0.lock().unwrap();

        // 同一源已有的标题和目标地址。地址相同的在标题比较之前丢掉。
        // 窗口按文章时间而不是「最近抓到的」，所以要带上更早的文章：
        // 源里一条旧闻如果换了 guid 又被送过来，发布时间仍落在 6 小时内就该丢掉。
        let mut seen_urls: HashSet<String> = HashSet::new();
        let mut kept: Vec<TitleStamp> = {
            let mut stmt = conn.prepare(
                "SELECT title, COALESCE(published_at, fetched_at), url FROM articles WHERE feed_id = ?1",
            )?;
            let rows = stmt
                .query_map(params![feed_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            let mut kept = Vec::new();
            for (title, ts, url) in rows {
                if let Some(key) = article_url_key(url.as_deref()) {
                    seen_urls.insert(key);
                }
                if let Some(keys) = title_keys(&title) {
                    kept.push(TitleStamp {
                        title,
                        keys,
                        at: parse_dt(ts).unwrap_or(now),
                    });
                }
            }
            kept
        };

        // 同一批里按时间从早到晚处理，让更早的那篇留下，后面接近的被忽略。
        let mut order: Vec<usize> = (0..items.len()).collect();
        order.sort_by(|&i, &j| {
            article_time(items[i].published_at.as_deref(), now)
                .cmp(&article_time(items[j].published_at.as_deref(), now))
                .then(i.cmp(&j))
        });

        let tx = conn.transaction()?;
        let mut count = 0usize;
        let mut skipped_pairs: Vec<(String, String)> = Vec::new();
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR IGNORE INTO articles
                    (feed_id, guid, title, author, url, summary, content, published_at, fetched_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )?;
            for idx in order {
                let a = &items[idx];
                if let Some(key) = article_url_key(a.url.as_deref()) {
                    if seen_urls.contains(&key) {
                        continue;
                    }
                }
                let at = article_time(a.published_at.as_deref(), now);
                if let Some(keys) = title_keys(&a.title) {
                    if let Some(prev) = kept
                        .iter()
                        .find(|k| within_dedup_window(k.at, at) && title_keys_close(&k.keys, &keys))
                    {
                        skipped_pairs.push((prev.title.clone(), a.title.clone()));
                        continue;
                    }
                    let n = stmt.execute(params![
                        feed_id,
                        a.guid,
                        a.title,
                        a.author,
                        a.url,
                        a.summary,
                        a.content,
                        a.published_at,
                        now_s,
                    ])?;
                    if n > 0 {
                        count += n;
                        if let Some(key) = article_url_key(a.url.as_deref()) {
                            seen_urls.insert(key);
                        }
                        kept.push(TitleStamp {
                            title: a.title.clone(),
                            keys,
                            at,
                        });
                    }
                } else {
                    let n = stmt.execute(params![
                        feed_id,
                        a.guid,
                        a.title,
                        a.author,
                        a.url,
                        a.summary,
                        a.content,
                        a.published_at,
                        now_s,
                    ])?;
                    if n > 0 {
                        if let Some(key) = article_url_key(a.url.as_deref()) {
                            seen_urls.insert(key);
                        }
                    }
                    count += n;
                }
            }
        }
        tx.commit()?;
        let skipped = skipped_pairs.len();
        for (kept_title, ignored_title) in &skipped_pairs {
            tracing::info!(
                feed_id,
                skipped,
                kept = kept_title.as_str(),
                ignored = ignored_title.as_str(),
                "同一源 6 小时内标题相同或接近，已忽略"
            );
        }
        Ok(count)
    }

    pub fn list_articles(&self, q: &ArticleQuery) -> Result<Vec<Article>> {
        let conn = self.0.lock().unwrap();

        let mut sql = String::from(
            r#"
            SELECT a.id, a.feed_id, a.guid, a.title, a.author, a.url, a.summary, a.content,
                   a.published_at, a.fetched_at, a.is_read, a.is_starred,
                   f.title AS feed_title, f.site_url AS feed_site_url
            FROM articles a JOIN feeds f ON f.id = a.feed_id
            WHERE 1 = 1
            "#,
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(fid) = q.feed_id {
            sql.push_str(" AND a.feed_id = ?");
            params.push(Box::new(fid));
        }
        if q.unread_only {
            sql.push_str(" AND a.is_read = 0");
        }
        if q.starred_only {
            sql.push_str(" AND a.is_starred = 1");
        }
        if let Some(kw) = &q.keyword {
            let kw = kw.trim();
            if !kw.is_empty() {
                sql.push_str(" AND (a.title LIKE ? ESCAPE '\\' OR a.summary LIKE ? ESCAPE '\\')");
                let like = format!(
                    "%{}%",
                    kw.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                );
                params.push(Box::new(like.clone()));
                params.push(Box::new(like));
            }
        }
        // 游标（keyset）分页：只取"排在上一页最后一条之后"的记录。
        // 未读过滤下结果集随阅读不断收缩，OFFSET 会跳过中间条目，
        // 因此只要客户端传了 before_time 就一律走 keyset，忽略 offset。
        let use_cursor = q.before_time.is_some();
        if use_cursor {
            sql.push_str(
                " AND (COALESCE(a.published_at, a.fetched_at) < ? \
                 OR (COALESCE(a.published_at, a.fetched_at) = ? AND a.id < ?))",
            );
            let t = q.before_time.clone().unwrap_or_default();
            params.push(Box::new(t.clone()));
            params.push(Box::new(t));
            params.push(Box::new(q.before_id.unwrap_or(i64::MAX)));
        }
        sql.push_str(" ORDER BY COALESCE(a.published_at, a.fetched_at) DESC, a.id DESC LIMIT ?");
        params.push(Box::new(q.limit));
        if !use_cursor {
            sql.push_str(" OFFSET ?");
            params.push(Box::new(q.offset));
        }

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(
                rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
                |row| Ok(article_from_row(row)?),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    pub fn get_article(&self, id: i64) -> Result<Option<Article>> {
        let conn = self.0.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT a.id, a.feed_id, a.guid, a.title, a.author, a.url, a.summary, a.content,
                   a.published_at, a.fetched_at, a.is_read, a.is_starred,
                   f.title AS feed_title, f.site_url AS feed_site_url
            FROM articles a JOIN feeds f ON f.id = a.feed_id
            WHERE a.id = ?1
            "#,
        )?;
        let a = stmt
            .query_row(params![id], |row| Ok(article_from_row(row)?))
            .optional()?;
        Ok(a)
    }

    pub fn set_read(&self, id: i64, read: bool) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let n = conn.execute(
            "UPDATE articles SET is_read = ?1 WHERE id = ?2",
            params![if read { 1 } else { 0 }, id],
        )?;
        Ok(n > 0)
    }

    pub fn set_starred(&self, id: i64, starred: bool) -> Result<bool> {
        let conn = self.0.lock().unwrap();
        let n = conn.execute(
            "UPDATE articles SET is_starred = ?1 WHERE id = ?2",
            params![if starred { 1 } else { 0 }, id],
        )?;
        Ok(n > 0)
    }

    /// 全部标为已读；feed_id 为 None 时表示跨所有源
    pub fn mark_all_read(&self, feed_id: Option<i64>) -> Result<usize> {
        let conn = self.0.lock().unwrap();
        let n = match feed_id {
            Some(fid) => conn.execute(
                "UPDATE articles SET is_read = 1 WHERE is_read = 0 AND feed_id = ?1",
                params![fid],
            )?,
            None => conn.execute("UPDATE articles SET is_read = 1 WHERE is_read = 0", [])?,
        };
        Ok(n)
    }

    pub fn count_articles(&self, q: &ArticleQuery) -> Result<i64> {
        let conn = self.0.lock().unwrap();
        let mut sql = String::from("SELECT COUNT(*) FROM articles a WHERE 1 = 1");
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if let Some(fid) = q.feed_id {
            sql.push_str(" AND a.feed_id = ?");
            params.push(Box::new(fid));
        }
        if q.unread_only {
            sql.push_str(" AND a.is_read = 0");
        }
        if q.starred_only {
            sql.push_str(" AND a.is_starred = 1");
        }
        if let Some(kw) = &q.keyword {
            let kw = kw.trim();
            if !kw.is_empty() {
                sql.push_str(" AND (a.title LIKE ? ESCAPE '\\' OR a.summary LIKE ? ESCAPE '\\')");
                let like = format!(
                    "%{}%",
                    kw.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                );
                params.push(Box::new(like.clone()));
                params.push(Box::new(like));
            }
        }
        let mut stmt = conn.prepare(&sql)?;
        let n: i64 = stmt.query_row(
            rusqlite::params_from_iter(params.iter().map(|p| p.as_ref())),
            |row| row.get(0),
        )?;
        Ok(n)
    }

    /// 每个源最多保留 max 篇：优先清理最旧的「已读且未加星」文章
    pub fn trim_feed(&self, feed_id: i64, max: usize) -> Result<usize> {
        if max == 0 {
            return Ok(0);
        }
        let conn = self.0.lock().unwrap();
        let n = conn.execute(
            r#"
            DELETE FROM articles
            WHERE feed_id = ?1
              AND is_read = 1
              AND is_starred = 0
              AND id NOT IN (
                  SELECT id FROM articles
                  WHERE feed_id = ?1
                  ORDER BY COALESCE(published_at, fetched_at) DESC
                  LIMIT ?2
              )
            "#,
            params![feed_id, max as i64],
        )?;
        Ok(n)
    }

    /// 全局统计，给侧边栏用
    pub fn stats(&self) -> Result<(i64, i64, i64)> {
        let conn = self.0.lock().unwrap();
        let total: i64 = conn.query_row("SELECT COUNT(*) FROM articles", [], |r| r.get(0))?;
        let unread: i64 =
            conn.query_row("SELECT COUNT(*) FROM articles WHERE is_read = 0", [], |r| {
                r.get(0)
            })?;
        let starred: i64 = conn.query_row(
            "SELECT COUNT(*) FROM articles WHERE is_starred = 1",
            [],
            |r| r.get(0),
        )?;
        Ok((total, unread, starred))
    }
}

/// 刷新任务需要的精简源信息
#[derive(Debug, Clone)]
pub struct FeedTarget {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub guid: String,
    pub title: String,
    pub author: Option<String>,
    pub url: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub published_at: Option<String>,
}

fn feed_from_row(row: &rusqlite::Row) -> rusqlite::Result<Feed> {
    Ok(Feed {
        id: row.get(0)?,
        url: row.get(1)?,
        title: row.get(2)?,
        site_url: row.get(3)?,
        description: row.get(4)?,
        category: row.get(5)?,
        last_fetched_at: parse_dt(row.get::<_, Option<String>>(6)?),
        last_error: row.get(7)?,
        created_at: parse_dt(row.get::<_, Option<String>>(8)?).unwrap_or_else(Utc::now),
        unread_count: row.get(9)?,
        total_count: row.get(10)?,
        starred_count: row.get(11)?,
    })
}

fn article_from_row(row: &rusqlite::Row) -> rusqlite::Result<Article> {
    Ok(Article {
        id: row.get(0)?,
        feed_id: row.get(1)?,
        guid: row.get(2)?,
        title: row.get(3)?,
        author: row.get(4)?,
        url: row.get(5)?,
        summary: row.get(6)?,
        content: row.get(7)?,
        published_at: parse_dt(row.get::<_, Option<String>>(8)?),
        fetched_at: parse_dt(row.get::<_, Option<String>>(9)?).unwrap_or_else(Utc::now),
        is_read: row.get::<_, i64>(10)? != 0,
        is_starred: row.get::<_, i64>(11)? != 0,
        feed_title: row.get(12)?,
        feed_site_url: row.get(13)?,
    })
}

fn parse_dt(s: Option<String>) -> Option<DateTime<Utc>> {
    let s = s?;
    DateTime::parse_from_rfc3339(&s)
        .map(|d| d.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S")
                .map(|d| DateTime::<Utc>::from_naive_utc_and_offset(d, Utc))
                .ok()
        })
}

/// 同一源标题去重的时间窗口（小时）。两篇文章的时间差不超过这个值才比较标题。
const TITLE_DEDUP_WINDOW_HOURS: i64 = 6;
/// 归一化后允许多出来的连续字符数。用来吃掉「搜狐 / 搜狐网」这种站点尾巴。
const TITLE_NEAR_MAX_EXTRA: usize = 3;
/// 短标题只认完全相同，避免「上涨 / 上涨了」这类被误伤。
/// 去掉来源后缀后，正文也要达到这个长度，才把后缀当成来源而不是标题的一部分。
const TITLE_NEAR_MIN_CHARS: usize = 10;
/// 来源名最长字符数。「21世纪经济报道」这类媒体名放得下，整句标题放不下。
const OUTLET_MAX_CHARS: usize = 32;

struct TitleKeys {
    /// 整条标题归一化后的结果，保留来源后缀。
    full: String,
    /// 去掉末尾来源后的正文。没有可识别的来源时与 `full` 相同。
    head: String,
}

struct TitleStamp {
    title: String,
    keys: TitleKeys,
    at: DateTime<Utc>,
}

/// 同一源下去重用的文章地址。空白不算地址，避免把没链接的条目合成一篇。
fn article_url_key(url: Option<&str>) -> Option<String> {
    let url = url?.trim();
    if url.is_empty() {
        None
    } else {
        Some(url.to_string())
    }
}

fn article_time(published_at: Option<&str>, fallback: DateTime<Utc>) -> DateTime<Utc> {
    published_at
        .and_then(|s| parse_dt(Some(s.to_string())))
        .unwrap_or(fallback)
}

fn within_dedup_window(a: DateTime<Utc>, b: DateTime<Utc>) -> bool {
    let secs = (a - b).num_seconds().saturating_abs();
    secs <= TITLE_DEDUP_WINDOW_HOURS * 3600
}

/// 标题相同，或去掉空白和标点后只差一小段连续字符（加在开头、中间或结尾都行）。
/// 末尾如果是媒体来源（「 - 新华网」「 - ThePaper.cn」），比较时也会丢掉再看一遍。
fn titles_near_duplicate(a: &str, b: &str) -> bool {
    match (title_keys(a), title_keys(b)) {
        (Some(x), Some(y)) => title_keys_close(&x, &y),
        _ => false,
    }
}

fn title_keys_close(a: &TitleKeys, b: &TitleKeys) -> bool {
    normalized_titles_close(&a.full, &b.full) || normalized_titles_close(&a.head, &b.head)
}

fn title_keys(title: &str) -> Option<TitleKeys> {
    let full = title_key(title)?;
    let head = title_key(strip_outlet_suffix(title.trim())).unwrap_or_else(|| full.clone());
    Some(TitleKeys { full, head })
}

/// 去掉 Google 新闻那种「标题 - 来源」。认不出来源时原样返回。
fn strip_outlet_suffix(title: &str) -> &str {
    let Some((head, tail)) = split_outlet(title) else {
        return title;
    };
    if head.chars().count() >= TITLE_NEAR_MIN_CHARS && looks_like_outlet(tail) {
        head
    } else {
        title
    }
}

fn split_outlet(title: &str) -> Option<(&str, &str)> {
    const SEPS: &[&str] = &[" - ", " — ", " – ", " － ", " | ", " ｜ "];
    let mut best: Option<(usize, usize)> = None;
    for sep in SEPS {
        if let Some(i) = title.rfind(sep) {
            let end = i + sep.len();
            if best.map(|(at, _)| i >= at).unwrap_or(true) {
                best = Some((i, end));
            }
        }
    }
    let (at, end) = best?;
    let head = title[..at].trim();
    let tail = title[end..].trim();
    if head.is_empty() || tail.is_empty() {
        None
    } else {
        Some((head, tail))
    }
}

fn looks_like_outlet(s: &str) -> bool {
    let s = s.trim();
    let n = s.chars().count();
    if n < 2 || n > OUTLET_MAX_CHARS || s.chars().any(is_clause_punct) {
        return false;
    }
    if ends_with_outlet_marker(s) || looks_like_latin_outlet(s) {
        return true;
    }
    if looks_like_digit_brand(s) || looks_like_mixed_brand(s) {
        return true;
    }
    // 新浪、网易、搜狐这种没有「网 / 报」尾巴的短名。
    // 「美股大跌」含涨跌结果字，不当成来源。
    n <= 4 && s.chars().all(is_cjk) && !s.chars().any(is_result_char)
}

fn looks_like_latin_outlet(s: &str) -> bool {
    !s.chars().any(is_cjk)
        && s.chars().any(|c| c.is_ascii_alphabetic())
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | ' ' | '&' | '\'' | '-'))
}

/// 「36氪」：数字加一个汉字。
fn looks_like_digit_brand(s: &str) -> bool {
    let mut cjk = 0;
    let mut digits = 0;
    for c in s.chars() {
        if is_cjk(c) {
            cjk += 1;
        } else if c.is_ascii_digit() {
            digits += 1;
        } else {
            return false;
        }
    }
    digits > 0 && cjk == 1
}

fn looks_like_mixed_brand(s: &str) -> bool {
    let n = s.chars().count();
    if n > 8 {
        return false;
    }
    let has_cjk = s.chars().any(is_cjk);
    let has_latin = s.chars().any(|c| c.is_ascii_alphabetic());
    has_cjk
        && has_latin
        && !s.chars().any(is_result_char)
        && s.chars().all(|c| is_cjk(c) || c.is_ascii_alphanumeric())
}

fn ends_with_outlet_marker(s: &str) -> bool {
    const MARKERS: &[&str] = &[
        "网",
        "报",
        "報",
        "社",
        "刊",
        "新闻",
        "新聞",
        "时报",
        "時報",
        "日报",
        "日報",
        "晚报",
        "晚報",
        "周刊",
        "见闻",
        "見聞",
        "财经",
        "財經",
        "传媒",
        "傳媒",
        "在线",
        "在線",
        "电视台",
    ];
    MARKERS.iter().any(|m| s.ends_with(m))
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF)
}

fn is_clause_punct(c: char) -> bool {
    matches!(
        c,
        '，' | '。'
            | '！'
            | '？'
            | '；'
            | '、'
            | ','
            | '!'
            | '?'
            | ';'
            | ':'
            | '：'
            | '（'
            | '）'
            | '('
            | ')'
            | '「'
            | '」'
            | '"'
            | '“'
            | '”'
    )
}

fn is_result_char(c: char) -> bool {
    matches!(c, '涨' | '跌' | '升' | '降' | '弹' | '暴' | '增' | '减')
}

/// `(无标题)` 是抓取时的占位，不参与去重，否则无标题文章会互相挡住。
fn title_key(title: &str) -> Option<String> {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed == "(无标题)" {
        return None;
    }
    let key = normalize_title(trimmed);
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

fn normalize_title(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        let ch = fold_width(ch);
        if ch.is_whitespace() || is_title_punct(ch) {
            continue;
        }
        for c in ch.to_lowercase() {
            out.push(c);
        }
    }
    out
}

fn fold_width(ch: char) -> char {
    match ch as u32 {
        0x3000 => ' ',
        0xFF01..=0xFF5E => char::from_u32(ch as u32 - 0xFEE0).unwrap_or(ch),
        _ => ch,
    }
}

fn is_title_punct(ch: char) -> bool {
    ch.is_ascii_punctuation()
        || matches!(
            ch as u32,
            0x2010..=0x2027 | 0x2030..=0x205E | 0x3001..=0x303F
        )
}

fn normalized_titles_close(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (shorter, longer) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    let sl = shorter.chars().count();
    let extra = longer.chars().count() - sl;
    if sl < TITLE_NEAR_MIN_CHARS || extra == 0 || extra > TITLE_NEAR_MAX_EXTRA {
        return false;
    }
    let sb: Vec<char> = shorter.chars().collect();
    let lb: Vec<char> = longer.chars().collect();
    let mut i = 0;
    while i < sb.len() && sb[i] == lb[i] {
        i += 1;
    }
    sb[i..] == lb[i + extra..]
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use super::{titles_near_duplicate, Db};
    use crate::models::{ArticleQuery, NewFeed};

    const SOHU: &str = "伊朗袭击行动升级，一艘集装箱船于霍尔木兹海峡沉没 - 搜狐";
    const SOHU_NET: &str = "伊朗袭击行动升级，一艘集装箱船于霍尔木兹海峡沉没 - 搜狐网";

    #[test]
    fn site_suffix_counts_as_near_duplicate() {
        assert!(titles_near_duplicate(SOHU, SOHU_NET));
        assert!(titles_near_duplicate(
            "搜狐：伊朗袭击行动升级，一艘集装箱船于霍尔木兹海峡沉没",
            "搜狐网：伊朗袭击行动升级，一艘集装箱船于霍尔木兹海峡沉没",
        ));
        assert!(titles_near_duplicate(
            "Hello, World — Daily",
            "hello world daily"
        ));
    }

    #[test]
    fn google_news_source_counts_as_near_duplicate() {
        assert!(titles_near_duplicate(
            "国际油价15日涨跌不一 - 新华网",
            "国际油价15日涨跌不一 - ThePaper.cn",
        ));
        assert!(titles_near_duplicate(
            "国际油价15日涨跌不一 - 新浪",
            "国际油价15日涨跌不一 - 网易",
        ));
        assert!(titles_near_duplicate(
            "国际油价15日涨跌不一",
            "国际油价15日涨跌不一 - ThePaper.cn",
        ));
        assert!(titles_near_duplicate(
            "国际油价15日涨跌不一 - 36氪",
            "国际油价15日涨跌不一 — Reuters",
        ));
    }

    #[test]
    fn distinct_headlines_stay() {
        assert!(!titles_near_duplicate(
            "伊朗袭击行动升级，一艘集装箱船于霍尔木兹海峡沉没",
            "伊朗袭击行动升级，两艘油轮在霍尔木兹海峡遭袭",
        ));
        assert!(!titles_near_duplicate(
            "会议在北京召开并公布完整方案 - 新华网",
            "会议在上海召开并公布完整方案 - ThePaper.cn",
        ));
        assert!(!titles_near_duplicate(
            "特朗普称将加税并引发市场波动 - 美股大跌",
            "特朗普称将加税并引发市场波动 - 美股反弹",
        ));
        assert!(!titles_near_duplicate(
            "会议在北京召开并公布完整方案",
            "会议在上海召开并公布完整方案",
        ));
        assert!(!titles_near_duplicate("上涨", "下跌"));
        assert!(titles_near_duplicate("上涨", "上涨"));
        assert!(!titles_near_duplicate("(无标题)", "(无标题)"));
    }

    #[test]
    fn logs_both_titles_when_a_near_duplicate_is_skipped() {
        let path = std::env::temp_dir().join(format!(
            "tiny-reader-title-dedup-log-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Db::open(&path).unwrap();
        let feed = db
            .insert_feed(&NewFeed {
                url: "https://example.com/log.xml".into(),
                title: Some("Log".into()),
                category: None,
            })
            .unwrap();
        let t0 = Utc::now();
        db.insert_articles(feed.id, &[article("keep", SOHU, Some(t0.to_rfc3339()))])
            .unwrap();

        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let writer = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .without_time()
            .with_writer(move || LogBuf(writer.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            db.insert_articles(
                feed.id,
                &[article(
                    "drop",
                    SOHU_NET,
                    Some((t0 + Duration::hours(1)).to_rfc3339()),
                )],
            )
            .unwrap();
        });

        let log = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            log.contains(SOHU) && log.contains(SOHU_NET),
            "日志应同时带上保留和被忽略的标题，实际：{log}"
        );

        drop(db);
        let _ = std::fs::remove_file(&path);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = std::fs::remove_file(side);
        }
    }

    #[test]
    fn same_url_is_dropped_before_title_filter_and_not_logged() {
        let path = std::env::temp_dir().join(format!(
            "tiny-reader-url-before-title-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Db::open(&path).unwrap();
        let feed = db
            .insert_feed(&NewFeed {
                url: "https://example.com/url-dedup.xml".into(),
                title: Some("U".into()),
                category: None,
            })
            .unwrap();
        let other = db
            .insert_feed(&NewFeed {
                url: "https://example.com/url-dedup-other.xml".into(),
                title: Some("V".into()),
                category: None,
            })
            .unwrap();

        let story = "https://news.example/story";
        let batch_url = "https://news.example/batch";
        let batch_early = "另一条足够长的新闻标题用于地址去重 - 甲";
        let batch_late = "另一条足够长的新闻标题用于地址去重 - 甲网";
        let other_url_title = "另一条足够长的新闻标题用于地址去重 - 乙网";
        let t0 = Utc::now();
        db.insert_articles(
            feed.id,
            &[article_with_url("keep", SOHU, story, Some(t0.to_rfc3339()))],
        )
        .unwrap();

        let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
        let writer = buf.clone();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
            .with_ansi(false)
            .without_time()
            .with_writer(move || LogBuf(writer.clone()))
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            // 库里已有同地址：相近标题，以及 6 小时窗外的不同标题，都在标题过滤前丢掉。
            assert_eq!(
                db.insert_articles(
                    feed.id,
                    &[
                        article_with_url(
                            "again-near",
                            SOHU_NET,
                            story,
                            Some((t0 + Duration::hours(1)).to_rfc3339()),
                        ),
                        article_with_url(
                            "again-diff",
                            "完全不同的另一则航运新闻标题",
                            story,
                            Some((t0 + Duration::hours(8)).to_rfc3339()),
                        ),
                    ],
                )
                .unwrap(),
                0
            );

            // 同一批里同地址只留更早的那篇，不打标题去重日志。
            assert_eq!(
                db.insert_articles(
                    feed.id,
                    &[
                        article_with_url(
                            "batch-late",
                            batch_late,
                            batch_url,
                            Some((t0 + Duration::hours(30)).to_rfc3339()),
                        ),
                        article_with_url(
                            "batch-early",
                            batch_early,
                            batch_url,
                            Some((t0 + Duration::hours(29)).to_rfc3339()),
                        ),
                    ],
                )
                .unwrap(),
                1
            );

            // 别的源可以收同一篇文章。
            assert_eq!(
                db.insert_articles(
                    other.id,
                    &[article_with_url(
                        "other-feed",
                        SOHU_NET,
                        story,
                        Some((t0 + Duration::hours(1)).to_rfc3339()),
                    )],
                )
                .unwrap(),
                1
            );

            // 地址不同，标题接近，仍然走标题去重并打日志。
            assert_eq!(
                db.insert_articles(
                    feed.id,
                    &[article_with_url(
                        "other-url",
                        other_url_title,
                        "https://news.example/other",
                        Some((t0 + Duration::hours(29) + Duration::minutes(30)).to_rfc3339()),
                    )],
                )
                .unwrap(),
                0
            );
        });

        let log = String::from_utf8(buf.lock().unwrap().clone()).unwrap();
        assert!(
            !log.contains(SOHU_NET) && !log.contains(batch_late),
            "同地址不应打标题去重日志，实际：{log}"
        );
        assert!(
            log.contains(batch_early) && log.contains(other_url_title),
            "不同地址的近重复标题仍应打日志，实际：{log}"
        );

        let guids = |id: i64| -> Vec<String> {
            db.list_articles(&ArticleQuery {
                feed_id: Some(id),
                limit: 50,
                ..ArticleQuery::default()
            })
            .unwrap()
            .into_iter()
            .map(|a| a.guid)
            .collect()
        };
        let got = guids(feed.id);
        for guid in ["keep", "batch-early"] {
            assert!(got.iter().any(|g| g == guid), "missing {guid} in {got:?}");
        }
        for guid in ["again-near", "again-diff", "batch-late", "other-url"] {
            assert!(
                got.iter().all(|g| g != guid),
                "{guid} should have been skipped, got {got:?}"
            );
        }
        assert_eq!(guids(other.id), vec!["other-feed".to_string()]);

        drop(db);
        let _ = std::fs::remove_file(&path);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = std::fs::remove_file(side);
        }
    }

    #[test]
    fn skips_later_near_title_from_same_feed_within_six_hours() {
        let path = std::env::temp_dir().join(format!(
            "tiny-reader-title-dedup-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Db::open(&path).unwrap();
        let feed = db
            .insert_feed(&NewFeed {
                url: "https://example.com/a.xml".into(),
                title: Some("A".into()),
                category: None,
            })
            .unwrap();
        let other = db
            .insert_feed(&NewFeed {
                url: "https://example.com/b.xml".into(),
                title: Some("B".into()),
                category: None,
            })
            .unwrap();

        let t0 = Utc::now();
        let at = |d: Duration| Some((t0 + d).to_rfc3339());

        assert_eq!(
            db.insert_articles(feed.id, &[article("g-early", SOHU, at(Duration::zero()))])
                .unwrap(),
            1
        );

        // 同一批里：2 小时后的近重复标题丢掉，另一篇不同标题留下。
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[
                    article("g-near-2h", SOHU_NET, at(Duration::hours(2))),
                    article(
                        "g-other",
                        "完全不同的另一则航运新闻标题",
                        at(Duration::hours(2))
                    ),
                ],
            )
            .unwrap(),
            1
        );

        // 刚好 6 小时仍算窗口内；超过 1 秒则留下。
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[article("g-boundary", SOHU_NET, at(Duration::hours(6)))],
            )
            .unwrap(),
            0
        );
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[article(
                    "g-outside",
                    SOHU_NET,
                    at(Duration::hours(6) + Duration::seconds(1)),
                )],
            )
            .unwrap(),
            1
        );

        // 别的源不受影响。
        assert_eq!(
            db.insert_articles(
                other.id,
                &[article("g-other-feed", SOHU_NET, at(Duration::hours(1)))],
            )
            .unwrap(),
            1
        );

        // 同一批乱序到达时，留下更早的那篇。
        let batch_early = "另一条足够长的新闻标题用于批量去重 - 甲";
        let batch_late = "另一条足够长的新闻标题用于批量去重 - 甲网";
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[
                    article("g-batch-late", batch_late, at(Duration::hours(30))),
                    article("g-batch-early", batch_early, at(Duration::hours(29))),
                ],
            )
            .unwrap(),
            1
        );

        let guids = |id: i64| -> Vec<String> {
            db.list_articles(&ArticleQuery {
                feed_id: Some(id),
                limit: 50,
                ..ArticleQuery::default()
            })
            .unwrap()
            .into_iter()
            .map(|a| a.guid)
            .collect()
        };

        let got = guids(feed.id);
        for guid in ["g-early", "g-other", "g-outside", "g-batch-early"] {
            assert!(got.iter().any(|g| g == guid), "missing {guid} in {got:?}");
        }
        for guid in ["g-near-2h", "g-boundary", "g-batch-late"] {
            assert!(
                got.iter().all(|g| g != guid),
                "{guid} should have been skipped, got {got:?}"
            );
        }
        assert_eq!(guids(other.id), vec!["g-other-feed".to_string()]);

        drop(db);
        let _ = std::fs::remove_file(&path);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = std::fs::remove_file(side);
        }
    }

    #[test]
    fn skips_same_headline_with_different_google_news_source() {
        let path = std::env::temp_dir().join(format!(
            "tiny-reader-title-dedup-source-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Db::open(&path).unwrap();
        let feed = db
            .insert_feed(&NewFeed {
                url: "https://news.google.com/rss".into(),
                title: Some("Google News".into()),
                category: None,
            })
            .unwrap();
        let t0 = Utc::now();
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[article(
                    "oil-xinhua",
                    "国际油价15日涨跌不一 - 新华网",
                    Some(t0.to_rfc3339()),
                )],
            )
            .unwrap(),
            1
        );
        assert_eq!(
            db.insert_articles(
                feed.id,
                &[article(
                    "oil-thepaper",
                    "国际油价15日涨跌不一 - ThePaper.cn",
                    Some((t0 + Duration::hours(1)).to_rfc3339()),
                )],
            )
            .unwrap(),
            0
        );
        let guids: Vec<String> = db
            .list_articles(&ArticleQuery {
                feed_id: Some(feed.id),
                limit: 10,
                ..ArticleQuery::default()
            })
            .unwrap()
            .into_iter()
            .map(|a| a.guid)
            .collect();
        assert_eq!(guids, vec!["oil-xinhua".to_string()]);

        drop(db);
        let _ = std::fs::remove_file(&path);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = std::fs::remove_file(side);
        }
    }

    #[test]
    fn feed_counts_include_starred() {
        let path = std::env::temp_dir().join(format!(
            "tiny-reader-star-count-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let db = Db::open(&path).unwrap();
        let feed = db
            .insert_feed(&NewFeed {
                url: "https://example.com/star.xml".into(),
                title: Some("Star".into()),
                category: None,
            })
            .unwrap();
        db.insert_articles(
            feed.id,
            &[
                article("s1", "未读未星标", Some(Utc::now().to_rfc3339())),
                article("s2", "已读已星标", Some(Utc::now().to_rfc3339())),
            ],
        )
        .unwrap();
        let saved = db
            .list_articles(&ArticleQuery {
                feed_id: Some(feed.id),
                limit: 10,
                ..ArticleQuery::default()
            })
            .unwrap();
        let starred = saved.iter().find(|a| a.guid == "s2").unwrap();
        db.set_read(starred.id, true).unwrap();
        db.set_starred(starred.id, true).unwrap();

        let listed = db.list_feeds().unwrap();
        let got = listed.iter().find(|f| f.id == feed.id).unwrap();
        assert_eq!(got.total_count, 2);
        assert_eq!(got.unread_count, 1);
        assert_eq!(got.starred_count, 1);

        drop(db);
        let _ = std::fs::remove_file(&path);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = std::fs::remove_file(side);
        }
    }

    struct LogBuf(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for LogBuf {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn article(guid: &str, title: &str, published_at: Option<String>) -> super::NewArticle {
        super::NewArticle {
            guid: guid.into(),
            title: title.into(),
            author: None,
            url: None,
            summary: None,
            content: None,
            published_at,
        }
    }

    fn article_with_url(
        guid: &str,
        title: &str,
        url: &str,
        published_at: Option<String>,
    ) -> super::NewArticle {
        let mut article = article(guid, title, published_at);
        article.url = Some(url.into());
        article
    }
}
