package com.simonlei.tinyreader.util

import com.simonlei.tinyreader.data.Article
import java.net.URI

/**
 * 正文 HTML 处理，对应桌面端 `src/lib/html.ts` + `ArticleView.vue` 的渲染。
 *
 * 很多源（如 Hugo 生成的站点）正文里的图片是根相对路径 `<img src="/images/x.png">`，
 * 服务端按原样保存，客户端渲染前需要按 feed_site_url 的 origin 补成绝对地址。
 */

private fun isAbsoluteUrl(u: String): Boolean =
    u.startsWith("http://") ||
        u.startsWith("https://") ||
        u.startsWith("//") ||
        u.startsWith("data:") ||
        u.startsWith("blob:") ||
        u.startsWith("#") ||
        u.startsWith("mailto:") ||
        u.startsWith("javascript:")

private fun resolveUrl(u: String, base: String): String {
    val rawUrl = u.trim()
    if (rawUrl.isEmpty() || isAbsoluteUrl(rawUrl)) return rawUrl
    return runCatching { URI(base).resolve(rawUrl).toString() }.getOrDefault(rawUrl)
}

/** 取 origin（scheme://host[:port]），避免把 /feed/ 这种路径带进来 */
private fun originOf(base: String): String? {
    val uri = runCatching { URI(base) }.getOrNull() ?: return null
    val scheme = uri.scheme ?: return null
    val host = uri.host ?: return null
    val port = if (uri.port > 0) ":${uri.port}" else ""
    return "$scheme://$host$port/"
}

/** 补全 HTML 里 img/@src 与 a/@href 的相对地址 */
fun absolutizeHtml(html: String, base: String?): String {
    if (html.isEmpty()) return html
    if (base.isNullOrBlank()) return html
    val root = originOf(base) ?: return html

    val imgRe = Regex("(<img\\b[^>]*?\\bsrc\\s*=\\s*)([\"'])(.*?)\\2", RegexOption.IGNORE_CASE)
    val aRe = Regex("(<a\\b[^>]*?\\bhref\\s*=\\s*)([\"'])(.*?)\\2", RegexOption.IGNORE_CASE)

    fun rewrite(input: String, re: Regex): String = re.replace(input) { m ->
        val prefix = m.groupValues[1]
        val quote = m.groupValues[2]
        val url = m.groupValues[3]
        val resolved = resolveUrl(url, root)
        if (resolved == url) m.value else "$prefix$quote$resolved$quote"
    }

    return rewrite(rewrite(html, imgRe), aRe)
}

private fun escapeHtml(s: String): String = s
    .replace("&", "&amp;")
    .replace("<", "&lt;")
    .replace(">", "&gt;")
    .replace("\"", "&quot;")

/**
 * 把一篇文章渲染成完整的 HTML 页面（含标题头），交给 WebView 显示。
 *
 * 之所以把标题/来源也放进 HTML，而不是用 Compose 组件叠在 WebView 上方，
 * 是为了让整页只有 WebView 一个垂直滚动容器，避免嵌套滚动的手势冲突。
 * 样式取自桌面端 `src/styles/main.css` 的正文排版。
 */
fun buildArticleHtml(article: Article): String {
    val rawBody = article.content?.takeIf { it.isNotBlank() }
        ?: article.summary?.takeIf { it.isNotBlank() }
        ?: ""
    val body = absolutizeHtml(rawBody, article.feedSiteUrl)

    val onlySummary = article.content.isNullOrBlank() && !article.summary.isNullOrBlank()

    val metaParts = buildList {
        add(escapeHtml(article.feedTitle?.takeIf { it.isNotBlank() } ?: "未知来源"))
        formatTime(article.publishedAt).takeIf { it.isNotEmpty() }?.let { add(escapeHtml(it)) }
        article.author?.takeIf { it.isNotBlank() }?.let { add(escapeHtml(it)) }
    }

    val titleText = escapeHtml(article.title.ifBlank { "(无标题)" })
    val titleHtml = if (!article.url.isNullOrBlank()) {
        """<a class="tr-title-link" href="${escapeHtml(article.url)}">$titleText</a>"""
    } else {
        titleText
    }

    val hint = if (onlySummary) {
        """<div class="tr-hint">该源只提供了摘要，点标题可在浏览器里查看全文。</div>"""
    } else {
        ""
    }

    val emptyBody = if (body.isBlank()) {
        """<div class="tr-hint">这篇文章没有正文内容，点标题可在浏览器里查看原文。</div>"""
    } else {
        ""
    }

    return """<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=5">
<style>
  :root { color-scheme: dark; }
  html, body { margin: 0; padding: 0; background: #14161a; }
  body {
    color: #d8dce2;
    font-family: -apple-system, "Noto Sans CJK SC", "Source Han Sans", Roboto, sans-serif;
    font-size: 17px;
    line-height: 1.78;
    padding: 16px 18px 56px;
    -webkit-text-size-adjust: 100%;
    overflow-wrap: break-word;
    word-break: break-word;
  }
  .tr-meta { color: #6f7887; font-size: 13px; margin-bottom: 6px; }
  h1.tr-title { font-size: 22px; line-height: 1.4; margin: 0 0 14px; color: #e6e8eb; font-weight: 600; }
  h1.tr-title a.tr-title-link { color: #e6e8eb; text-decoration: none; }
  .tr-sep { border: 0; border-top: 1px solid #2b3038; margin: 0 0 16px; }
  .tr-hint {
    color: #a2abb9; background: #1a1d23; border: 1px solid #2b3038;
    border-radius: 8px; padding: 10px 12px; font-size: 14px; margin: 0 0 16px;
  }
  a { color: #4c8dff; }
  img, video, iframe { max-width: 100%; height: auto; border-radius: 6px; }
  pre {
    background: #101216; border: 1px solid #2b3038; border-radius: 8px;
    padding: 12px; overflow-x: auto; font-size: 14px;
  }
  code { background: #21252d; border-radius: 4px; padding: 1px 4px; font-size: 14px; }
  pre code { background: none; padding: 0; }
  blockquote {
    margin: 16px 0; padding: 2px 14px; border-left: 3px solid #2b3038; color: #a2abb9;
  }
  table { display: block; max-width: 100%; overflow-x: auto; border-collapse: collapse; }
  td, th { border: 1px solid #2b3038; padding: 6px 8px; }
  hr { border: 0; border-top: 1px solid #2b3038; }
</style>
</head>
<body>
  <div class="tr-meta">${metaParts.joinToString(" · ")}</div>
  <h1 class="tr-title">$titleHtml</h1>
  <hr class="tr-sep">
  $hint
  $emptyBody
  $body
</body>
</html>"""
}
