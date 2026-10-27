package com.simonlei.tinyreader.util

import com.simonlei.tinyreader.data.CatalogFeed
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * 对应桌面端 `src/lib/recommendFeeds.test.ts` 里与推荐排序有关的部分。
 * 两边用例同构，避免移植时把排序改歪。
 */
class RecommendFeedsTest {

    private val catalog = listOf(
        CatalogFeed(
            id = "a1", title = "OpenAI", url = "https://openai.com/feed",
            site = "https://openai.com", category = "Artificial Intelligence",
            language = "en", packs = listOf("ai", "top200"),
        ),
        CatalogFeed(
            id = "a2", title = "DeepSeek", url = "https://deepseek.com/feed",
            site = "https://deepseek.com", category = "Artificial Intelligence",
            language = "zh", packs = listOf("ai", "top200"),
        ),
        CatalogFeed(
            id = "e1", title = "Rust Blog", url = "https://blog.rust-lang.org/feed.xml",
            site = "https://blog.rust-lang.org", category = "Engineering & Technology",
            language = "en", packs = listOf("engineering", "top200"),
        ),
        CatalogFeed(
            id = "b1", title = "某博客", url = "https://example.com/atom",
            site = "https://example.com", category = "Personal Blogs",
            language = "zh", packs = listOf("blogs"),
        ),
        CatalogFeed(
            id = "n1", title = "每日新闻", url = "https://news.example/rss",
            site = "https://news.example", category = "News",
            language = "zh", packs = listOf("news", "top200"),
            description = "News 订阅源",
        ),
        CatalogFeed(
            id = "s1", title = "安全通告", url = "https://sec.example/rss",
            site = "https://sec.example", category = "Security",
            language = "zh", packs = listOf("security"),
        ),
    )

    @Test
    fun 分类名显示成侧边栏用的中文分组() {
        assertEquals("AI", catalogCategoryLabel("Artificial Intelligence"))
        assertEquals("技术", catalogCategoryLabel("Engineering & Technology"))
        assertEquals("没见过的分类", catalogCategoryLabel("没见过的分类"))
    }

    @Test
    fun 没有订阅时第一批分散在不同分类并优先中文() {
        val page = recommendFeedPage(catalog, emptyList(), 4)

        assertEquals(listOf("a2", "n1", "b1", "s1"), page.items.map { it.id })
        assertEquals(2, page.pages)
        assertEquals(6, page.total)
        assertTrue(page.items.all { it.language == "zh" })
        assertEquals(4, page.items.map { it.category }.distinct().size)
    }

    @Test
    fun 已订阅的地址换协议www或末尾斜杠后仍然排除() {
        val page = recommendFeedPage(
            catalog,
            listOf(SubscribedFeedHint("https://www.OpenAI.com/feed/", "OpenAI", "AI")),
            6,
        )

        assertTrue(page.items.none { it.id == "a1" })
        assertEquals(listOf("a2"), page.items.map { it.id })
    }

    @Test
    fun 按已订阅源的标题匹配主题时中文同类排在前面() {
        val subscribed = listOf(SubscribedFeedHint("https://mine.example/rss", "LLM 周刊", "AI"))
        val page = recommendFeedPage(catalog, subscribed, 6)

        assertEquals(listOf("a2", "a1"), page.items.map { it.id })
        assertEquals(1, page.pages)
    }

    @Test
    fun 换一批时跳过这一轮已经展示过的源() {
        val first = recommendFeedPage(catalog, emptyList(), 2)
        val second = recommendFeedPage(catalog, emptyList(), 2, first.items.map { it.id }.toSet())

        assertEquals(listOf("a2", "n1"), first.items.map { it.id })
        assertEquals(listOf("b1", "s1"), second.items.map { it.id })
        assertTrue(second.items.none { item -> first.items.any { it.id == item.id } })
    }

    @Test
    fun 目录里的源都订阅过时没有推荐() {
        val all = catalog.map { SubscribedFeedHint(it.url, it.title, catalogCategoryLabel(it.category)) }
        val page = recommendFeedPage(catalog, all, 6)

        assertTrue(page.items.isEmpty())
        assertEquals(0, page.pages)
    }

    @Test
    fun feedDetail里分类套话和空字段不占一行() {
        val detail = feedDetail(catalog.first { it.id == "n1" })

        assertEquals("", detail.description)
        assertEquals(listOf("新闻", "精选"), detail.tags)
        assertEquals(listOf("语言", "站点", "订阅地址"), detail.facts.map { it.label })
        assertTrue(detail.facts.any { it.label == "订阅地址" && it.value == "https://news.example/rss" })
    }
}
