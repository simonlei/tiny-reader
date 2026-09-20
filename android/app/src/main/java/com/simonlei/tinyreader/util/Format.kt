package com.simonlei.tinyreader.util

import java.time.Instant
import java.time.LocalDateTime
import java.time.OffsetDateTime
import java.time.ZoneId
import java.time.ZonedDateTime
import java.time.format.DateTimeFormatter

/**
 * 时间与文本工具，逐条对应桌面端 `src/lib/format.ts`。
 */

private fun parseInstant(raw: String): Instant? {
    val s = raw.trim()
    if (s.isEmpty()) return null
    // 服务端写的是 RFC3339；旧数据可能是 "yyyy-MM-dd HH:mm:ss"
    runCatching { return OffsetDateTime.parse(s).toInstant() }
    runCatching { return Instant.parse(s) }
    runCatching {
        return LocalDateTime
            .parse(s, DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss"))
            .atZone(ZoneId.systemDefault())
            .toInstant()
    }
    return null
}

/**
 * 相对时间：今天显示 HH:mm，今年显示 M月D日，更早显示 YYYY-MM-DD。
 *
 * @param iso RFC3339 时间字符串
 * @param fallback 为空时的兜底时间。传 null 表示「没有真实发表时间就不显示」，
 *   避免把拉取时间伪装成发表时间（与桌面端一致）。
 */
fun formatTime(iso: String?, fallback: String? = null): String {
    val raw = iso?.takeIf { it.isNotBlank() } ?: fallback?.takeIf { it.isNotBlank() } ?: return ""
    val instant = parseInstant(raw) ?: return ""
    val d: ZonedDateTime = instant.atZone(ZoneId.systemDefault())
    val now = ZonedDateTime.now(ZoneId.systemDefault())

    val sameDay = d.year == now.year && d.monthValue == now.monthValue && d.dayOfMonth == now.dayOfMonth
    if (sameDay) return "%02d:%02d".format(d.hour, d.minute)
    if (d.year == now.year) return "${d.monthValue}月${d.dayOfMonth}日"
    return "%04d-%02d-%02d".format(d.year, d.monthValue, d.dayOfMonth)
}

/** 去掉 HTML 标签，用于列表摘要 */
fun stripHtml(html: String?): String {
    if (html.isNullOrEmpty()) return ""
    return html
        .replace(Regex("<script[\\s\\S]*?</script>", RegexOption.IGNORE_CASE), " ")
        .replace(Regex("<style[\\s\\S]*?</style>", RegexOption.IGNORE_CASE), " ")
        .replace(Regex("<br\\s*/?>", RegexOption.IGNORE_CASE), " ")
        .replace(Regex("</(p|div|li|h[1-6])>", RegexOption.IGNORE_CASE), " ")
        .replace(Regex("<[^>]+>"), "")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace(Regex("\\s+"), " ")
        .trim()
}

/** 截断到指定长度 */
fun truncate(s: String, n: Int): String = if (s.length > n) s.take(n) + "…" else s
