package com.simonlei.tinyreader.data

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/**
 * 与服务端 `server/src/models.rs` / 桌面端 `src/api/types.ts` 一一对应。
 * 服务端返回的是 snake_case，这里用 @SerialName 映射成 Kotlin 习惯的 camelCase。
 */

@Serializable
data class Feed(
    val id: Long,
    val url: String = "",
    val title: String = "",
    @SerialName("site_url") val siteUrl: String? = null,
    val description: String? = null,
    /** '' 表示未分组 */
    val category: String = "",
    @SerialName("last_fetched_at") val lastFetchedAt: String? = null,
    @SerialName("last_error") val lastError: String? = null,
    @SerialName("created_at") val createdAt: String = "",
    @SerialName("unread_count") val unreadCount: Int = 0,
    @SerialName("total_count") val totalCount: Int = 0,
)

@Serializable
data class Article(
    val id: Long,
    @SerialName("feed_id") val feedId: Long = 0,
    val guid: String = "",
    val title: String = "",
    val author: String? = null,
    val url: String? = null,
    /** HTML */
    val summary: String? = null,
    /** HTML，可能为 null → 回退到 summary */
    val content: String? = null,
    @SerialName("published_at") val publishedAt: String? = null,
    @SerialName("fetched_at") val fetchedAt: String = "",
    @SerialName("is_read") val isRead: Boolean = false,
    @SerialName("is_starred") val isStarred: Boolean = false,
    @SerialName("feed_title") val feedTitle: String? = null,
    @SerialName("feed_site_url") val feedSiteUrl: String? = null,
)

@Serializable
data class ArticlePage(
    val items: List<Article> = emptyList(),
    val total: Int = 0,
    val offset: Int = 0,
    val limit: Int = 0,
)

@Serializable
data class Stats(
    val total: Int = 0,
    val unread: Int = 0,
    val starred: Int = 0,
)

@Serializable
data class FeedError(
    @SerialName("feed_id") val feedId: Long = 0,
    val title: String = "",
    val message: String = "",
)

@Serializable
data class RefreshStatus(
    val running: Boolean = false,
    val total: Int = 0,
    val done: Int = 0,
    @SerialName("new_articles") val newArticles: Int = 0,
    val errors: List<FeedError> = emptyList(),
    @SerialName("started_at") val startedAt: String? = null,
    @SerialName("finished_at") val finishedAt: String? = null,
)

@Serializable
data class ImportResult(
    val ok: Boolean = false,
    val added: Int = 0,
    val skipped: Int = 0,
)

@Serializable
data class OkResult(
    val ok: Boolean = false,
)

@Serializable
data class RefreshFeedResult(
    val ok: Boolean = false,
    @SerialName("new_articles") val newArticles: Int = 0,
)

@Serializable
data class MarkReadResult(
    val ok: Boolean = false,
    val updated: Int = 0,
)

/** 文章过滤器，对应 TS 的 `ArticleFilter = 'all' | 'unread' | 'starred'` */
enum class ArticleFilter(val key: String, val label: String) {
    ALL("all", "全部"),
    UNREAD("unread", "未读"),
    STARRED("starred", "星标"),
}

// ------------------------------------------------------------------ 请求体

@Serializable
data class NewFeedBody(
    val url: String,
    val title: String? = null,
    val category: String? = null,
)

@Serializable
data class UpdateFeedBody(
    val url: String? = null,
    val title: String? = null,
    val category: String? = null,
)

@Serializable
internal data class ValueBody(val value: Boolean)

@Serializable
internal data class MarkReadBody(
    @SerialName("feed_id") val feedId: Long? = null,
    val ids: List<Long>? = null,
)

@Serializable
internal data class ImportBody(
    val opml: String,
    @SerialName("fetch_now") val fetchNow: Boolean = true,
)
