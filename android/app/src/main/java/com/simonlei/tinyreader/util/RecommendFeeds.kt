package com.simonlei.tinyreader.util

import com.simonlei.tinyreader.data.CatalogFeed
import java.net.URL
import java.text.Collator
import java.time.Instant
import java.time.ZoneId
import java.util.Locale

/**
 * 从内置精选目录里按「当前已订阅的源」挑下一批可添加的订阅。
 * 逐条移植桌面端 `src/lib/recommendFeeds.ts`，两边行为保持一致。
 */

const val RECOMMEND_BATCH_SIZE = 6

/** 已订阅源里给推荐算法的线索，对应 `SubscribedFeedHint` */
data class SubscribedFeedHint(val url: String, val title: String, val category: String)

data class RecommendPage(
    val items: List<CatalogFeed>,
    /** 归一化后的批次，从 0 开始 */
    val page: Int,
    val pages: Int,
    /** 去掉已订阅、已展示之后还剩多少条 */
    val total: Int,
)

data class FeedDetailRow(val label: String, val value: String)

data class FeedDetail(
    val description: String,
    val tags: List<String>,
    val facts: List<FeedDetailRow>,
)

/** 目录英文分类 → 侧边栏分组。未知分类原样返回。 */
private val CATEGORY_ZH = mapOf(
    "Artificial Intelligence" to "AI",
    "Engineering & Technology" to "技术",
    "Personal Blogs" to "博客",
    "News" to "新闻",
    "Research & Science" to "科研",
    "Communities" to "社区",
    "Culture & Ideas" to "文化",
    "Technology Media" to "科技媒体",
    "Tech Newsletters & Weeklies" to "周刊",
    "Security" to "安全",
    "Business & Startups" to "商业",
    "Product & Design" to "产品设计",
    "Videos" to "视频",
    "Podcasts" to "播客",
)

/** 分类对应的主题。有的分类没有单独的 pack，靠这个对上关键词。 */
private val CATEGORY_INTEREST = mapOf(
    "Artificial Intelligence" to "ai",
    "Engineering & Technology" to "engineering",
    "Personal Blogs" to "blogs",
    "News" to "news",
    "Research & Science" to "research",
    "Communities" to "communities",
    "Culture & Ideas" to "culture",
    "Technology Media" to "tech-media",
    "Tech Newsletters & Weeklies" to "weeklies",
    "Security" to "security",
    "Business & Startups" to "business",
    "Product & Design" to "product",
    "Videos" to "videos",
    "Podcasts" to "podcasts",
)

private val PACK_HINTS = listOf(
    "ai" to listOf("ai", "llm", "gpt", "openai", "claude", "deepseek", "gemini", "人工智能", "机器学习", "大模型"),
    "security" to listOf("安全", "漏洞", "security", "cve"),
    "news" to listOf("新闻", "资讯"),
    "research" to listOf("论文", "科研", "科学", "arxiv", "research"),
    "communities" to listOf("社区", "论坛", "v2ex", "reddit", "hacker news", "hackernews"),
    "weeklies" to listOf("周刊", "周报", "月刊", "newsletter"),
    "tech-media" to listOf("少数派", "极客公园", "科技媒体"),
    "blogs" to listOf("博客", "随笔"),
    "engineering" to listOf("技术", "编程", "开发", "程序员", "前端", "后端", "架构", "rust", "python", "javascript", "typescript", "golang"),
    "company-tech" to listOf("字节", "阿里", "美团", "腾讯"),
    "product" to listOf("产品设计", "用户体验"),
    "culture" to listOf("文化", "思想"),
    "business" to listOf("创业", "商业"),
)

private val IGNORED_PACKS = setOf("all", "chinese", "top200")

private val PACK_ZH = mapOf(
    "ai" to "AI",
    "engineering" to "技术",
    "blogs" to "博客",
    "top200" to "精选",
    "news" to "新闻",
    "research" to "科研",
    "communities" to "社区",
    "tech-media" to "科技媒体",
    "weeklies" to "周刊",
    "security" to "安全",
    "company-tech" to "机构技术",
    "wechat" to "公众号",
    "chinese" to "中文",
    "videos" to "视频",
    "podcasts" to "播客",
    "product" to "产品设计",
    "culture" to "文化",
    "business" to "商业",
)

private val LANGUAGE_ZH = mapOf("zh" to "中文", "en" to "English")

private val CJK = Regex("[\\u4e00-\\u9fff]")
private val collator: Collator = Collator.getInstance(Locale.SIMPLIFIED_CHINESE)

fun catalogCategoryLabel(category: String): String = CATEGORY_ZH[category] ?: category

/** 列表里能展示的简介、标签和其余字段。空的和分类套话都不占一行。 */
fun feedDetail(feed: CatalogFeed): FeedDetail {
    val facts = mutableListOf<FeedDetailRow>()
    val language = feed.language.trim()
    if (language.isNotEmpty()) facts.add(FeedDetailRow("语言", LANGUAGE_ZH[language] ?: language))
    pushFact(facts, "机构", feed.organization)
    pushFact(facts, "方向", feed.companyDirection)
    pushFact(facts, "站点", feed.site)
    pushFact(facts, "订阅地址", feed.url)
    val updated = formatCatalogDate(feed.latestItemAt)
    if (updated.isNotEmpty()) facts.add(FeedDetailRow("最近更新", updated))
    return FeedDetail(
        description = usefulDescription(feed),
        tags = feed.packs.filter { it != "all" }.map { PACK_ZH[it] ?: it },
        facts = facts,
    )
}

fun recommendFeedPage(
    catalog: List<CatalogFeed>,
    subscribed: List<SubscribedFeedHint>,
    batchSize: Int = RECOMMEND_BATCH_SIZE,
    /** 这一轮对话框里已经展示过的目录 id，换一批时用来接着往下取 */
    excludeIds: Set<String> = emptySet(),
): RecommendPage {
    if (batchSize < 1) return RecommendPage(emptyList(), 0, 0, 0)

    val taken = subscribed.map { feedUrlKey(it.url) }.toSet()
    val available = catalog.filter { it.url.isNotEmpty() && !taken.contains(feedUrlKey(it.url)) }
    val language = preferredLanguage(subscribed)
    val interests = inferInterests(catalog, subscribed)

    val ranked = if (interests.isEmpty()) {
        browseOrder(available, language)
    } else {
        available.filter { overlap(it, interests) > 0 }.sortedWith(byScore(language, interests))
    }
    val unseen = ranked.filter { !excludeIds.contains(it.id) }
    return slicePage(unseen, batchSize)
}

private fun preferredLanguage(subscribed: List<SubscribedFeedHint>): String {
    if (subscribed.isEmpty()) return "zh"
    val blob = subscribed.joinToString("\n") { "${it.title}\n${it.category}" }
    return if (CJK.containsMatchIn(blob)) "zh" else "en"
}

private fun inferInterests(
    catalog: List<CatalogFeed>,
    subscribed: List<SubscribedFeedHint>,
): Set<String> {
    val interests = mutableSetOf<String>()
    for (sub in subscribed) {
        val blob = "${sub.title}\n${sub.category}\n${hostOf(sub.url)}"
        for ((pack, words) in PACK_HINTS) {
            if (words.any { mentions(blob, it) }) interests.add(pack)
        }
    }

    val byHost = mutableMapOf<String, MutableList<CatalogFeed>>()
    for (feed in catalog) {
        for (host in listOf(hostOf(feed.url), hostOf(feed.site))) {
            if (host.isEmpty() || isGenericHost(host)) continue
            byHost.getOrPut(host) { mutableListOf() }.add(feed)
        }
    }
    for (sub in subscribed) {
        val host = hostOf(sub.url)
        if (host.isEmpty() || isGenericHost(host)) continue
        for (feed in byHost[host].orEmpty()) {
            interests.addAll(feedInterests(feed))
        }
    }
    return interests
}

private fun feedInterests(feed: CatalogFeed): List<String> {
    val keys = mutableSetOf<String>()
    for (pack in feed.packs) {
        if (!IGNORED_PACKS.contains(pack)) keys.add(pack)
    }
    CATEGORY_INTEREST[feed.category]?.let { keys.add(it) }
    return keys.toList()
}

private fun overlap(feed: CatalogFeed, interests: Set<String>): Int =
    feedInterests(feed).count { interests.contains(it) }

private fun byScore(language: String, interests: Set<String>): Comparator<CatalogFeed> =
    Comparator { a, b ->
        val diff = score(b, language, interests) - score(a, language, interests)
        if (diff != 0) diff
        else {
            val title = collator.compare(a.title, b.title)
            if (title != 0) title else collator.compare(a.id, b.id)
        }
    }

private fun score(feed: CatalogFeed, language: String, interests: Set<String>): Int {
    var value = overlap(feed, interests) * 5
    if (feed.language == language) value += 2
    if (feed.packs.contains("top200")) value += 1
    return value
}

/** 没有可匹配的主题时，按分类轮转，避免第一批全是同一个分类。 */
private fun browseOrder(feeds: List<CatalogFeed>, language: String): List<CatalogFeed> {
    val groups = linkedMapOf<String, MutableList<CatalogFeed>>()
    for (feed in feeds) {
        groups.getOrPut(feed.category) { mutableListOf() }.add(feed)
    }
    val quality = { feed: CatalogFeed ->
        (if (feed.language == language) 2 else 0) + (if (feed.packs.contains("top200")) 1 else 0)
    }
    for (list in groups.values) {
        list.sortWith { a, b ->
            val diff = quality(b) - quality(a)
            if (diff != 0) diff
            else {
                val title = collator.compare(a.title, b.title)
                if (title != 0) title else collator.compare(a.id, b.id)
            }
        }
    }
    val categories = groups.entries.sortedWith { a, b ->
        val diff = quality(b.value[0]) - quality(a.value[0])
        if (diff != 0) diff else collator.compare(a.key, b.key)
    }

    val ordered = mutableListOf<CatalogFeed>()
    val max = categories.maxOfOrNull { it.value.size } ?: 0
    for (index in 0 until max) {
        for ((_, list) in categories) {
            list.getOrNull(index)?.let { ordered.add(it) }
        }
    }
    return ordered
}

private fun slicePage(items: List<CatalogFeed>, batchSize: Int): RecommendPage {
    if (items.isEmpty()) return RecommendPage(emptyList(), 0, 0, 0)
    val pages = (items.size + batchSize - 1) / batchSize
    return RecommendPage(items.take(batchSize), 0, pages, items.size)
}

private fun feedUrlKey(raw: String): String {
    val trimmed = raw.trim()
    return try {
        val url = URL(trimmed)
        val host = url.host.removePrefix("www.").lowercase(Locale.ROOT)
        val path = url.path.trimEnd('/').lowercase(Locale.ROOT)
        val query = (url.query ?: "").lowercase(Locale.ROOT)
        "$host$path$query"
    } catch (_: Exception) {
        trimmed.lowercase(Locale.ROOT).trimEnd('/')
    }
}

private fun hostOf(raw: String): String = try {
    URL(raw.trim()).host.removePrefix("www.").lowercase(Locale.ROOT)
} catch (_: Exception) {
    ""
}

private fun isGenericHost(host: String): Boolean =
    host.startsWith("rsshub.") || host.contains("rsshub")

private fun mentions(text: String, word: String): Boolean {
    if (CJK.containsMatchIn(word)) return text.lowercase(Locale.ROOT).contains(word.lowercase(Locale.ROOT))
    val escaped = escapeRegex(word)
    return Regex("(?:^|[^a-z0-9])$escaped(?:[^a-z0-9]|$)", RegexOption.IGNORE_CASE).containsMatchIn(text)
}

private val REGEX_META = setOf('.', '*', '+', '?', '^', '$', '{', '}', '(', ')', '|', '[', ']', '\\')

private fun escapeRegex(word: String): String {
    val out = StringBuilder(word.length)
    for (ch in word) {
        if (REGEX_META.contains(ch)) out.append('\\')
        out.append(ch)
    }
    return out.toString()
}

private fun usefulDescription(feed: CatalogFeed): String {
    val text = (feed.description ?: "").trim()
    if (text.isEmpty() || text == "${feed.category} 订阅源") return ""
    return text
}

private fun pushFact(facts: MutableList<FeedDetailRow>, label: String, value: String?) {
    val text = (value ?: "").trim()
    if (text.isNotEmpty()) facts.add(FeedDetailRow(label, text))
}

private fun formatCatalogDate(iso: String?): String {
    val text = (iso ?: "").trim()
    if (text.isEmpty()) return ""
    val instant = try {
        Instant.parse(text)
    } catch (_: Exception) {
        return text
    }
    return instant.atZone(ZoneId.of("Asia/Shanghai")).toLocalDate().toString()
}
