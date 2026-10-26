package com.simonlei.tinyreader.ui

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.ui.theme.Accent
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.Panel
import com.simonlei.tinyreader.ui.theme.Panel2
import com.simonlei.tinyreader.ui.theme.TextMute
import com.simonlei.tinyreader.util.AppUpdater
import com.simonlei.tinyreader.util.InstallStart
import com.simonlei.tinyreader.util.UpdateOffer
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

@Composable
fun AppUpdateDialog(
    updater: AppUpdater,
    offer: UpdateOffer,
    onDismiss: () -> Unit,
    onMessage: (String) -> Unit,
) {
    val scope = rememberCoroutineScope()
    var downloading by remember { mutableStateOf(false) }
    var downloaded by remember { mutableLongStateOf(0L) }
    var total by remember { mutableLongStateOf(-1L) }
    var error by remember { mutableStateOf("") }

    AlertDialog(
        onDismissRequest = { if (!downloading) onDismiss() },
        containerColor = Panel,
        title = { Text("发现新版本 ${offer.version}", fontSize = 17.sp) },
        text = {
            Column {
                Text(
                    offer.notes.ifBlank { "有新版本可以安装。" },
                    fontSize = 13.sp,
                    color = TextMute,
                    modifier = Modifier
                        .heightIn(max = 200.dp)
                        .verticalScroll(rememberScrollState()),
                )
                if (downloading) {
                    Spacer(Modifier.height(12.dp))
                    UpdateProgressBar(downloaded, total)
                }
                if (error.isNotEmpty()) {
                    Spacer(Modifier.height(8.dp))
                    Text(error, fontSize = 12.sp, color = Danger)
                }
            }
        },
        confirmButton = {
            TextButton(
                enabled = !downloading,
                onClick = {
                    downloading = true
                    error = ""
                    downloaded = 0
                    total = -1
                    scope.installOffer(
                        updater,
                        offer,
                        onProgress = { got, all ->
                            downloaded = got
                            total = all
                        },
                        onSuccess = { start ->
                            downloading = false
                            onMessage(
                                when (start) {
                                    InstallStart.NeedPermission -> "请允许安装未知应用，返回后会继续升级"
                                    InstallStart.Opened -> "已打开系统安装器，确认后即可完成升级"
                                },
                            )
                            onDismiss()
                        },
                        onError = { msg ->
                            downloading = false
                            error = msg
                        },
                    )
                },
            ) {
                Text(if (downloading) "下载中…" else "下载并安装")
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss, enabled = !downloading) { Text("稍后") }
        },
    )
}

@Composable
internal fun UpdateProgressBar(downloaded: Long, total: Long) {
    if (total > 0) {
        LinearProgressIndicator(
            progress = { (downloaded.toFloat() / total.toFloat()).coerceIn(0f, 1f) },
            modifier = Modifier.fillMaxWidth(),
            color = Accent,
            trackColor = Panel2,
        )
        Text(
            "${downloaded * 100 / total}%",
            fontSize = 11.sp,
            color = TextMute,
            modifier = Modifier.padding(top = 4.dp),
        )
    } else {
        LinearProgressIndicator(
            modifier = Modifier.fillMaxWidth(),
            color = Accent,
            trackColor = Panel2,
        )
    }
}

internal fun CoroutineScope.installOffer(
    updater: AppUpdater,
    offer: UpdateOffer,
    onProgress: (Long, Long) -> Unit,
    onSuccess: (InstallStart) -> Unit,
    onError: (String) -> Unit,
) = launch {
    try {
        onSuccess(updater.downloadAndInstall(offer, onProgress))
    } catch (e: CancellationException) {
        throw e
    } catch (e: Exception) {
        if (isActive) onError(e.message ?: "更新失败")
    }
}
