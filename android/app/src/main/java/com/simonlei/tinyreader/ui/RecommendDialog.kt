package com.simonlei.tinyreader.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.data.CatalogFeed
import com.simonlei.tinyreader.ui.theme.Accent
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.Panel
import com.simonlei.tinyreader.ui.theme.Panel2
import com.simonlei.tinyreader.ui.theme.TextDim
import com.simonlei.tinyreader.ui.theme.TextMain
import com.simonlei.tinyreader.ui.theme.TextMute
import com.simonlei.tinyreader.util.catalogCategoryLabel
import com.simonlei.tinyreader.util.feedDetail
import com.simonlei.tinyreader.util.openExternal
import java.net.URL

/**
 * 推荐订阅源，对应桌面端 `src/components/RecommendDialog.vue`。
 * 桌面端把批次状态放在组件里，这里放在 [ReaderViewModel] 里，界面只负责渲染。
 */
@Composable
fun RecommendDialog(
    ui: UiState,
    onDismiss: () -> Unit,
    onShuffle: () -> Unit,
    onAdd: (CatalogFeed) -> Unit,
) {
    val context = LocalContext.current
    val busy = ui.recommendPending != null

    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        containerColor = Panel,
        title = { Text("推荐订阅源", fontSize = 17.sp) },
        text = {
            Column {
                Text(
                    if (ui.feeds.isEmpty()) {
                        "还没有订阅。下面是精选目录里不同分类的一批，可以直接添加。"
                    } else {
                        "按你已经订阅的源匹配主题，订过的不会再出现。"
                    },
                    fontSize = 12.5.sp,
                    color = TextMute,
                )

                Spacer(Modifier.height(10.dp))

                if (ui.recommendItems.isEmpty()) {
                    Text(
                        // 目录还没读出来之前不要显示「都订阅过了」
                        if (ui.recommendCatalogSize == 0) "正在读取目录…" else "能推荐的源都已经在订阅列表里了。",
                        fontSize = 13.sp,
                        color = TextMute,
                        modifier = Modifier.padding(vertical = 12.dp),
                    )
                } else {
                    LazyColumn(modifier = Modifier.heightIn(max = 320.dp)) {
                        items(ui.recommendItems, key = { it.id }) { item ->
                            RecommendRow(
                                item = item,
                                pending = ui.recommendPending == item.id,
                                added = ui.recommendAdded.contains(item.id),
                                busy = busy,
                                error = ui.recommendErrors[item.id],
                                onAdd = { onAdd(item) },
                            )
                        }
                    }
                }

                if (ui.recommendCatalogSize > 0) {
                    Spacer(Modifier.height(8.dp))
                    Row {
                        Text("目录来自 ", fontSize = 11.5.sp, color = TextMute)
                        Text(
                            "Tidings RSS 精选",
                            fontSize = 11.5.sp,
                            color = Accent,
                            modifier = Modifier.clickable { openExternal(context, ui.recommendSource) },
                        )
                        Text("，共 ${ui.recommendCatalogSize} 个源。", fontSize = 11.5.sp, color = TextMute)
                    }
                }

                if (ui.recommendPages > 1) {
                    Spacer(Modifier.height(4.dp))
                    Text(
                        "第 ${ui.recommendPage + 1} / ${ui.recommendPages} 批",
                        fontSize = 11.5.sp,
                        color = TextMute,
                    )
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = onShuffle,
                enabled = !busy && ui.recommendPages > 1,
            ) {
                Text("换一批")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !busy) { Text("关闭") }
        },
    )
}

@Composable
private fun RecommendRow(
    item: CatalogFeed,
    pending: Boolean,
    added: Boolean,
    busy: Boolean,
    error: String?,
    onAdd: () -> Unit,
) {
    val description = remember(item.id) { feedDetail(item).description }

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                item.title,
                fontSize = 13.5.sp,
                color = TextMain,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(3.dp))
            Row(verticalAlignment = Alignment.CenterVertically) {
                Box(
                    modifier = Modifier
                        .clip(RoundedCornerShape(9.dp))
                        .background(Panel2)
                        .padding(horizontal = 6.dp),
                    contentAlignment = Alignment.Center,
                ) {
                    Text(
                        catalogCategoryLabel(item.category),
                        fontSize = 11.sp,
                        color = TextDim,
                        style = MaterialTheme.typography.labelSmall,
                    )
                }
                Spacer(Modifier.width(6.dp))
                Text(
                    hostLabel(item),
                    fontSize = 11.5.sp,
                    color = TextMute,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
            }
            if (description.isNotEmpty()) {
                Spacer(Modifier.height(4.dp))
                Text(
                    description,
                    fontSize = 12.sp,
                    lineHeight = 16.sp,
                    color = TextMute,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
            if (error != null) {
                Spacer(Modifier.height(4.dp))
                Text(error, fontSize = 12.sp, color = Danger)
            }
        }

        Button(
            onClick = onAdd,
            enabled = !busy && !added,
            contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = Accent,
                contentColor = Color.White,
                disabledContainerColor = Panel2,
                disabledContentColor = TextMute,
            ),
        ) {
            if (pending) {
                CircularProgressIndicator(
                    modifier = Modifier.size(14.dp),
                    strokeWidth = 2.dp,
                    color = Accent,
                )
            } else {
                Text(
                    if (added) "已添加" else "添加",
                    fontSize = 12.5.sp,
                    fontWeight = FontWeight.Medium,
                )
            }
        }
    }
}

private fun hostLabel(item: CatalogFeed): String {
    val raw = item.site.ifBlank { item.url }
    return try {
        URL(raw).host.removePrefix("www.")
    } catch (_: Exception) {
        raw
    }
}
