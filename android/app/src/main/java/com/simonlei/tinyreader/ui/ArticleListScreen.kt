package com.simonlei.tinyreader.ui

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.DoneAll
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Search
import androidx.compose.material.icons.filled.Star
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.data.Article
import com.simonlei.tinyreader.data.ArticleFilter
import com.simonlei.tinyreader.ui.theme.Accent
import com.simonlei.tinyreader.ui.theme.AccentSoft
import com.simonlei.tinyreader.ui.theme.BgAlt
import com.simonlei.tinyreader.ui.theme.BorderColor
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.TextDim
import com.simonlei.tinyreader.ui.theme.TextMain
import com.simonlei.tinyreader.ui.theme.TextMute
import com.simonlei.tinyreader.ui.theme.Warn
import com.simonlei.tinyreader.util.formatTime
import com.simonlei.tinyreader.util.stripHtml
import com.simonlei.tinyreader.util.truncate

/**
 * 文章列表页，对应桌面端 `ArticleList.vue` + `App.vue` 的顶栏。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ArticleListScreen(
    ui: UiState,
    onOpenDrawer: () -> Unit,
    onOpenArticle: (Long) -> Unit,
    onSetFilter: (ArticleFilter) -> Unit,
    onSubmitKeyword: (String) -> Unit,
    onToggleStar: (Article) -> Unit,
    onMarkAllRead: () -> Unit,
    onRefreshAll: () -> Unit,
    onOpenSettings: () -> Unit,
    onLoadMore: () -> Unit,
) {
    val listState = rememberLazyListState()
    var searchVisible by remember { mutableStateOf(false) }
    var searchText by remember { mutableStateOf(ui.keyword) }

    // 首次进入（含从阅读页返回）时把当前文章滚进可视区，之后切换订阅源 / 过滤器则回到顶部
    var restored by remember { mutableStateOf(false) }
    LaunchedEffect(ui.selectedFeedId, ui.filter) {
        if (!restored) {
            restored = true
            val idx = ui.selectedIndex
            if (idx > 0) listState.scrollToItem(idx)
        } else {
            listState.scrollToItem(0)
        }
    }

    // 无限滚动：接近底部时加载下一页（对应桌面端距底 240px 触发）
    LaunchedEffect(listState) {
        snapshotFlow { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: -1 }
            .collect { lastVisible ->
                if (lastVisible >= 0 && lastVisible >= listState.layoutInfo.totalItemsCount - 5) {
                    onLoadMore()
                }
            }
    }

    Scaffold(
        containerColor = com.simonlei.tinyreader.ui.theme.Bg,
        topBar = {
            Column {
                TopAppBar(
                    colors = TopAppBarDefaults.topAppBarColors(
                        containerColor = BgAlt,
                        titleContentColor = TextMain,
                    ),
                    navigationIcon = {
                        IconButton(onClick = onOpenDrawer) {
                            Icon(Icons.Filled.Menu, contentDescription = "订阅源", tint = TextDim)
                        }
                    },
                    title = {
                        Column {
                            Text(
                                ui.currentFeedTitle,
                                fontSize = 16.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                            Text(
                                text = statusLine(ui),
                                fontSize = 11.sp,
                                color = if (ui.error.isNotEmpty()) Danger else TextMute,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis,
                            )
                        }
                    },
                    actions = {
                        IconButton(onClick = { searchVisible = !searchVisible }) {
                            Icon(
                                if (searchVisible) Icons.Filled.Close else Icons.Filled.Search,
                                contentDescription = "搜索",
                                tint = TextDim,
                            )
                        }
                        IconButton(onClick = onMarkAllRead) {
                            Icon(Icons.Filled.DoneAll, contentDescription = "全部标为已读", tint = TextDim)
                        }
                        IconButton(onClick = onRefreshAll, enabled = !ui.refreshing) {
                            if (ui.refreshing) {
                                CircularProgressIndicator(
                                    modifier = Modifier.size(18.dp),
                                    strokeWidth = 2.dp,
                                    color = Accent,
                                )
                            } else {
                                Icon(Icons.Filled.Refresh, contentDescription = "刷新", tint = TextDim)
                            }
                        }
                    },
                )

                if (ui.refreshing && ui.refreshInfo.total > 0) {
                    LinearProgressIndicator(
                        progress = {
                            (ui.refreshInfo.done.toFloat() / ui.refreshInfo.total.toFloat())
                                .coerceIn(0f, 1f)
                        },
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(3.dp),
                        color = Accent,
                        trackColor = BorderColor,
                    )
                }

                AnimatedVisibility(visible = searchVisible) {
                    OutlinedTextField(
                        value = searchText,
                        onValueChange = { searchText = it },
                        placeholder = { Text("搜索标题 / 摘要…", fontSize = 14.sp) },
                        singleLine = true,
                        trailingIcon = {
                            if (searchText.isNotEmpty()) {
                                IconButton(onClick = {
                                    searchText = ""
                                    onSubmitKeyword("")
                                }) {
                                    Icon(Icons.Filled.Close, contentDescription = "清空", tint = TextMute)
                                }
                            }
                        },
                        keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                        keyboardActions = KeyboardActions(onSearch = { onSubmitKeyword(searchText) }),
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }

                // 过滤器：全部 / 未读 / 星标
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(BgAlt)
                        .padding(horizontal = 12.dp, vertical = 6.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    ArticleFilter.entries.forEach { filter ->
                        FilterChip(
                            selected = ui.filter == filter,
                            onClick = { onSetFilter(filter) },
                            label = { Text(filter.label, fontSize = 13.sp) },
                            colors = FilterChipDefaults.filterChipColors(
                                selectedContainerColor = AccentSoft,
                                selectedLabelColor = Accent,
                                labelColor = TextDim,
                            ),
                        )
                    }
                    Spacer(Modifier.weight(1f))
                    Text(
                        "${ui.articles.size} / ${ui.total}",
                        fontSize = 11.sp,
                        color = TextMute,
                    )
                }

                HorizontalDivider(color = BorderColor)
            }
        },
    ) { padding ->
        PullToRefreshBox(
            isRefreshing = ui.refreshing,
            onRefresh = onRefreshAll,
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            when {
                ui.needsSetup -> EmptyHint("请先在设置里填写服务端地址和 token", action = "去设置", onAction = onOpenSettings)

                ui.loading && ui.articles.isEmpty() -> EmptyHint("加载中…")

                ui.error.isNotEmpty() && ui.articles.isEmpty() ->
                    EmptyHint("⚠ ${ui.error}", action = "去设置", onAction = onOpenSettings)

                ui.articles.isEmpty() -> EmptyHint(
                    if (ui.filter == ArticleFilter.UNREAD) "没有未读文章 🎉" else "这里还没有文章"
                )

                else -> LazyColumn(state = listState, modifier = Modifier.fillMaxSize()) {
                    items(ui.articles, key = { it.id }) { article ->
                        ArticleRow(
                            article = article,
                            active = article.id == ui.selectedId,
                            showFeedName = ui.selectedFeedId == null,
                            onClick = { onOpenArticle(article.id) },
                            onStarClick = { onToggleStar(article) },
                        )
                        HorizontalDivider(color = BorderColor.copy(alpha = 0.5f))
                    }

                    item {
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(vertical = 16.dp),
                            contentAlignment = Alignment.Center,
                        ) {
                            Text(
                                when {
                                    ui.loadingMore -> "加载更多…"
                                    ui.hasMore -> "上滑加载更多"
                                    else -> "— 到底了 —"
                                },
                                fontSize = 12.sp,
                                color = TextMute,
                            )
                        }
                    }
                }
            }
        }
    }
}

/** 顶栏第二行：刷新进度 / 错误 / 统计，对应桌面端 topbar 的信息区 */
private fun statusLine(ui: UiState): String = when {
    ui.refreshing && ui.refreshInfo.total > 0 -> {
        val base = "刷新中 ${ui.refreshInfo.done} / ${ui.refreshInfo.total} · 新增 ${ui.refreshInfo.newArticles} 篇"
        if (ui.failedFeedCount > 0) "$base · ${ui.failedFeedCount} 个源失败" else base
    }

    ui.refreshing -> "刷新中…"
    ui.error.isNotEmpty() -> "⚠ ${ui.error}"
    else -> {
        val base = "${ui.feeds.size} 个订阅源 · 未读 ${ui.stats.unread}"
        if (ui.failedFeedCount > 0) "$base · ${ui.failedFeedCount} 个源失败" else base
    }
}

/**
 * 单条文章，对应桌面端 `.art` 行：
 * 未读圆点 + 标题 + 星标 / 来源 + 时间 / 两行摘要。
 */
@Composable
private fun ArticleRow(
    article: Article,
    active: Boolean,
    showFeedName: Boolean,
    onClick: () -> Unit,
    onStarClick: () -> Unit,
) {
    val summary = remember(article.content, article.summary) {
        truncate(stripHtml(article.content ?: article.summary), 160)
    }
    val time = remember(article.publishedAt) { formatTime(article.publishedAt) }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(if (active) AccentSoft else Color.Transparent)
            .clickable(onClick = onClick)
            .padding(start = 10.dp, end = 12.dp, top = 11.dp, bottom = 11.dp),
    ) {
        // 左侧 3px 蓝条（选中态）
        Box(
            modifier = Modifier
                .width(3.dp)
                .height(38.dp)
                .clip(RoundedCornerShape(2.dp))
                .background(if (active) Accent else Color.Transparent),
        )
        Spacer(Modifier.width(7.dp))

        // 未读圆点
        Box(
            modifier = Modifier
                .padding(top = 6.dp)
                .size(7.dp)
                .clip(CircleShape)
                .background(Accent.copy(alpha = if (article.isRead) 0.22f else 1f)),
        )
        Spacer(Modifier.width(10.dp))

        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.Top) {
                Text(
                    article.title.ifBlank { "(无标题)" },
                    fontSize = 15.sp,
                    lineHeight = 21.sp,
                    color = if (article.isRead) TextDim else TextMain,
                    fontWeight = if (article.isRead) FontWeight.Normal else FontWeight.Medium,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                if (article.isStarred) {
                    Spacer(Modifier.width(6.dp))
                    Icon(
                        Icons.Filled.Star,
                        contentDescription = "已收藏",
                        tint = Warn,
                        modifier = Modifier
                            .size(16.dp)
                            .clickable(onClick = onStarClick),
                    )
                }
            }

            Spacer(Modifier.height(3.dp))

            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    if (showFeedName) article.feedTitle.orEmpty() else "",
                    fontSize = 11.sp,
                    color = TextMute,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                // 没有真实发表时间就不显示，避免把拉取时间伪装成发表时间
                if (time.isNotEmpty()) {
                    Spacer(Modifier.width(8.dp))
                    Text(time, fontSize = 11.sp, color = TextMute)
                }
            }

            if (summary.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    summary,
                    fontSize = 12.sp,
                    lineHeight = 17.sp,
                    color = TextMute,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
    }
}

@Composable
private fun EmptyHint(text: String, action: String? = null, onAction: (() -> Unit)? = null) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(text, fontSize = 14.sp, color = TextMute)
            if (action != null && onAction != null) {
                Spacer(Modifier.height(8.dp))
                TextButton(onClick = onAction) { Text(action) }
            }
        }
    }
}
