package com.simonlei.tinyreader.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.data.Feed
import com.simonlei.tinyreader.data.UpdateFeedBody
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.Panel
import com.simonlei.tinyreader.ui.theme.TextMute

/**
 * 新增 / 编辑订阅源，对应桌面端 `FeedDialog.vue`。
 *
 * @param feed 为 null 表示新增
 */
@Composable
fun FeedEditDialog(
    feed: Feed?,
    onDismiss: () -> Unit,
    onAdd: (url: String, title: String?, category: String?, onDone: (String?) -> Unit) -> Unit,
    onUpdate: (id: Long, patch: UpdateFeedBody, onDone: (String?) -> Unit) -> Unit,
    onDelete: (id: Long, onDone: (String?) -> Unit) -> Unit,
    onRefreshOne: (Long) -> Unit,
) {
    // 用局部 val 承接，才能让 Kotlin 对 feed 做智能转换
    val target: Feed? = feed

    var url by remember { mutableStateOf(target?.url.orEmpty()) }
    var title by remember { mutableStateOf(target?.title.orEmpty()) }
    var category by remember { mutableStateOf(target?.category.orEmpty()) }
    var busy by remember { mutableStateOf(false) }
    var error by remember { mutableStateOf("") }
    var confirmDelete by remember { mutableStateOf(false) }

    fun submit() {
        if (url.isBlank()) {
            error = "请填写订阅源地址"
            return
        }
        busy = true
        error = ""
        val done: (String?) -> Unit = { err ->
            busy = false
            if (err == null) onDismiss() else error = err
        }
        if (target != null) {
            onUpdate(
                target.id,
                UpdateFeedBody(
                    url = url.trim(),
                    title = title.trim().ifBlank { null },
                    category = category.trim(),
                ),
                done,
            )
        } else {
            onAdd(url.trim(), title.trim().ifBlank { null }, category.trim(), done)
        }
    }

    AlertDialog(
        onDismissRequest = { if (!busy) onDismiss() },
        containerColor = Panel,
        title = { Text(if (target != null) "编辑订阅源" else "添加订阅源", fontSize = 17.sp) },
        text = {
            Column(modifier = Modifier.verticalScroll(rememberScrollState())) {
                OutlinedTextField(
                    value = url,
                    onValueChange = { url = it },
                    label = { Text("订阅源地址 (RSS / Atom)") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    "不加 https:// 也可以，服务端会自动补上",
                    fontSize = 11.sp,
                    color = TextMute,
                    modifier = Modifier.padding(start = 4.dp, top = 4.dp),
                )

                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = title,
                    onValueChange = { title = it },
                    label = { Text("显示名称（留空自动取源标题）") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )

                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = category,
                    onValueChange = { category = it },
                    label = { Text("分组（留空为「未分类」）") },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )

                if (target != null && !target.lastError.isNullOrBlank()) {
                    Spacer(Modifier.height(12.dp))
                    Text("上次拉取失败：${target.lastError}", fontSize = 12.sp, color = Danger)
                }

                if (error.isNotEmpty()) {
                    Spacer(Modifier.height(12.dp))
                    Text(error, fontSize = 12.sp, color = Danger)
                }

                if (target != null) {
                    Spacer(Modifier.height(10.dp))
                    TextButton(
                        onClick = {
                            onRefreshOne(target.id)
                            onDismiss()
                        },
                        enabled = !busy,
                    ) {
                        Text("只刷新这个源", fontSize = 13.sp)
                    }
                    TextButton(
                        onClick = {
                            if (!confirmDelete) {
                                confirmDelete = true
                            } else {
                                busy = true
                                onDelete(target.id) { err ->
                                    busy = false
                                    if (err == null) onDismiss() else error = err
                                }
                            }
                        },
                        enabled = !busy,
                    ) {
                        Text(
                            if (confirmDelete) "连同该源下所有文章一起删除，确定？" else "删除订阅源",
                            color = Danger,
                            fontSize = 13.sp,
                        )
                    }
                }
            }
        },
        confirmButton = {
            TextButton(onClick = { submit() }, enabled = !busy) {
                Text(
                    when {
                        busy -> "处理中…"
                        target != null -> "保存"
                        else -> "添加并拉取"
                    }
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !busy) { Text("取消") }
        },
    )
}
