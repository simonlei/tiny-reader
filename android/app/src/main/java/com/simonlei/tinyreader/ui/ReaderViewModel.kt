package com.simonlei.tinyreader.ui

import android.app.Application
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.simonlei.tinyreader.data.ApiClient
import com.simonlei.tinyreader.data.ApiError
import com.simonlei.tinyreader.data.Article
import com.simonlei.tinyreader.data.ArticleFilter
import com.simonlei.tinyreader.data.CatalogFeed
import com.simonlei.tinyreader.data.Cursor
import com.simonlei.tinyreader.data.Feed
import com.simonlei.tinyreader.data.FeedCatalog
import com.simonlei.tinyreader.data.MediaHosts
import com.simonlei.tinyreader.data.RefreshStatus
import com.simonlei.tinyreader.data.SettingsStore
import com.simonlei.tinyreader.data.Stats
import com.simonlei.tinyreader.data.UpdateFeedBody
import com.simonlei.tinyreader.util.MediaProxy
import com.simonlei.tinyreader.util.RECOMMEND_BATCH_SIZE
import com.simonlei.tinyreader.util.SubscribedFeedHint
import com.simonlei.tinyreader.util.catalogCategoryLabel
import com.simonlei.tinyreader.util.recommendFeedPage
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.withContext
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

/** 每页条数，与桌面端一致 */
private const val PAGE_SIZE = 50

/** 选中后多久自动标记已读（毫秒），与桌面端 AUTO_READ_DELAY 一致 */
private const val AUTO_READ_DELAY = 600L

data class UiState(
    val connected: Boolean = false,
    val loading: Boolean = false,
    val loadingMore: Boolean = false,
    val error: String = "",

    val feeds: List<Feed> = emptyList(),
    val articles: List<Article> = emptyList(),
    val total: Int = 0,

    val selectedFeedId: Long? = null,
    val filter: ArticleFilter = ArticleFilter.UNREAD,
    val selectedId: Long? = null,
    val keyword: String = "",

    val stats: Stats = Stats(),

    val refreshing: Boolean = false,
    val refreshInfo: RefreshStatus = RefreshStatus(),
    /** 从服务端重载当前文章列表（不抓 RSS），对应桌面端 `reloading` */
    val reloading: Boolean = false,
    /**
     * 刷新完成后让列表滚回顶部的令牌。每次刷新 +1，界面比对到变化就滚一次；
     * 用递增整数而不是 Boolean，避免「刷新期间界面离开组合」丢掉这次事件。
     */
    val scrollTopToken: Int = 0,

    // --- 移动端特有的界面状态 ---
    val serverUrl: String = "",
    val token: String = "",
    val needsSetup: Boolean = true,
    /** 阅读页是否打开 */
    val readerOpen: Boolean = false,
    /** 一次性提示（OPML 导入结果等） */
    val toast: String? = null,

    /**
     * 下一页游标（服务端下发）。
     * 未读过滤下结果集随阅读收缩，OFFSET 分页会跳过文章，必须用 keyset 游标。
     * 为 null 表示没有更多。
     */
    val cursor: Cursor? = null,

    /** 上一页是否返回满页，仅作兜底显示用 */
    val lastPageFull: Boolean = false,

    /** 服务端配置的图片代理域名 */
    val mediaHosts: List<String> = emptyList(),
    /** 本机图片代理根地址。空字符串表示还没起来，正文退回服务端代拉。 */
    val localMediaBase: String = "",

    // --- 订阅推荐（对应桌面端 RecommendDialog.vue 的局部状态） ---
    val recommendItems: List<CatalogFeed> = emptyList(),
    val recommendPage: Int = 0,
    val recommendPages: Int = 0,
    /** 本次打开对话框后已经添加过的目录 id */
    val recommendAdded: Set<String> = emptySet(),
    /** 正在添加的目录 id */
    val recommendPending: String? = null,
    val recommendErrors: Map<String, String> = emptyMap(),
    val recommendSource: String = "",
    val recommendCatalogSize: Int = 0,
) {
    /** 是否还有下一页：以服务端游标为准，不受已读导致的 total 递减影响 */
    val hasMore: Boolean get() = cursor != null

    val selectedIndex: Int get() = articles.indexOfFirst { it.id == selectedId }

    val selectedArticle: Article? get() = articles.firstOrNull { it.id == selectedId }

    val currentFeedTitle: String
        get() = if (selectedFeedId == null) "全部文章"
        else feeds.firstOrNull { it.id == selectedFeedId }?.title ?: "全部文章"

    val failedFeedCount: Int get() = refreshInfo.errors.size
}

/**
 * 应用状态机，逐条移植桌面端 `src/stores/app.ts`。
 * 所有阅读状态都在服务端，本地只做乐观更新 + 回拉对账。
 */
class ReaderViewModel(app: Application) : AndroidViewModel(app) {

    private val _ui = MutableStateFlow(UiState())
    val ui: StateFlow<UiState> = _ui.asStateFlow()

    /** 自动已读定时器：全局只保留一个，新的选中会取消上一个 */
    private var autoReadJob: Job? = null

    /** 内置订阅目录，第一次打开推荐时从 assets 读一次 */
    private var catalog: List<CatalogFeed> = emptyList()
    private var catalogSource: String = ""
    /** 推荐对话框这一轮已经展示过的目录 id，换一批时接着往下取 */
    private var recommendShown: MutableSet<String> = mutableSetOf()

    init {
        val saved = SettingsStore.load()
        ApiClient.configure(saved.serverUrl, saved.token)
        _ui.update {
            it.copy(
                serverUrl = saved.serverUrl,
                token = saved.token,
                needsSetup = saved.needsSetup,
            )
        }
        if (!saved.needsSetup) bootstrap()
    }

    // ------------------------------------------------------------------ 错误处理

    private fun handleError(e: Throwable) {
        // 对应桌面端 handleError 里的 console.error
        Log.e("tiny-reader", "request failed", e)
        if (e is ApiError) {
            _ui.update {
                it.copy(
                    connected = e.status != 0,
                    error = if (e.isUnauthorized) "token 无效，请在设置里重新填写" else (e.message ?: "请求失败"),
                )
            }
        } else {
            _ui.update { it.copy(connected = false, error = e.message ?: e.toString()) }
        }
    }

    fun dismissError() = _ui.update { it.copy(error = "") }

    fun showToast(msg: String) = _ui.update { it.copy(toast = msg) }

    fun dismissToast() = _ui.update { it.copy(toast = null) }

    // ------------------------------------------------------------------ 加载数据

    private suspend fun doLoadFeeds() {
        try {
            val feeds = ApiClient.listFeeds()
            _ui.update { it.copy(feeds = feeds, error = "", connected = true) }
        } catch (e: Throwable) {
            handleError(e)
        }
    }

    private suspend fun doLoadStats() {
        try {
            val stats = ApiClient.stats()
            _ui.update { it.copy(stats = stats) }
        } catch (e: Throwable) {
            handleError(e)
        }
    }

    private suspend fun doLoadArticles(reset: Boolean) {
        val snapshot = _ui.value
        if (reset) {
            if (snapshot.loading) return
            _ui.update { it.copy(loading = true) }
        } else {
            if (snapshot.loadingMore || !snapshot.hasMore) return
            _ui.update { it.copy(loadingMore = true) }
        }

        try {
            val cur = _ui.value
            val page = ApiClient.listArticles(
                feedId = cur.selectedFeedId,
                filter = cur.filter,
                limit = PAGE_SIZE,
                // 未读模式下结果集随阅读收缩，OFFSET 会跳过文章，统一用游标翻页
                offset = 0,
                keyword = cur.keyword.trim().ifBlank { null },
                cursor = if (reset) null else cur.cursor,
            )
            _ui.update { s ->
                val merged: List<Article>
                var selected = s.selectedId
                if (reset) {
                    merged = page.items
                    // 刷新后保留当前选中的文章；若已不在列表里则选第一篇
                    if (page.items.none { it.id == s.selectedId }) {
                        selected = page.items.firstOrNull()?.id
                    }
                } else {
                    val known = s.articles.mapTo(HashSet()) { it.id }
                    merged = s.articles + page.items.filter { it.id !in known }
                    if (selected == null) selected = merged.firstOrNull()?.id
                }
                s.copy(
                    articles = merged,
                    selectedId = selected,
                    total = page.total,
                    // 服务端权威游标：非空表示还有下一页
                    cursor = page.nextCursor,
                    lastPageFull = page.items.size >= PAGE_SIZE,
                    error = "",
                    connected = true,
                )
            }
        } catch (e: Throwable) {
            handleError(e)
        } finally {
            _ui.update { it.copy(loading = false, loadingMore = false) }
        }
    }

    fun loadArticles(reset: Boolean = true) {
        viewModelScope.launch { doLoadArticles(reset) }
    }

    fun loadMore() = loadArticles(false)

    /** 切换订阅源 / 筛选条件 / 关键词时调用 */
    private suspend fun doReload() {
        _ui.update { it.copy(selectedId = null) }
        doLoadArticles(true)
    }

    fun reload() {
        viewModelScope.launch { doReload() }
    }

    fun selectFeed(id: Long?) {
        _ui.update { it.copy(selectedFeedId = id) }
        reload()
    }

    fun setFilter(filter: ArticleFilter) {
        if (_ui.value.filter == filter) return
        _ui.update { it.copy(filter = filter) }
        reload()
    }

    /** 只记录关键词，不立即查询（与桌面端一致，需要显式提交） */
    fun setKeyword(keyword: String) = _ui.update { it.copy(keyword = keyword) }

    fun submitKeyword(keyword: String) {
        _ui.update { it.copy(keyword = keyword) }
        reload()
    }

    /** 首次进入 / 设置变更后的统一入口 */
    fun bootstrap() {
        viewModelScope.launch {
            doLoadFeeds()
            doLoadStats()
            doLoadMediaHosts()
            doLoadArticles(true)
        }
    }

    private suspend fun doLoadMediaHosts() {
        val policy = runCatching { ApiClient.mediaPolicy() }.getOrDefault(MediaHosts())
        val base = if (policy.hosts.isEmpty()) {
            ""
        } else {
            withContext(Dispatchers.IO) {
                runCatching { MediaProxy.setPolicy(policy.hosts, policy.referer) }.getOrDefault("")
            }
        }
        _ui.update { it.copy(mediaHosts = policy.hosts, localMediaBase = base) }
    }

    // ------------------------------------------------------------------ 选择文章

    /**
     * 选中 0.6 秒后自动标记已读（和 Inoreader / 桌面端一致）。
     * 列表不会把它移除，方便回看；可以手动标回未读。
     */
    private fun scheduleAutoRead(id: Long) {
        autoReadJob?.cancel()
        autoReadJob = viewModelScope.launch {
            delay(AUTO_READ_DELAY)
            val article = _ui.value.articles.firstOrNull { it.id == id } ?: return@launch
            if (article.isRead) return@launch

            // 乐观更新：本地置已读 + 该源未读数 -1 + 全局未读 -1
            _ui.update { s ->
                s.copy(
                    articles = s.articles.map { if (it.id == id) it.copy(isRead = true) else it },
                    feeds = s.feeds.map {
                        if (it.id == article.feedId && it.unreadCount > 0) {
                            it.copy(unreadCount = it.unreadCount - 1)
                        } else {
                            it
                        }
                    },
                    stats = s.stats.copy(unread = (s.stats.unread - 1).coerceAtLeast(0)),
                )
            }

            try {
                ApiClient.setRead(id, true)
                doLoadStats()
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    fun selectArticle(id: Long?, autoRead: Boolean = true) {
        _ui.update { it.copy(selectedId = id) }
        if (id != null && autoRead) scheduleAutoRead(id)
    }

    /** 点开一篇文章：进入阅读页并启动 0.6s 自动已读 */
    fun openArticle(id: Long) {
        _ui.update { it.copy(readerOpen = true) }
        selectArticle(id)
    }

    fun closeReader() = _ui.update { it.copy(readerOpen = false) }

    /**
     * 左右滑动 / 上下篇按钮的核心，移植桌面端 `move(delta)`：
     * 到末尾时自动加载下一页；首尾夹住，不循环。
     */
    fun move(delta: Int) {
        viewModelScope.launch {
            val list = _ui.value.articles
            if (list.isEmpty()) return@launch

            var idx = _ui.value.selectedIndex
            idx = if (idx < 0) 0 else idx + delta

            if (idx < 0) idx = 0
            if (idx >= list.size) {
                if (_ui.value.hasMore) {
                    doLoadArticles(false)
                    val newIdx = _ui.value.selectedIndex + delta
                    if (newIdx >= _ui.value.articles.size) return@launch
                    selectArticle(_ui.value.articles[newIdx].id)
                    return@launch
                }
                idx = list.size - 1
            }

            selectArticle(list[idx].id)
        }
    }

    /** 滑到接近末尾时提前预取下一页，让滑动更顺 */
    fun prefetchIfNearEnd() {
        val s = _ui.value
        if (s.hasMore && !s.loadingMore && s.selectedIndex >= s.articles.size - 3) {
            loadArticles(false)
        }
    }

    // ------------------------------------------------------------- 文章状态操作

    fun toggleRead(article: Article? = null) {
        val target = article ?: _ui.value.selectedArticle ?: return
        val next = !target.isRead
        _ui.update { s ->
            s.copy(
                articles = s.articles.map { if (it.id == target.id) it.copy(isRead = next) else it },
                feeds = s.feeds.map {
                    if (it.id == target.feedId) {
                        it.copy(unreadCount = (it.unreadCount + if (next) -1 else 1).coerceAtLeast(0))
                    } else {
                        it
                    }
                },
                stats = s.stats.copy(
                    unread = (s.stats.unread + if (next) -1 else 1).coerceAtLeast(0),
                ),
            )
        }
        viewModelScope.launch {
            try {
                ApiClient.setRead(target.id, next)
                doLoadStats()
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    fun toggleStar(article: Article? = null) {
        val target = article ?: _ui.value.selectedArticle ?: return
        val next = !target.isStarred
        _ui.update { s ->
            s.copy(
                articles = s.articles.map { if (it.id == target.id) it.copy(isStarred = next) else it },
                stats = s.stats.copy(
                    starred = (s.stats.starred + if (next) 1 else -1).coerceAtLeast(0),
                ),
            )
        }
        viewModelScope.launch {
            try {
                ApiClient.setStarred(target.id, next)
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    /** 当前订阅源范围内全部标为已读（不受过滤器与搜索词影响，与服务端语义一致） */
    fun markAllRead() {
        viewModelScope.launch {
            try {
                val res = ApiClient.markRead(feedId = _ui.value.selectedFeedId)
                doLoadFeeds()
                doLoadStats()
                doLoadArticles(true)
                showToast("已标记 ${res.updated} 篇为已读")
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    // ----------------------------------------------------------------- 刷新

    /**
     * 只从服务端重载订阅源 / 统计 / 当前文章列表，不触发 RSS 抓取。
     * 对应桌面端 `stores/app.ts` 的 `refreshList()`，是列表顶栏刷新按钮的行为。
     */
    fun refreshList() {
        if (_ui.value.reloading) return
        _ui.update { it.copy(reloading = true) }
        viewModelScope.launch {
            try {
                // 与桌面端 Promise.all 一致：订阅源和统计并发，再重载列表
                coroutineScope {
                    launch { doLoadFeeds() }
                    launch { doLoadStats() }
                }
                doLoadArticles(true)
                requestScrollTop()
            } finally {
                _ui.update { it.copy(reloading = false) }
            }
        }
    }

    /** 刷新类操作重载列表后调用：列表回到顶部 */
    private fun requestScrollTop() = _ui.update { it.copy(scrollTopToken = it.scrollTopToken + 1) }

    fun refreshAll() {
        if (_ui.value.refreshing) return
        _ui.update { it.copy(refreshing = true, error = "") }
        viewModelScope.launch {
            try {
                ApiClient.refreshAll()
                // 轮询刷新进度，最多 300 次 × 500ms（与桌面端一致）
                for (i in 0 until 300) {
                    val status = ApiClient.refreshStatus()
                    _ui.update { s -> s.copy(refreshInfo = status) }
                    if (!status.running) break
                    delay(500)
                }
                doLoadFeeds()
                doLoadStats()
                doLoadArticles(true)
                requestScrollTop()
            } catch (e: Throwable) {
                handleError(e)
            } finally {
                _ui.update { it.copy(refreshing = false) }
            }
        }
    }

    fun refreshOne(feedId: Long) {
        viewModelScope.launch {
            try {
                val res = ApiClient.refreshFeed(feedId)
                doLoadFeeds()
                doLoadStats()
                doLoadArticles(true)
                requestScrollTop()
                showToast("刷新完成，新增 ${res.newArticles} 篇")
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    // ------------------------------------------------------------ 订阅源管理

    fun addFeed(url: String, title: String?, category: String?, onDone: (String?) -> Unit) {
        viewModelScope.launch {
            try {
                ApiClient.addFeed(url, title?.takeIf { it.isNotBlank() }, category)
                doLoadFeeds()
                doLoadStats()
                doLoadArticles(true)
                onDone(null)
            } catch (e: Throwable) {
                onDone(e.message ?: "添加失败")
            }
        }
    }

    fun updateFeed(id: Long, patch: UpdateFeedBody, onDone: (String?) -> Unit) {
        viewModelScope.launch {
            try {
                ApiClient.updateFeed(id, patch)
                doLoadFeeds()
                onDone(null)
            } catch (e: Throwable) {
                onDone(e.message ?: "保存失败")
            }
        }
    }

    fun removeFeed(id: Long, onDone: (String?) -> Unit) {
        viewModelScope.launch {
            try {
                ApiClient.deleteFeed(id)
                if (_ui.value.selectedFeedId == id) {
                    _ui.update { it.copy(selectedFeedId = null) }
                }
                doLoadFeeds()
                doLoadStats()
                doLoadArticles(true)
                onDone(null)
            } catch (e: Throwable) {
                onDone(e.message ?: "删除失败")
            }
        }
    }

    fun importOpml(xml: String) {
        viewModelScope.launch {
            try {
                val res = ApiClient.importOpml(xml)
                doLoadFeeds()
                doLoadStats()
                showToast("导入完成：新增 ${res.added} 个，跳过 ${res.skipped} 个")
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    /** 拉取服务端生成的 OPML 原文，交给调用方写入用户选择的文件 */
    fun exportOpml(onReady: (String) -> Unit) {
        viewModelScope.launch {
            try {
                onReady(ApiClient.exportOpml())
            } catch (e: Throwable) {
                handleError(e)
            }
        }
    }

    // ------------------------------------------------------------ 订阅推荐

    /** 打开推荐对话框：清空本轮记录，取第一批 */
    fun openRecommend() {
        recommendShown = mutableSetOf()
        _ui.update {
            it.copy(recommendPending = null, recommendAdded = emptySet(), recommendErrors = emptyMap())
        }
        viewModelScope.launch {
            ensureCatalog()
            loadRecommend()
        }
    }

    fun closeRecommend() = _ui.update {
        it.copy(recommendItems = emptyList(), recommendPending = null, recommendErrors = emptyMap())
    }

    /** 换一批：当前这批记为已看过，从剩下的里面继续取 */
    fun shuffleRecommend() {
        val s = _ui.value
        if (s.recommendPending != null || s.recommendPages <= 1 || s.recommendItems.isEmpty()) return
        recommendShown.addAll(s.recommendItems.map { it.id })
        loadRecommend()
        // 看完了就从头再来一轮
        if (_ui.value.recommendItems.isEmpty()) {
            recommendShown = mutableSetOf()
            loadRecommend()
        }
    }

    fun addRecommended(item: CatalogFeed) {
        val s = _ui.value
        if (s.recommendPending != null || s.recommendAdded.contains(item.id)) return
        _ui.update { it.copy(recommendPending = item.id, recommendErrors = it.recommendErrors - item.id) }
        addFeed(item.url, item.title, catalogCategoryLabel(item.category)) { err ->
            _ui.update { cur ->
                if (err == null) {
                    cur.copy(
                        recommendPending = null,
                        recommendAdded = cur.recommendAdded + item.id,
                        toast = "已添加「${item.title}」",
                    )
                } else {
                    cur.copy(
                        recommendPending = null,
                        recommendErrors = cur.recommendErrors + (item.id to err),
                    )
                }
            }
        }
    }

    private fun hints(): List<SubscribedFeedHint> =
        _ui.value.feeds.map { SubscribedFeedHint(url = it.url, title = it.title, category = it.category) }

    private suspend fun ensureCatalog() {
        if (catalog.isNotEmpty()) return
        val app = getApplication<Application>()
        val (feeds, source) = withContext(Dispatchers.IO) {
            runCatching { FeedCatalog.load(app) to FeedCatalog.source(app) }
                .getOrDefault(emptyList<CatalogFeed>() to "")
        }
        catalog = feeds
        catalogSource = source
    }

    private fun loadRecommend() {
        if (catalog.isEmpty()) {
            _ui.update {
                it.copy(recommendItems = emptyList(), recommendPage = 0, recommendPages = 0)
            }
            return
        }
        val result = recommendFeedPage(catalog, hints(), RECOMMEND_BATCH_SIZE, recommendShown)
        val total = recommendShown.size + result.total
        _ui.update { s ->
            s.copy(
                recommendItems = result.items,
                recommendPages = if (total == 0) 0 else (total + RECOMMEND_BATCH_SIZE - 1) / RECOMMEND_BATCH_SIZE,
                recommendPage = if (total == 0) 0 else recommendShown.size / RECOMMEND_BATCH_SIZE,
                recommendErrors = emptyMap(),
                recommendSource = catalogSource,
                recommendCatalogSize = catalog.size,
            )
        }
    }

    // ---------------------------------------------------------------- 设置

    fun saveSettings(serverUrl: String, token: String) {
        SettingsStore.save(serverUrl, token)
        val saved = SettingsStore.load()
        ApiClient.configure(saved.serverUrl, saved.token)
        _ui.update {
            it.copy(
                serverUrl = saved.serverUrl,
                token = saved.token,
                needsSetup = saved.needsSetup,
                error = "",
            )
        }
        if (!saved.needsSetup) bootstrap()
    }

    /** 测试连接：先 /health，再 /api/feeds 验证 token */
    fun testConnection(serverUrl: String, token: String, onResult: (Boolean, String) -> Unit) {
        viewModelScope.launch {
            ApiClient.configure(serverUrl, token)
            try {
                if (!ApiClient.health()) {
                    onResult(false, "连不上服务端，检查地址和端口是否正确")
                    return@launch
                }
                val feeds = ApiClient.listFeeds()
                onResult(true, "连接成功，共 ${feeds.size} 个订阅源")
            } catch (e: Throwable) {
                val msg = if (e is ApiError && e.isUnauthorized) "token 无效" else (e.message ?: "连接失败")
                onResult(false, msg)
            } finally {
                // 恢复成已保存的配置，避免测试用的临时值影响后续请求
                val saved = SettingsStore.load()
                ApiClient.configure(saved.serverUrl, saved.token)
            }
        }
    }
}
