package com.simonlei.tinyreader

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import android.net.Uri
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.DrawerValue
import androidx.compose.material3.ModalDrawerSheet
import androidx.compose.material3.ModalNavigationDrawer
import androidx.compose.material3.SnackbarHost
import androidx.compose.material3.SnackbarHostState
import androidx.compose.material3.rememberDrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.compose.material3.Surface
import com.simonlei.tinyreader.data.Feed
import com.simonlei.tinyreader.ui.ArticleListScreen
import com.simonlei.tinyreader.ui.FeedEditDialog
import com.simonlei.tinyreader.ui.ReaderScreen
import com.simonlei.tinyreader.ui.ReaderViewModel
import com.simonlei.tinyreader.ui.SettingsScreen
import com.simonlei.tinyreader.ui.FeedDrawerContent
import com.simonlei.tinyreader.ui.theme.Bg
import com.simonlei.tinyreader.ui.theme.Panel
import com.simonlei.tinyreader.ui.theme.TinyReaderTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            TinyReaderTheme {
                Surface(color = Bg, modifier = Modifier.fillMaxSize()) {
                    AppRoot()
                }
            }
        }
    }
}

/** 订阅源对话框状态 */
private sealed interface FeedDialogState {
    data object Add : FeedDialogState
    data class Edit(val feed: Feed) : FeedDialogState
}

@Composable
private fun AppRoot(vm: ReaderViewModel = viewModel()) {
    val ui by vm.ui.collectAsState()
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    val drawerState = rememberDrawerState(DrawerValue.Closed)
    val snackbarHostState = remember { SnackbarHostState() }

    var showSettings by remember { mutableStateOf(false) }
    var feedDialog by remember { mutableStateOf<FeedDialogState?>(null) }

    // OPML 导入：选文件 → 读文本 → POST /api/feeds/import
    val opmlPicker = rememberOpmlOpenLauncher { uri ->
        scope.launch {
            val xml = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                }.getOrNull()
            }
            if (xml.isNullOrBlank()) vm.showToast("读取文件失败") else vm.importOpml(xml)
        }
    }

    // OPML 导出：GET /api/feeds/export → 写入用户选择的文件
    var pendingOpml by remember { mutableStateOf<String?>(null) }
    val opmlSaver = rememberOpmlSaveLauncher { uri ->
        val xml = pendingOpml
        pendingOpml = null
        if (xml == null) return@rememberOpmlSaveLauncher
        scope.launch {
            val ok = withContext(Dispatchers.IO) {
                runCatching {
                    context.contentResolver.openOutputStream(uri)?.use { out ->
                        out.write(xml.toByteArray(Charsets.UTF_8))
                    }
                }.isSuccess
            }
            vm.showToast(if (ok) "已导出 OPML" else "写入文件失败")
        }
    }

    // 一次性提示
    LaunchedEffect(ui.toast) {
        val msg = ui.toast
        if (msg != null) {
            snackbarHostState.showSnackbar(msg)
            vm.dismissToast()
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        when {
            // 首启必须先配置服务端
            ui.needsSetup || showSettings -> SettingsScreen(
                ui = ui,
                canClose = !ui.needsSetup,
                onClose = { showSettings = false },
                onSave = { url, token ->
                    vm.saveSettings(url, token)
                    showSettings = false
                },
                onTest = { url, token, cb -> vm.testConnection(url, token, cb) },
            )

            ui.readerOpen -> ReaderScreen(
                ui = ui,
                onClose = { vm.closeReader() },
                onMove = { delta -> vm.move(delta) },
                onPrefetch = { vm.prefetchIfNearEnd() },
                onToggleStar = { vm.toggleStar() },
                onToggleRead = { vm.toggleRead() },
            )

            else -> ModalNavigationDrawer(
                drawerState = drawerState,
                drawerContent = {
                    ModalDrawerSheet(drawerContainerColor = Panel) {
                        FeedDrawerContent(
                            ui = ui,
                            onSelectFeed = { id ->
                                vm.selectFeed(id)
                                scope.launch { drawerState.close() }
                            },
                            onEditFeed = { feed ->
                                feedDialog = FeedDialogState.Edit(feed)
                                scope.launch { drawerState.close() }
                            },
                            onAddFeed = {
                                feedDialog = FeedDialogState.Add
                                scope.launch { drawerState.close() }
                            },
                            onImportOpml = {
                                opmlPicker()
                                scope.launch { drawerState.close() }
                            },
                            onExportOpml = {
                                vm.exportOpml { xml ->
                                    pendingOpml = xml
                                    opmlSaver()
                                }
                                scope.launch { drawerState.close() }
                            },
                            onOpenSettings = {
                                showSettings = true
                                scope.launch { drawerState.close() }
                            },
                        )
                    }
                },
            ) {
                ArticleListScreen(
                    ui = ui,
                    onOpenDrawer = { scope.launch { drawerState.open() } },
                    onOpenArticle = { id -> vm.openArticle(id) },
                    onSetFilter = { filter -> vm.setFilter(filter) },
                    onSubmitKeyword = { kw -> vm.submitKeyword(kw) },
                    onToggleStar = { article -> vm.toggleStar(article) },
                    onMarkAllRead = { vm.markAllRead() },
                    onRefreshAll = { vm.refreshAll() },
                    onOpenSettings = { showSettings = true },
                    onLoadMore = { vm.loadMore() },
                )
            }
        }

        val dialog = feedDialog
        if (dialog != null) {
            FeedEditDialog(
                feed = (dialog as? FeedDialogState.Edit)?.feed,
                onDismiss = { feedDialog = null },
                onAdd = { url, title, category, onDone ->
                    vm.addFeed(url, title, category, onDone)
                },
                onUpdate = { id, patch, onDone -> vm.updateFeed(id, patch, onDone) },
                onDelete = { id, onDone -> vm.removeFeed(id, onDone) },
                onRefreshOne = { id -> vm.refreshOne(id) },
            )
        }

        SnackbarHost(
            hostState = snackbarHostState,
            modifier = Modifier.align(Alignment.BottomCenter),
        )
    }
}

/** 选择一个 OPML 文件用于导入 */
@Composable
private fun rememberOpmlOpenLauncher(onPicked: (Uri) -> Unit): () -> Unit {
    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(),
    ) { uri -> if (uri != null) onPicked(uri) }

    return {
        launcher.launch(
            arrayOf("text/xml", "application/xml", "text/x-opml", "text/plain", "*/*")
        )
    }
}

/** 选择保存位置用于导出 OPML */
@Composable
private fun rememberOpmlSaveLauncher(onPicked: (Uri) -> Unit): () -> Unit {
    val launcher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.CreateDocument("text/xml"),
    ) { uri -> if (uri != null) onPicked(uri) }

    return { launcher.launch("tiny-reader-subscriptions.opml") }
}
