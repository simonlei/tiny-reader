package com.simonlei.tinyreader.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.simonlei.tinyreader.ui.theme.Bg
import com.simonlei.tinyreader.ui.theme.BgAlt
import com.simonlei.tinyreader.ui.theme.Danger
import com.simonlei.tinyreader.ui.theme.Ok
import com.simonlei.tinyreader.ui.theme.TextDim
import com.simonlei.tinyreader.ui.theme.TextMain
import com.simonlei.tinyreader.ui.theme.TextMute
import com.simonlei.tinyreader.util.AppUpdater
import com.simonlei.tinyreader.util.InstallStart
import com.simonlei.tinyreader.util.UpdateDecision
import com.simonlei.tinyreader.util.UpdateOffer
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.launch

/**
 * 服务端设置，对应桌面端 `SettingsDialog.vue`。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    ui: UiState,
    canClose: Boolean,
    versionName: String,
    updater: AppUpdater,
    onClose: () -> Unit,
    onSave: (String, String) -> Unit,
    onTest: (String, String, (Boolean, String) -> Unit) -> Unit,
) {
    var serverUrl by remember { mutableStateOf(ui.serverUrl) }
    var token by remember { mutableStateOf(ui.token) }
    var testing by remember { mutableStateOf(false) }
    var testOk by remember { mutableStateOf<Boolean?>(null) }
    var testMsg by remember { mutableStateOf("") }
    val scope = rememberCoroutineScope()
    var checking by remember { mutableStateOf(false) }
    var downloading by remember { mutableStateOf(false) }
    var downloaded by remember { mutableLongStateOf(0L) }
    var totalBytes by remember { mutableLongStateOf(-1L) }
    var updateOk by remember { mutableStateOf<Boolean?>(null) }
    var updateMsg by remember { mutableStateOf("") }
    var pendingUpdate by remember { mutableStateOf<UpdateOffer?>(null) }

    fun checkUpdate() {
        checking = true
        updateOk = null
        updateMsg = ""
        pendingUpdate = null
        scope.launch {
            try {
                when (val decision = updater.check(versionName)) {
                    UpdateDecision.UpToDate -> {
                        updateOk = true
                        updateMsg = "当前已是最新版 $versionName"
                    }
                    is UpdateDecision.Available -> {
                        pendingUpdate = decision.offer
                        updateOk = true
                        updateMsg = "发现新版本 ${decision.offer.version}"
                    }
                }
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                updateOk = false
                updateMsg = e.message ?: "检查更新失败"
            } finally {
                checking = false
            }
        }
    }

    fun installUpdate(offer: UpdateOffer) {
        downloading = true
        downloaded = 0
        totalBytes = -1
        updateOk = null
        updateMsg = ""
        scope.installOffer(
            updater,
            offer,
            onProgress = { got, all ->
                downloaded = got
                totalBytes = all
            },
            onSuccess = { start ->
                downloading = false
                updateOk = true
                updateMsg = when (start) {
                    InstallStart.NeedPermission -> "请允许安装未知应用，返回后会继续升级"
                    InstallStart.Opened -> "已打开系统安装器，确认后即可完成升级"
                }
            },
            onError = { msg ->
                downloading = false
                updateOk = false
                updateMsg = msg
            },
        )
    }

    Scaffold(
        containerColor = Bg,
        topBar = {
            TopAppBar(
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = BgAlt,
                    titleContentColor = TextMain,
                ),
                navigationIcon = {
                    if (canClose) {
                        IconButton(onClick = onClose) {
                            Icon(
                                Icons.AutoMirrored.Filled.ArrowBack,
                                contentDescription = "返回",
                                tint = TextDim,
                            )
                        }
                    }
                },
                title = { Text("服务端设置", fontSize = 16.sp, fontWeight = FontWeight.SemiBold) },
            )
        },
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 16.dp, vertical = 12.dp),
        ) {
            Text(
                "阅读状态保存在服务端，多个客户端填同一个地址和 token 即可共享已读 / 星标。",
                fontSize = 13.sp,
                color = TextMute,
            )

            Spacer(Modifier.height(16.dp))

            OutlinedTextField(
                value = serverUrl,
                onValueChange = { serverUrl = it; testOk = null },
                label = { Text("服务端地址") },
                placeholder = { Text("http://192.168.1.10:8787") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Text(
                "手机不能用 127.0.0.1，要填服务端那台机器的局域网 IP，"
                    + "同时把服务端 config.toml 里的 server.host 改成 0.0.0.0。",
                fontSize = 11.sp,
                color = TextMute,
                modifier = Modifier.padding(top = 4.dp, start = 4.dp),
            )

            Spacer(Modifier.height(14.dp))

            OutlinedTextField(
                value = token,
                onValueChange = { token = it; testOk = null },
                label = { Text("访问 token") },
                placeholder = { Text("填服务端 config.toml 里的 auth.token") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )

            Spacer(Modifier.height(18.dp))

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedButton(
                    onClick = {
                        testing = true
                        testOk = null
                        onTest(serverUrl, token) { ok, msg ->
                            testing = false
                            testOk = ok
                            testMsg = msg
                        }
                    },
                    enabled = !testing && serverUrl.isNotBlank() && token.isNotBlank(),
                ) {
                    Text(if (testing) "测试中…" else "测试连接")
                }
                Button(
                    onClick = { onSave(serverUrl, token) },
                    enabled = serverUrl.isNotBlank() && token.isNotBlank(),
                ) {
                    Text("保存")
                }
            }

            if (testOk != null) {
                Spacer(Modifier.height(12.dp))
                Text(
                    (if (testOk == true) "✓ " else "✕ ") + testMsg,
                    fontSize = 13.sp,
                    color = if (testOk == true) Ok else Danger,
                )
            }

            Spacer(Modifier.height(28.dp))

            Text("版本与更新", fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextDim)
            Spacer(Modifier.height(8.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("当前版本 $versionName", fontSize = 13.sp, color = TextMute)
                OutlinedButton(
                    onClick = { checkUpdate() },
                    enabled = !checking && !downloading,
                ) {
                    Text(if (checking) "检查中…" else "检查更新")
                }
            }
            Text(
                "从发布通道检查新版本。确认后会下载安装包，校验通过再交给系统安装器升级。",
                fontSize = 11.sp,
                color = TextMute,
                modifier = Modifier.padding(top = 4.dp),
            )
            if (updateOk != null) {
                Spacer(Modifier.height(8.dp))
                Text(
                    updateMsg,
                    fontSize = 13.sp,
                    color = if (updateOk == true) Ok else Danger,
                )
            }
            val offer = pendingUpdate
            if (offer != null) {
                Spacer(Modifier.height(10.dp))
                Text("更新说明", fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextDim)
                Spacer(Modifier.height(4.dp))
                Text(
                    offer.notes.ifBlank { "（本次发布无说明）" },
                    fontSize = 12.sp,
                    color = TextMute,
                )
                Spacer(Modifier.height(10.dp))
                Button(
                    onClick = { installUpdate(offer) },
                    enabled = !downloading,
                ) {
                    Text(
                        when {
                            downloading && totalBytes > 0 -> "下载中 ${downloaded * 100 / totalBytes}%"
                            downloading -> "下载中…"
                            else -> "下载并安装更新"
                        },
                    )
                }
                if (downloading) {
                    Spacer(Modifier.height(8.dp))
                    UpdateProgressBar(downloaded, totalBytes)
                }
                Text(
                    "安装时系统会要求确认。若提示不允许安装未知应用，先允许本应用，返回后会继续。",
                    fontSize = 11.sp,
                    color = TextMute,
                    modifier = Modifier.padding(top = 6.dp),
                )
            }

            Spacer(Modifier.height(28.dp))

            Text("关于", fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextDim)
            Spacer(Modifier.height(6.dp))
            Text(
                "Tiny Reader for Android · $versionName\n"
                    + "与桌面端（Windows / macOS）共用同一个 Rust 服务端。",
                fontSize = 12.sp,
                color = TextMute,
            )

            Spacer(Modifier.height(18.dp))
            Text("使用提示", fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextDim)
            Spacer(Modifier.height(6.dp))
            Text(
                "· 点开文章 0.6 秒后自动标为已读，列表不会移除，方便回看\n"
                    + "· 阅读页左右滑动切换上 / 下一篇，滑到末尾自动加载更多\n"
                    + "· 左上角菜单切换订阅源，顶部切换 全部 / 未读 / 星标\n"
                    + "· 列表下拉可刷新所有订阅源\n"
                    + "· 长按抽屉里的订阅源可以编辑或删除",
                fontSize = 12.sp,
                color = TextMute,
            )
        }
    }
}
