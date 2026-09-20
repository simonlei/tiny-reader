package com.simonlei.tinyreader.data

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.decodeFromString
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import okhttp3.HttpUrl.Companion.toHttpUrlOrNull
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

/** 对应桌面端 `src/api/client.ts` 的 ApiError */
class ApiError(val status: Int, message: String) : Exception(message) {
    /** 401 / 403 都视为 token 无效 */
    val isUnauthorized: Boolean get() = status == 401 || status == 403
}

/**
 * 服务端 HTTP 客户端，逐条对应 `src/api/client.ts`。
 * 所有 `/api` 下的请求都带 `Authorization: Bearer <token>`。
 */
object ApiClient {

    private val json = Json {
        ignoreUnknownKeys = true
        encodeDefaults = true
        explicitNulls = true
    }

    private val jsonMediaType = "application/json; charset=utf-8".toMediaType()

    private val http: OkHttpClient = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .retryOnConnectionFailure(true)
        .build()

    @Volatile
    private var serverUrl: String = ""

    @Volatile
    private var token: String = ""

    /** 与 TS 版一致：去掉结尾所有 `/`，token 去空白 */
    fun configure(url: String, accessToken: String) {
        serverUrl = url.trim().trimEnd('/')
        token = accessToken.trim()
    }

    fun currentServerUrl(): String = serverUrl

    private fun ensureConfigured() {
        if (serverUrl.isBlank()) throw ApiError(0, "尚未配置服务端地址")
        if (token.isBlank()) throw ApiError(0, "尚未配置访问 token")
    }

    // ---------------------------------------------------------------- 底层请求

    private suspend fun raw(path: String, method: String = "GET", body: String? = null): String =
        withContext(Dispatchers.IO) {
            ensureConfigured()
            val url = "$serverUrl$path"
            if (url.toHttpUrlOrNull() == null) throw ApiError(0, "服务端地址不合法：$serverUrl")

            val requestBody = when {
                body != null -> body.toRequestBody(jsonMediaType)
                method != "GET" && method != "DELETE" -> "".toRequestBody(jsonMediaType)
                else -> null
            }

            val request = Request.Builder()
                .url(url)
                .method(method, requestBody)
                .header("Authorization", "Bearer $token")
                .header("Content-Type", "application/json")
                .header("Accept", "application/json")
                .build()

            val response = try {
                http.newCall(request).execute()
            } catch (e: IOException) {
                throw ApiError(0, "无法连接服务端（$serverUrl）：${e.message}")
            }

            response.use { resp ->
                val text = resp.body?.string().orEmpty()
                if (!resp.isSuccessful) {
                    // 服务端统一错误体是 {"error": "..."}
                    val msg = runCatching {
                        json.parseToJsonElement(text).jsonObject["error"]?.jsonPrimitive?.content
                    }.getOrNull() ?: "HTTP ${resp.code}"
                    throw ApiError(resp.code, msg)
                }
                text
            }
        }

    private fun queryOf(params: List<Pair<String, Any?>>): String {
        // 与 TS 版一致：跳过 undefined/null/空串，但不跳过 false
        val parts = params.mapNotNull { (k, v) ->
            if (v == null || v == "") null else "$k=${java.net.URLEncoder.encode(v.toString(), "UTF-8")}"
        }
        return if (parts.isEmpty()) "" else "?" + parts.joinToString("&")
    }

    // ------------------------------------------------------------------ 连通性

    /** GET /health（不带 token） */
    suspend fun health(): Boolean = withContext(Dispatchers.IO) {
        ensureConfigured()
        runCatching {
            val request = Request.Builder().url("$serverUrl/health").build()
            http.newCall(request).execute().use { it.isSuccessful }
        }.getOrDefault(false)
    }

    // ------------------------------------------------------------------- 订阅源

    suspend fun listFeeds(): List<Feed> =
        json.decodeFromString(raw("/api/feeds"))

    suspend fun getFeed(id: Long): Feed =
        json.decodeFromString(raw("/api/feeds/$id"))

    suspend fun addFeed(url: String, title: String?, category: String?): Feed =
        json.decodeFromString(
            raw("/api/feeds", "POST", json.encodeToString(NewFeedBody(url, title, category)))
        )

    suspend fun updateFeed(id: Long, patch: UpdateFeedBody): Feed =
        json.decodeFromString(
            raw("/api/feeds/$id", "PUT", json.encodeToString(patch))
        )

    suspend fun deleteFeed(id: Long): OkResult =
        json.decodeFromString(raw("/api/feeds/$id", "DELETE"))

    suspend fun refreshFeed(id: Long): RefreshFeedResult =
        json.decodeFromString(raw("/api/feeds/$id/refresh", "POST"))

    suspend fun importOpml(xml: String, fetchNow: Boolean = true): ImportResult =
        json.decodeFromString(
            raw("/api/feeds/import", "POST", json.encodeToString(ImportBody(xml, fetchNow)))
        )

    /** GET /api/feeds/export，返回 OPML 原文（需要 token） */
    suspend fun exportOpml(): String = raw("/api/feeds/export")

    // --------------------------------------------------------------------- 文章

    suspend fun listArticles(
        feedId: Long?,
        filter: ArticleFilter,
        limit: Int,
        offset: Int,
        keyword: String?,
    ): ArticlePage {
        val q = queryOf(
            listOf(
                "feed_id" to feedId,
                "unread_only" to (filter == ArticleFilter.UNREAD),
                "starred_only" to (filter == ArticleFilter.STARRED),
                "limit" to limit,
                "offset" to offset,
                "q" to keyword,
            )
        )
        return json.decodeFromString(raw("/api/articles$q"))
    }

    suspend fun getArticle(id: Long): Article =
        json.decodeFromString(raw("/api/articles/$id"))

    suspend fun setRead(id: Long, value: Boolean): OkResult =
        json.decodeFromString(
            raw("/api/articles/$id/read", "POST", json.encodeToString(ValueBody(value)))
        )

    suspend fun setStarred(id: Long, value: Boolean): OkResult =
        json.decodeFromString(
            raw("/api/articles/$id/star", "POST", json.encodeToString(ValueBody(value)))
        )

    suspend fun markRead(feedId: Long? = null, ids: List<Long>? = null): MarkReadResult =
        json.decodeFromString(
            raw("/api/articles/mark-read", "POST", json.encodeToString(MarkReadBody(feedId, ids)))
        )

    suspend fun stats(): Stats =
        json.decodeFromString(raw("/api/stats"))

    // --------------------------------------------------------------------- 刷新

    suspend fun refreshAll(): String = raw("/api/refresh", "POST")

    suspend fun refreshStatus(): RefreshStatus =
        json.decodeFromString(raw("/api/refresh/status"))
}
