package com.simonlei.tinyreader.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.ChevronLeft
import androidx.compose.material.icons.filled.ChevronRight
import androidx.compose.material.icons.filled.MarkEmailRead
import androidx.compose.material.icons.filled.MarkEmailUnread
import androidx.compose.material.icons.filled.OpenInBrowser
import androidx.compose.material.icons.filled.Star
import androidx.compose.material.icons.filled.StarBorder
import androidx.compose.material3.BottomAppBar
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.key
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import com.simonlei.tinyreader.data.Article
import com.simonlei.tinyreader.ui.theme.Accent
import com.simonlei.tinyreader.ui.theme.Bg
import com.simonlei.tinyreader.ui.theme.BgAlt
import com.simonlei.tinyreader.ui.theme.TextDim
import com.simonlei.tinyreader.ui.theme.TextMain
import com.simonlei.tinyreader.ui.theme.TextMute
import com.simonlei.tinyreader.ui.theme.Warn
import com.simonlei.tinyreader.util.buildArticleHtml
import com.simonlei.tinyreader.util.openExternal
import kotlin.math.abs

/**
 * 阅读页。
 *
 * 交互对应桌面端的 `j` / `k`：
 * - 向左滑 = 下一篇（move(+1)），向右滑 = 上一篇（move(-1)）
 * - 首尾夹住不循环；滑到末尾且还有更多时自动加载下一页
 * - 每次落位都会重新启动 0.6s 自动已读定时器
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ReaderScreen(
    ui: UiState,
    onClose: () -> Unit,
    onMove: (Int) -> Unit,
    onPrefetch: () -> Unit,
    onToggleStar: () -> Unit,
    onToggleRead: () -> Unit,
) {
    val context = LocalContext.current
    val article = ui.selectedArticle
    val index = ui.selectedIndex

    BackHandler(enabled = true, onBack = onClose)

    // 滑到接近末尾时提前拉下一页，让连续滑动不卡顿
    LaunchedEffect(index, ui.total) { onPrefetch() }

    Scaffold(
        containerColor = Bg,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = BgAlt,
                    titleContentColor = TextMain,
                ),
                navigationIcon = {
                    IconButton(onClick = onClose) {
                        Icon(
                            Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "返回列表",
                            tint = TextDim,
                        )
                    }
                },
                title = {
                    Column {
                        Text(
                            article?.feedTitle?.takeIf { it.isNotBlank() } ?: ui.currentFeedTitle,
                            fontSize = 15.sp,
                            fontWeight = FontWeight.Medium,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                        )
                        Text(
                            if (index >= 0) "${index + 1} / ${ui.total}" else "",
                            fontSize = 11.sp,
                            color = TextMute,
                        )
                    }
                },
                actions = {
                    IconButton(onClick = onToggleStar, enabled = article != null) {
                        Icon(
                            if (article?.isStarred == true) Icons.Filled.Star else Icons.Filled.StarBorder,
                            contentDescription = if (article?.isStarred == true) "取消收藏" else "收藏",
                            tint = if (article?.isStarred == true) Warn else TextDim,
                        )
                    }
                    IconButton(onClick = onToggleRead, enabled = article != null) {
                        Icon(
                            if (article?.isRead == true) Icons.Filled.MarkEmailUnread else Icons.Filled.MarkEmailRead,
                            contentDescription = if (article?.isRead == true) "标为未读" else "标为已读",
                            tint = if (article?.isRead == true) TextDim else Accent,
                        )
                    }
                    IconButton(
                        onClick = { openExternal(context, article?.url) },
                        enabled = !article?.url.isNullOrBlank(),
                    ) {
                        Icon(
                            Icons.Filled.OpenInBrowser,
                            contentDescription = "在浏览器中打开",
                            tint = TextDim,
                        )
                    }
                },
            )
        },
        bottomBar = {
            BottomAppBar(
                containerColor = BgAlt,
                contentPadding = androidx.compose.foundation.layout.PaddingValues(horizontal = 8.dp),
            ) {
                IconButton(onClick = { onMove(-1) }, enabled = index > 0) {
                    Icon(Icons.Filled.ChevronLeft, contentDescription = "上一篇", tint = TextDim)
                }
                Spacer(Modifier.weight(1f))
                Text("← 左右滑动切换文章 →", fontSize = 12.sp, color = TextMute)
                Spacer(Modifier.weight(1f))
                IconButton(
                    onClick = { onMove(1) },
                    enabled = index >= 0 && (index < ui.articles.size - 1 || ui.hasMore),
                ) {
                    Icon(Icons.Filled.ChevronRight, contentDescription = "下一篇", tint = TextDim)
                }
            }
        },
    ) { padding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding),
        ) {
            if (article == null) {
                Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("没有可阅读的文章", fontSize = 14.sp, color = TextMute)
                }
            } else {
                AnimatedContent(
                    targetState = index,
                    transitionSpec = {
                        val forward = targetState > initialState
                        val spec = tween<IntOffset>(220)
                        val fade = tween<Float>(220)
                        (
                            slideInHorizontally(spec) { w -> if (forward) w else -w } +
                                fadeIn(fade)
                            ) togetherWith (
                            slideOutHorizontally(spec) { w -> if (forward) -w else w } +
                                fadeOut(fade)
                            )
                    },
                    label = "article-switch",
                ) { targetIndex ->
                    val target = ui.articles.getOrNull(targetIndex)
                    if (target == null) {
                        Box(modifier = Modifier.fillMaxSize())
                    } else {
                        ArticleBody(
                            article = target,
                            onSwipeNext = { onMove(1) },
                            onSwipePrev = { onMove(-1) },
                        )
                    }
                }
            }
        }
    }
}

/**
 * 正文区：整页就是一个 WebView，横向手势由 [ArticleWebView] 自己识别后回调，
 * 竖向滚动仍然交给 WebView，避免嵌套滚动冲突。
 */
@Composable
private fun ArticleBody(
    article: Article,
    onSwipeNext: () -> Unit,
    onSwipePrev: () -> Unit,
) {
    val context = LocalContext.current
    val html = remember(article.id) { buildArticleHtml(article) }
    val baseUrl = remember(article.id) {
        article.feedSiteUrl?.takeIf { it.isNotBlank() } ?: article.url
    }

    // 换文章时重建 WebView，确保加载新内容且滚动回到顶部
    key(article.id) {
        AndroidView(
            modifier = Modifier
                .fillMaxSize()
                // 兜底：正文区域没被 WebView 消费掉的横向拖动也能翻页
                .pointerInput(article.id) {
                    var dragged = 0f
                    detectHorizontalDragGestures(
                        onDragStart = { dragged = 0f },
                        onDragEnd = {
                            if (abs(dragged) > 80f) {
                                if (dragged < 0) onSwipeNext() else onSwipePrev()
                            }
                        },
                    ) { _, amount -> dragged += amount }
                },
            factory = { ctx ->
                ArticleWebView(ctx).apply {
                    onLinkClick = { url -> openExternal(context, url) }
                    loadDataWithBaseURL(baseUrl, html, "text/html", "utf-8", null)
                }
            },
            update = { web ->
                web.onSwipeNext = onSwipeNext
                web.onSwipePrev = onSwipePrev
            },
        )
    }
}
