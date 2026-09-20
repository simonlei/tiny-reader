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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
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

/**
 * 服务端设置，对应桌面端 `SettingsDialog.vue`。
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(
    ui: UiState,
    canClose: Boolean,
    onClose: () -> Unit,
    onSave: (String, String) -> Unit,
    onTest: (String, String, (Boolean, String) -> Unit) -> Unit,
) {
    var serverUrl by remember { mutableStateOf(ui.serverUrl) }
    var token by remember { mutableStateOf(ui.token) }
    var testing by remember { mutableStateOf(false) }
    var testOk by remember { mutableStateOf<Boolean?>(null) }
    var testMsg by remember { mutableStateOf("") }

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

            Text("关于", fontSize = 12.sp, fontWeight = FontWeight.SemiBold, color = TextDim)
            Spacer(Modifier.height(6.dp))
            Text(
                "Tiny Reader for Android · 0.1.0\n"
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
