package com.simonlei.tinyreader.ui

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Download
import androidx.compose.material.icons.filled.ExpandLess
import androidx.compose.material.icons.filled.ExpandMore
import androidx.compose.material.icons.filled.Inbox
import androidx.compose.material.icons.filled.Settings
import androidx.compose.material.icons.filled.UploadFile
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.data.Feed
import com.simonlei.tinyreader.ui.theme.Accent
import com.simonlei.tinyreader.ui.theme.AccentSoft
import com.simonlei.tinyreader.ui.theme.BorderColor
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.TextDim
import com.simonlei.tinyreader.ui.theme.TextMain
import com.simonlei.tinyreader.ui.theme.TextMute
import java.text.Collator
import java.util.Locale

/**
 * 订阅源抽屉，对应桌面端 `src/components/Sidebar.vue`：
 * 「全部文章」+ 按 category 分组的订阅源树（可折叠、显示未读数、失败标红）。
 */
@OptIn(ExperimentalFoundationApi::class)
@Composable
fun FeedDrawerContent(
    ui: UiState,
    onSelectFeed: (Long?) -> Unit,
    onEditFeed: (Feed) -> Unit,
    onAddFeed: () -> Unit,
    onImportOpml: () -> Unit,
    onExportOpml: () -> Unit,
    onOpenSettings: () -> Unit,
) {
    // 折叠状态只保存在内存里，与桌面端一致
    val collapsed = remember { mutableStateMapOf<String, Boolean>() }

    val groups = remember(ui.feeds) {
        val collator = Collator.getInstance(Locale.SIMPLIFIED_CHINESE)
        ui.feeds
            .groupBy { it.category.ifBlank { "未分类" } }
            .entries
            .sortedWith { a, b -> collator.compare(a.key, b.key) }
            .map { it.key to it.value }
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .fillMaxHeight(),
    ) {

        // ---------------------------------------------------------- 品牌区
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 8.dp, top = 18.dp, bottom = 12.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Box(
                modifier = Modifier
                    .size(30.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(Accent),
                contentAlignment = Alignment.Center,
            ) {
                Text("TR", fontSize = 12.sp, fontWeight = FontWeight.Bold, color = androidx.compose.ui.graphics.Color.White)
            }
            Spacer(Modifier.width(10.dp))
            Text(
                "Tiny Reader",
                fontSize = 16.sp,
                fontWeight = FontWeight.SemiBold,
                color = TextMain,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = onOpenSettings) {
                Icon(Icons.Filled.Settings, contentDescription = "设置", tint = TextDim)
            }
        }

        HorizontalDivider(color = BorderColor)

        LazyColumn(modifier = Modifier.weight(1f)) {

            // ------------------------------------------------ 全部文章
            item {
                DrawerRow(
                    title = "全部文章",
                    badge = ui.stats.unread,
                    active = ui.selectedFeedId == null,
                    leading = {
                        Icon(
                            Icons.Filled.Inbox,
                            contentDescription = null,
                            tint = if (ui.selectedFeedId == null) Accent else TextMute,
                            modifier = Modifier.size(18.dp),
                        )
                    },
                    onClick = { onSelectFeed(null) },
                )
            }

            item {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(start = 16.dp, end = 8.dp, top = 14.dp, bottom = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        "订阅源 (${ui.feeds.size})",
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold,
                        color = TextMute,
                        modifier = Modifier.weight(1f),
                    )
                    IconButton(onClick = onAddFeed, modifier = Modifier.size(32.dp)) {
                        Icon(
                            Icons.Filled.Add,
                            contentDescription = "添加订阅源",
                            tint = TextDim,
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }

            if (ui.feeds.isEmpty()) {
                item {
                    Text(
                        "还没有订阅源，点上面的 ＋ 或导入 OPML",
                        fontSize = 12.sp,
                        color = TextMute,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
                    )
                }
            }

            groups.forEach { (name, feeds) ->
                val isCollapsed = collapsed[name] == true

                item(key = "group-$name") {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable { collapsed[name] = !isCollapsed }
                            .padding(start = 12.dp, end = 14.dp, top = 8.dp, bottom = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            if (isCollapsed) Icons.Filled.ExpandMore else Icons.Filled.ExpandLess,
                            contentDescription = null,
                            tint = TextMute,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(2.dp))
                        Text(
                            name,
                            fontSize = 12.sp,
                            fontWeight = FontWeight.Medium,
                            color = TextDim,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Text("${feeds.size}", fontSize = 11.sp, color = TextMute)
                    }
                }

                if (!isCollapsed) {
                    items(feeds, key = { "feed-${it.id}" }) { feed ->
                        DrawerRow(
                            title = feed.title.ifBlank { feed.url },
                            badge = feed.unreadCount,
                            active = ui.selectedFeedId == feed.id,
                            failed = !feed.lastError.isNullOrBlank(),
                            indent = 16.dp,
                            onClick = { onSelectFeed(feed.id) },
                            onLongClick = { onEditFeed(feed) },
                        )
                    }
                }
            }

            item { Spacer(Modifier.height(8.dp)) }
        }

        HorizontalDivider(color = BorderColor)

        // ---------------------------------------------------------- 底部
        Column(modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp)) {
            Row(modifier = Modifier.fillMaxWidth()) {
                TextButton(onClick = onImportOpml, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Filled.UploadFile, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("导入", fontSize = 13.sp)
                }
                TextButton(onClick = onExportOpml, modifier = Modifier.weight(1f)) {
                    Icon(Icons.Filled.Download, contentDescription = null, modifier = Modifier.size(16.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("导出", fontSize = 13.sp)
                }
            }
            Text(
                "共 ${ui.stats.total} 篇 · 未读 ${ui.stats.unread} · 星标 ${ui.stats.starred}",
                fontSize = 11.sp,
                color = TextMute,
                modifier = Modifier.padding(start = 8.dp, top = 2.dp, bottom = 6.dp),
            )
            Text(
                "长按订阅源可编辑 / 删除",
                fontSize = 11.sp,
                color = TextMute,
                modifier = Modifier.padding(start = 8.dp, bottom = 8.dp),
            )
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun DrawerRow(
    title: String,
    badge: Int,
    active: Boolean,
    failed: Boolean = false,
    indent: androidx.compose.ui.unit.Dp = 0.dp,
    leading: @Composable (() -> Unit)? = null,
    onClick: () -> Unit,
    onLongClick: (() -> Unit)? = null,
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 1.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (active) AccentSoft else androidx.compose.ui.graphics.Color.Transparent)
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .padding(start = 8.dp + indent, end = 10.dp, top = 9.dp, bottom = 9.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        leading?.invoke()
        Text(
            title,
            fontSize = 14.sp,
            color = when {
                failed -> Danger
                active -> TextMain
                else -> TextDim
            },
            fontWeight = if (active) FontWeight.Medium else FontWeight.Normal,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f),
        )
        if (badge > 0) {
            Text(
                if (badge > 999) "999+" else "$badge",
                fontSize = 11.sp,
                color = if (active) Accent else TextMute,
                fontWeight = FontWeight.Medium,
                style = MaterialTheme.typography.labelSmall,
            )
        }
    }
}
