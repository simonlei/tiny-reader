//! 日报订阅：地址里带日期，每次刷新拉取东八区的「昨天」。
//!
//! 库里存的是 `daily://https://example.com/digest/{date}`。
//! 煎蛋这种页面正文由接口另拉，普通日报则直接抓当天的 HTML。

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, Utc};
use serde::Deserialize;

use crate::db::NewArticle;

/// 一次日报拉取解析出的订阅源
pub struct DailyEdition {
    pub title: Option<String>,
    pub site_url: Option<String>,
    pub description: Option<String>,
    pub entries: Vec<NewArticle>,
}

/// 今天该去请求的地址
pub enum DailyRequest {
    /// 煎蛋日报页面本身没有正文，要打热评接口，并带上页面地址当 Referer
    Jandan { page_url: String, api_url: String },
    /// 直接抓日期页
    Page { page_url: String },
}

pub fn is_stored_daily(url: &str) -> bool {
    url.starts_with("daily://")
}

/// 把用户输入收敛成库存形态。不是日报地址时返回 None，交给普通 URL 归一化。
pub fn normalize_daily_input(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let (explicit, rest) = match strip_daily_prefix(s) {
        Some(rest) => (true, rest.trim().trim_start_matches("//").trim()),
        None => (false, s),
    };
    if explicit && rest.is_empty() {
        return None;
    }

    let rest = if rest.starts_with("http://") || rest.starts_with("https://") {
        rest.to_string()
    } else if explicit {
        format!("https://{rest}")
    } else {
        return None;
    };

    if !explicit && !path_ends_with_date_dir(&rest) {
        return None;
    }

    let templated = replace_iso_dates(&rest, "{date}");
    if !templated.contains("{date}") {
        return None;
    }
    let sample = templated.replace("{date}", "2000-01-01");
    reqwest::Url::parse(&sample).ok()?;
    Some(format!("daily://{templated}"))
}

/// 东八区日历上的昨天。服务器时区不一定是中国，日报按读者所在的这一天算。
pub fn china_yesterday(now: DateTime<Utc>) -> NaiveDate {
    let offset = FixedOffset::east_opt(8 * 3600).expect("东八区偏移");
    let today = now.with_timezone(&offset).date_naive();
    today
        .checked_sub_signed(Duration::days(1))
        .expect("昨天的日期")
}

pub fn plan_daily_request(stored: &str, now: DateTime<Utc>) -> Result<DailyRequest, String> {
    let template = stored
        .strip_prefix("daily://")
        .ok_or_else(|| "不是日报地址".to_string())?;
    if !template.contains("{date}") {
        return Err("日报地址里没有 {date}".into());
    }
    let day = china_yesterday(now);
    let page_url = template.replace("{date}", &day.format("%Y-%m-%d").to_string());
    if is_jandan_daily_page(&page_url) {
        Ok(DailyRequest::Jandan {
            api_url: jandan_api_url(day, 1),
            page_url,
        })
    } else {
        Ok(DailyRequest::Page { page_url })
    }
}

/// 这一天中午（东八区）对应的 UTC 时间，用来给「整页一篇」的日报排序
pub fn published_stamp(day: NaiveDate) -> String {
    format!("{}T04:00:00+00:00", day.format("%Y-%m-%d"))
}

/// 煎蛋热评接口的一页
pub struct JandanPage {
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub is_login: bool,
    pub entries: Vec<NewArticle>,
}

pub fn parse_jandan_daily(body: &str) -> Result<DailyEdition, String> {
    let page = parse_jandan_page(body)?;
    if page.entries.is_empty() {
        return Err("这一天还没有日报内容".into());
    }
    Ok(jandan_edition(page.entries, page.total, page.is_login))
}

pub fn parse_jandan_page(body: &str) -> Result<JandanPage, String> {
    let resp: JandanResp =
        serde_json::from_str(body).map_err(|e| format!("煎蛋日报解析失败: {e}"))?;
    if resp.code != 0 {
        let msg = resp.msg.trim();
        return Err(if msg.is_empty() {
            format!("煎蛋日报返回错误（code {}）", resp.code)
        } else {
            format!("煎蛋日报返回错误：{msg}")
        });
    }
    let data = resp
        .data
        .ok_or_else(|| "煎蛋日报没有返回内容（可能被拦截，或这一天还没发布）".to_string())?;
    let list = data.list.unwrap_or_default();
    let entries = list
        .iter()
        .map(jandan_comment_to_article)
        .collect::<Vec<_>>();
    let total = data.total.unwrap_or(entries.len() as i64);
    Ok(JandanPage {
        total,
        page: data.page.unwrap_or(1),
        page_size: data.page_size.unwrap_or(entries.len() as i64),
        is_login: data.is_login,
        entries,
    })
}

/// 这一页的内容该不该收，以及要不要继续请求下一页。
/// 未登录时服务端会把第 2 页请求原样退回第 1 页，这时 `response_page` 对不上。
pub fn jandan_has_more(
    requested_page: i64,
    response_page: i64,
    page_len: usize,
    page_size: i64,
    total: i64,
    fetched: usize,
) -> bool {
    if page_len == 0 || response_page != requested_page {
        return false;
    }
    if total > 0 && fetched as i64 >= total {
        return false;
    }
    if page_size > 0 && (page_len as i64) < page_size {
        return false;
    }
    true
}

pub fn jandan_edition(entries: Vec<NewArticle>, total: i64, is_login: bool) -> DailyEdition {
    let n = entries.len() as i64;
    let description = if total > n {
        if is_login {
            format!("按东八区昨天拉取。本次拿到 {n} 条，当天共 {total} 条。")
        } else {
            format!("按东八区昨天拉取。未登录只能拿到热度前 {n} 条，当天共 {total} 条。")
        }
    } else {
        format!("按东八区昨天拉取，当天 {n} 条。")
    };
    DailyEdition {
        title: Some("煎蛋日报".into()),
        site_url: Some("https://jandan.net/new/daily".into()),
        description: Some(description),
        entries,
    }
}

pub fn parse_html_daily(
    page_url: &str,
    html: &str,
    published_at: &str,
) -> Result<DailyEdition, String> {
    let cleaned = strip_blocks(
        &strip_blocks(&strip_blocks(html, "script"), "style"),
        "noscript",
    );
    let region = extract_region(&cleaned, "article")
        .or_else(|| extract_region(&cleaned, "main"))
        .or_else(|| extract_region(&cleaned, "body"))
        .unwrap_or_else(|| cleaned.clone());
    let text = html_to_text(&region);
    let meaningful = strip_placeholders(&text);
    let has_img = region.to_ascii_lowercase().contains("<img");
    if meaningful.chars().count() < 4 && !has_img {
        return Err("页面正文是空的，这个地址可能要浏览器执行脚本才能看到内容".into());
    }

    let page_title = extract_region(&cleaned, "title")
        .map(|t| html_to_text(&t))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| page_url.to_string());
    let feed_title = stable_feed_title(&page_title, page_url);

    Ok(DailyEdition {
        title: Some(feed_title),
        site_url: origin_of(page_url),
        description: Some("按东八区昨天拉取的日报页面".into()),
        entries: vec![NewArticle {
            guid: page_url.to_string(),
            title: page_title,
            author: None,
            url: Some(page_url.to_string()),
            summary: Some(truncate_chars(&text, 180)),
            content: Some(region),
            published_at: Some(published_at.to_string()),
        }],
    })
}

pub fn jandan_api_url(day: NaiveDate, page: u32) -> String {
    format!(
        "https://jandan.net/api/v1/daily-hot/comments?cdn_line=default&date={}&sort=vote_desc&page={}&page_size=50&from=",
        day.format("%Y-%m-%d"),
        page.max(1)
    )
}

fn is_jandan_daily_page(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    let host = u.host_str().unwrap_or("");
    if host != "jandan.net" && host != "www.jandan.net" {
        return false;
    }
    let path = u.path().trim_end_matches('/');
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segs.windows(3)
        .any(|w| w[0] == "daily" && w[1].eq_ignore_ascii_case("date") && is_iso_date(w[2]))
}

fn jandan_comment_to_article(c: &JandanComment) -> NewArticle {
    let own = html_to_text(c.content.as_deref().unwrap_or(""));
    let tucao_text = c
        .hot_tucao_list
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find_map(|t| {
            let text = html_to_text(t.content.as_deref().unwrap_or(""));
            (!text.is_empty()).then_some(text)
        })
        .unwrap_or_default();
    let excerpt = if !own.is_empty() {
        own
    } else if !tucao_text.is_empty() {
        tucao_text
    } else {
        c.post_title.clone().unwrap_or_else(|| "煎蛋".into())
    };
    let title = format!("{} #{}", truncate_chars(&excerpt, 60), c.id);

    let author_name = c.author.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let location = c
        .ip_location
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let author = match (author_name, location) {
        (Some(name), Some(loc)) => Some(format!("{name} · {loc}")),
        (Some(name), None) => Some(name.to_string()),
        (None, Some(loc)) => Some(loc.to_string()),
        (None, None) => None,
    };

    let mut content = String::new();
    let section = c.post_title.as_deref().unwrap_or("").trim();
    let oo = c.vote_positive.unwrap_or(0);
    let xx = c.vote_negative.unwrap_or(0);
    if !section.is_empty() || oo > 0 || xx > 0 {
        content.push_str(&format!(
            "<p>{}OO {oo} / XX {xx}</p>\n",
            if section.is_empty() {
                String::new()
            } else {
                format!("{} · ", escape_html(section))
            }
        ));
    }
    if let Some(html) = c.content.as_deref() {
        content.push_str(html);
        if !html.ends_with('\n') {
            content.push('\n');
        }
    }
    if let Some(list) = c.hot_tucao_list.as_deref() {
        let list: Vec<_> = list.iter().take(3).collect();
        if !list.is_empty() {
            content.push_str("<h4>热评</h4>\n");
            for t in list {
                let who = t.author.as_deref().unwrap_or("").trim();
                let votes = t.vote_positive.unwrap_or(0);
                let text = t.content.as_deref().unwrap_or("");
                content.push_str("<blockquote><p>");
                if !who.is_empty() {
                    content.push_str(&format!("<strong>{}</strong>", escape_html(who)));
                    content.push_str(&format!(" · OO {votes}"));
                }
                content.push_str("</p><p>");
                content.push_str(&escape_html(text));
                content.push_str("</p></blockquote>\n");
            }
        }
    }

    let published_at = c.date_gmt.as_deref().and_then(to_utc_stamp);

    NewArticle {
        guid: format!("jandan:{}", c.id),
        title,
        author,
        url: Some(format!("https://jandan.net/t/{}", c.id)),
        summary: Some(truncate_chars(&excerpt, 180)),
        content: Some(content),
        published_at,
    }
}

fn to_utc_stamp(raw: &str) -> Option<String> {
    let dt = DateTime::parse_from_rfc3339(raw).ok()?;
    Some(
        dt.with_timezone(&Utc)
            .format("%Y-%m-%dT%H:%M:%S+00:00")
            .to_string(),
    )
}

fn path_ends_with_date_dir(url: &str) -> bool {
    let Ok(u) = reqwest::Url::parse(url) else {
        return false;
    };
    let path = u.path().trim_end_matches('/');
    let mut segs = path.split('/').filter(|s| !s.is_empty());
    let last = segs.next_back();
    let prev = segs.next_back();
    match (prev, last) {
        (Some(dir), Some(date)) => dir.eq_ignore_ascii_case("date") && is_iso_date(date),
        _ => false,
    }
}

fn is_iso_date(s: &str) -> bool {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

fn replace_iso_dates(input: &str, replacement: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(len) = match_iso_date(&chars[i..]) {
            out.push_str(replacement);
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// 从字符切片开头匹配一个合法的 `YYYY-MM-DD`，返回消耗的字符数
fn match_iso_date(chars: &[char]) -> Option<usize> {
    if chars.len() < 10 {
        return None;
    }
    let s: String = chars[..10].iter().collect();
    is_iso_date(&s).then_some(10)
}

fn strip_known_dates(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if let Some(len) = match_chinese_date_at(&chars[i..]) {
            i += len;
            continue;
        }
        if let Some(len) = match_iso_date(&chars[i..]) {
            i += len;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

fn match_chinese_date_at(chars: &[char]) -> Option<usize> {
    let mut i = take_digits(chars, 0, 4)?;
    if chars.get(i).copied() != Some('年') {
        return None;
    }
    i += 1;
    let month_end = take_digits(chars, i, 2).or_else(|| take_digits(chars, i, 1))?;
    i = month_end;
    if chars.get(i).copied() != Some('月') {
        return None;
    }
    i += 1;
    let day_end = take_digits(chars, i, 2).or_else(|| take_digits(chars, i, 1))?;
    i = day_end;
    if chars.get(i).copied() != Some('日') {
        return None;
    }
    Some(i + 1)
}

/// 恰好 n 位数字，返回结束下标
fn take_digits(chars: &[char], start: usize, n: usize) -> Option<usize> {
    if n == 0 || start + n > chars.len() {
        return None;
    }
    if chars[start..start + n].iter().all(|c| c.is_ascii_digit()) {
        // 1 位匹配时，后面如果还是数字就不要在这里停（交给更长的匹配先试）
        if n == 1 && chars.get(start + 1).is_some_and(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(start + n)
    } else {
        None
    }
}

fn stable_feed_title(page_title: &str, page_url: &str) -> String {
    let stripped = strip_known_dates(page_title);
    let trimmed = trim_separators(&stripped);
    if trimmed.chars().count() >= 2 {
        trimmed
    } else {
        origin_of(page_url)
            .and_then(|u| reqwest::Url::parse(&u).ok())
            .and_then(|u| u.host_str().map(|h| h.to_string()))
            .unwrap_or_else(|| page_title.trim().to_string())
    }
}

fn trim_separators(s: &str) -> String {
    let trimmed = s
        .trim()
        .trim_matches(|c: char| {
            c.is_whitespace() || matches!(c, '-' | '—' | '–' | '|' | '｜' | '·' | '/' | '／')
        })
        .trim();
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn origin_of(url: &str) -> Option<String> {
    let u = reqwest::Url::parse(url).ok()?;
    let host = u.host_str()?;
    let scheme = u.scheme();
    match u.port() {
        Some(p) => Some(format!("{scheme}://{host}:{p}/")),
        None => Some(format!("{scheme}://{host}/")),
    }
}

fn strip_daily_prefix(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let prefix = b"daily:";
    if bytes.len() >= prefix.len() && bytes[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn strip_blocks(html: &str, tag: &str) -> String {
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut out = String::new();
    let mut rest = html;
    loop {
        let Some(start) = find_ignore_ascii_case(rest, &open) else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..start]);
        let after_open = start + open.len();
        let Some(close_rel) = find_ignore_ascii_case(&rest[after_open..], &close) else {
            break;
        };
        let after_close_name = after_open + close_rel + close.len();
        let Some(gt) = rest[after_close_name..].find('>') else {
            break;
        };
        rest = &rest[after_close_name + gt + 1..];
    }
    out
}

fn extract_region(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let start = find_ignore_ascii_case(html, &open)?;
    let after = html[start..].find('>')? + start + 1;
    let close = format!("</{tag}");
    let close_at = find_ignore_ascii_case(&html[after..], &close)? + after;
    Some(html[after..close_at].to_string())
}

fn find_ignore_ascii_case(hay: &str, needle: &str) -> Option<usize> {
    let h = hay.as_bytes();
    let n = needle.as_bytes();
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    let last = h.len() - n.len();
    let mut i = 0;
    while i <= last {
        if hay.is_char_boundary(i) && h[i..i + n.len()].eq_ignore_ascii_case(n) {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
            continue;
        }
        if ch == '>' {
            in_tag = false;
            out.push(' ');
            continue;
        }
        if !in_tag {
            out.push(ch);
        }
    }
    unescape_basic(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_placeholders(text: &str) -> String {
    let mut t = text.to_string();
    for phrase in ["数据加载中", "加载中", "Loading", "loading"] {
        t = t.replace(phrase, "");
    }
    t.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '.' | '。' | '…' | '!' | '！' | '-' | '—'))
        .collect()
}

fn unescape_basic(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate_chars(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}

#[derive(Debug, Deserialize)]
struct JandanResp {
    code: i64,
    #[serde(default)]
    msg: String,
    data: Option<JandanData>,
}

#[derive(Debug, Deserialize)]
struct JandanData {
    total: Option<i64>,
    page: Option<i64>,
    page_size: Option<i64>,
    #[serde(default)]
    is_login: bool,
    list: Option<Vec<JandanComment>>,
}

#[derive(Debug, Deserialize)]
struct JandanComment {
    id: i64,
    author: Option<String>,
    date_gmt: Option<String>,
    content: Option<String>,
    vote_positive: Option<i64>,
    vote_negative: Option<i64>,
    post_title: Option<String>,
    ip_location: Option<String>,
    hot_tucao_list: Option<Vec<JandanTucao>>,
}

#[derive(Debug, Deserialize)]
struct JandanTucao {
    author: Option<String>,
    content: Option<String>,
    vote_positive: Option<i64>,
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, TimeZone, Utc};

    use super::{
        china_yesterday, jandan_api_url, jandan_edition, jandan_has_more, normalize_daily_input,
        parse_html_daily, parse_jandan_daily, plan_daily_request, DailyRequest,
    };
    use crate::db::NewArticle;

    #[test]
    fn normalize_daily_prefix_replaces_sample_date() {
        assert_eq!(
            normalize_daily_input("daily:https://jandan.net/new/daily/date/2026-10-15").as_deref(),
            Some("daily://https://jandan.net/new/daily/date/{date}")
        );
        assert_eq!(
            normalize_daily_input("  daily://https://example.com/digest/{date}  ").as_deref(),
            Some("daily://https://example.com/digest/{date}")
        );
    }

    #[test]
    fn normalize_promotes_date_path_without_prefix() {
        assert_eq!(
            normalize_daily_input("https://jandan.net/new/daily/date/2026-10-15").as_deref(),
            Some("daily://https://jandan.net/new/daily/date/{date}")
        );
    }

    #[test]
    fn normalize_leaves_ordinary_urls_alone() {
        assert_eq!(normalize_daily_input("https://example.com/feed.xml"), None);
        assert_eq!(
            normalize_daily_input("https://example.com/posts/2026-10-15"),
            None
        );
        assert_eq!(
            normalize_daily_input("daily:https://example.com/archive"),
            None
        );
        assert_eq!(normalize_daily_input("rsshub://zhihu/daily"), None);
    }

    #[test]
    fn yesterday_follows_china_calendar() {
        // 2026-10-15 16:00 UTC = 2026-10-16 00:00 CST，昨天是 10 月 15 日
        let just_after_midnight = Utc.with_ymd_and_hms(2026, 10, 15, 16, 0, 0).unwrap();
        assert_eq!(
            china_yesterday(just_after_midnight),
            NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
        );
        // 差一秒还是 10 月 15 日，昨天是 14 日
        let before_midnight = Utc.with_ymd_and_hms(2026, 10, 15, 15, 59, 59).unwrap();
        assert_eq!(
            china_yesterday(before_midnight),
            NaiveDate::from_ymd_opt(2026, 10, 14).unwrap()
        );
        // 3 月 1 日凌晨，昨天是 2 月 28 日
        let march = Utc.with_ymd_and_hms(2026, 2, 28, 16, 30, 0).unwrap();
        assert_eq!(
            china_yesterday(march),
            NaiveDate::from_ymd_opt(2026, 2, 28).unwrap()
        );
    }

    #[test]
    fn plan_fetches_yesterdays_jandan_api() {
        let stored = "daily://https://jandan.net/new/daily/date/{date}";
        // 2026-10-16 04:00 UTC = 当天中午（东八区），昨天是 10 月 15 日
        let now = Utc.with_ymd_and_hms(2026, 10, 16, 4, 0, 0).unwrap();
        match plan_daily_request(stored, now).unwrap() {
            DailyRequest::Jandan { page_url, api_url } => {
                assert_eq!(page_url, "https://jandan.net/new/daily/date/2026-10-15");
                assert!(api_url.contains("date=2026-10-15"), "{api_url}");
                assert!(api_url.contains("/api/v1/daily-hot/comments"), "{api_url}");
            }
            DailyRequest::Page { page_url } => panic!("应走煎蛋接口，实际请求了 {page_url}"),
        }
    }

    #[test]
    fn plan_renders_generic_page() {
        let stored = "daily://https://example.com/digest/{date}/index.html";
        let now = Utc.with_ymd_and_hms(2026, 10, 16, 4, 0, 0).unwrap();
        match plan_daily_request(stored, now).unwrap() {
            DailyRequest::Page { page_url } => {
                assert_eq!(page_url, "https://example.com/digest/2026-10-15/index.html");
            }
            DailyRequest::Jandan { .. } => panic!("普通日报不应走煎蛋接口"),
        }
    }

    #[test]
    fn parse_jandan_maps_comments() {
        let body = r#"{
            "code": 0,
            "msg": "success",
            "data": {
                "report_date": "2026-10-15",
                "total": 66,
                "list": [
                    {
                        "id": 6226724,
                        "author": "Demaner",
                        "date_gmt": "2026-10-15T09:17:14+08:00",
                        "content": "<img src=\"https://img.example/a.jpg\" />",
                        "vote_positive": 626,
                        "vote_negative": 76,
                        "sub_comment_count": 54,
                        "post_title": "无聊图",
                        "ip_location": "南京",
                        "hot_tucao_list": [
                            {"author": "PtLuke", "content": "<b>别信</b>", "vote_positive": 183}
                        ]
                    },
                    {
                        "id": 6226941,
                        "author": "啊啊啊",
                        "date_gmt": "2026-10-15T16:45:00+08:00",
                        "content": "<p>减肥成功了</p>",
                        "vote_positive": 10,
                        "vote_negative": 1,
                        "post_title": "随手拍",
                        "ip_location": "青岛"
                    }
                ]
            }
        }"#;
        let edition = parse_jandan_daily(body).unwrap();
        assert_eq!(edition.title.as_deref(), Some("煎蛋日报"));
        assert!(edition.description.unwrap().contains("66"));
        assert_eq!(edition.entries.len(), 2);

        let img = &edition.entries[0];
        assert_eq!(img.guid, "jandan:6226724");
        assert_eq!(img.url.as_deref(), Some("https://jandan.net/t/6226724"));
        assert_eq!(img.author.as_deref(), Some("Demaner · 南京"));
        assert!(img.title.contains("别信"), "{}", img.title);
        assert!(img.title.contains("6226724"), "{}", img.title);
        assert!(img
            .content
            .as_deref()
            .unwrap()
            .contains("img.example/a.jpg"));
        // 热评里的 HTML 要转义，不能原样嵌进正文
        assert!(!img.content.as_deref().unwrap().contains("<b>"));
        assert!(img.content.as_deref().unwrap().contains("&lt;b&gt;"));
        assert_eq!(
            img.published_at.as_deref(),
            Some("2026-10-15T01:17:14+00:00")
        );

        let text = &edition.entries[1];
        assert!(text.title.contains("减肥成功了"), "{}", text.title);
        assert_eq!(text.author.as_deref(), Some("啊啊啊 · 青岛"));
    }

    #[test]
    fn parse_jandan_rejects_empty() {
        let body = r#"{"code":0,"msg":"success","data":null}"#;
        assert!(parse_jandan_daily(body).is_err());
        let empty = r#"{"code":0,"msg":"success","data":{"total":0,"list":[]}}"#;
        assert!(parse_jandan_daily(empty).is_err());
    }

    #[test]
    fn parse_html_daily_uses_article_and_strips_scripts() {
        let html = r#"<!doctype html><html><head><title>2026年10月15日 - 某站日报</title></head>
            <body><script>evil()</script>
            <article><p>今天的内容</p><img src="https://example.com/a.jpg"><script>also()</script></article>
            </body></html>"#;
        let edition = parse_html_daily(
            "https://example.com/digest/2026-10-15",
            html,
            "2026-10-15T04:00:00+00:00",
        )
        .unwrap();
        assert_eq!(edition.title.as_deref(), Some("某站日报"));
        assert_eq!(edition.entries.len(), 1);
        let a = &edition.entries[0];
        assert_eq!(a.guid, "https://example.com/digest/2026-10-15");
        assert_eq!(
            a.url.as_deref(),
            Some("https://example.com/digest/2026-10-15")
        );
        assert!(a.title.contains("2026年10月15日"), "{}", a.title);
        let content = a.content.as_deref().unwrap();
        assert!(content.contains("今天的内容"));
        assert!(content.contains("example.com/a.jpg"));
        assert!(!content.contains("evil"));
        assert!(!content.contains("also()"));
        assert_eq!(a.published_at.as_deref(), Some("2026-10-15T04:00:00+00:00"));
    }

    #[test]
    fn parse_html_daily_rejects_empty_shell() {
        let html =
            "<html><head><title>加载中</title></head><body><div>数据加载中</div></body></html>";
        assert!(parse_html_daily(
            "https://example.com/d/2026-10-15",
            html,
            "2026-10-15T04:00:00+00:00"
        )
        .is_err());
    }

    #[test]
    fn jandan_pages_follow_login_window() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        let first = jandan_api_url(day, 1);
        let second = jandan_api_url(day, 2);
        assert!(first.contains("page=1"), "{first}");
        assert!(second.contains("page=2"), "{second}");
        assert!(second.contains("date=2026-10-15"), "{second}");

        // 登录后第 1 页满 50，总共 65，还要翻页
        assert!(jandan_has_more(1, 1, 50, 50, 65, 50));
        // 第 2 页只剩 15，停
        assert!(!jandan_has_more(2, 2, 15, 50, 65, 65));
        // 未登录请求第 2 页时，服务端仍返回 page=1，不能再翻
        assert!(!jandan_has_more(2, 1, 10, 10, 65, 10));
    }

    #[test]
    fn edition_mentions_login_only_when_short() {
        let one = dummy_article("1");
        let partial = jandan_edition(vec![one.clone()], 65, false);
        assert!(partial.description.unwrap().contains("未登录"));
        let full = jandan_edition(vec![one, dummy_article("2")], 2, true);
        let desc = full.description.unwrap();
        assert!(!desc.contains("未登录"), "{desc}");
        assert!(desc.contains("2"), "{desc}");
    }

    fn dummy_article(guid: &str) -> NewArticle {
        NewArticle {
            guid: guid.into(),
            title: "t".into(),
            author: None,
            url: None,
            summary: None,
            content: None,
            published_at: None,
        }
    }
}
