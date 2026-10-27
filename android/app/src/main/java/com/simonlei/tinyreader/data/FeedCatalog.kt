package com.simonlei.tinyreader.data

import android.content.Context
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/**
 * 内置精选目录里的一条订阅，对应桌面端 `src/lib/recommendFeeds.ts` 的 `CatalogFeed`。
 */
@Serializable
data class CatalogFeed(
    val id: String = "",
    val title: String = "",
    val url: String = "",
    val site: String = "",
    val category: String = "",
    val language: String = "",
    val packs: List<String> = emptyList(),
    val description: String? = null,
    @SerialName("latest_item_at") val latestItemAt: String? = null,
    val organization: String? = null,
    @SerialName("company_direction") val companyDirection: String? = null,
)

@Serializable
private data class FeedCatalogFile(
    val source: String = "",
    val feeds: List<CatalogFeed> = emptyList(),
)

/**
 * 订阅推荐目录。数据文件与桌面端共用仓库里的 `src/data/feedCatalog.json`，
 * 构建时由 Gradle 拷进 assets（见 app/build.gradle.kts 的 copyFeedCatalog）。
 */
object FeedCatalog {

    private val json = Json { ignoreUnknownKeys = true }

    @Volatile
    private var cache: FeedCatalogFile? = null

    /** 目录里的全部源。首次调用会读 assets，请在 IO 线程调用。 */
    fun load(context: Context): List<CatalogFeed> = file(context).feeds

    /** 目录出处，用于界面上的来源说明 */
    fun source(context: Context): String = file(context).source

    private fun file(context: Context): FeedCatalogFile {
        cache?.let { return it }
        val text = context.assets.open("feedCatalog.json").bufferedReader().use { it.readText() }
        val parsed = json.decodeFromString<FeedCatalogFile>(text)
        cache = parsed
        return parsed
    }
}
