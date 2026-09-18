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
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id)                    AS total_count
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
                   (SELECT COUNT(*) FROM articles a WHERE a.feed_id = f.id)                    AS total_count
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

    pub fn insert_articles(&self, feed_id: i64, items: &[NewArticle]) -> Result<usize> {
        if items.is_empty() {
            return Ok(0);
        }
        let now = Utc::now().to_rfc3339();
        let mut conn = self.0.lock().unwrap();
        let tx = conn.transaction()?;
        let mut count = 0usize;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR IGNORE INTO articles
                    (feed_id, guid, title, author, url, summary, content, published_at, fetched_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
            )?;
            for a in items {
                let n = stmt.execute(params![
                    feed_id,
                    a.guid,
                    a.title,
                    a.author,
                    a.url,
                    a.summary,
                    a.content,
                    a.published_at,
                    now,
                ])?;
                count += n;
            }
        }
        tx.commit()?;
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
        sql.push_str(
            " ORDER BY COALESCE(a.published_at, a.fetched_at) DESC, a.id DESC LIMIT ? OFFSET ?",
        );
        params.push(Box::new(q.limit));
        params.push(Box::new(q.offset));

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
