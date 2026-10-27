package com.simonlei.tinyreader.util

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.core.content.FileProvider
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.withContext
import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.File
import java.util.concurrent.TimeUnit
import kotlin.coroutines.coroutineContext

sealed interface InstallStart {
    data object Opened : InstallStart
    data object NeedPermission : InstallStart
}

/** 已安装包的 versionName，设置页和启动检查都用它跟发布通道比较。 */
fun installedVersionName(context: Context): String {
    val info = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        context.packageManager.getPackageInfo(
            context.packageName,
            android.content.pm.PackageManager.PackageInfoFlags.of(0),
        )
    } else {
        @Suppress("DEPRECATION")
        context.packageManager.getPackageInfo(context.packageName, 0)
    }
    return info.versionName?.trim().takeUnless { it.isNullOrEmpty() } ?: "0.0.0"
}

/**
 * 拉取 android.json、下载 APK，再交给系统安装器覆盖安装。
 * 普通应用不能静默升级，最后一步仍要用户在系统界面点确认。
 */
class AppUpdater(context: Context) {
    private val context = context.applicationContext
    private val gate = Mutex()
    private val mainHandler = Handler(Looper.getMainLooper())

    private var pendingApk: File?
        get() = pendingApkPath?.let(::File)
        set(value) {
            pendingApkPath = value?.absolutePath
        }

    suspend fun check(currentVersion: String): UpdateDecision = withContext(Dispatchers.IO) {
        try {
            val request = Request.Builder()
                .url(UPDATE_MANIFEST_URL)
                .header("User-Agent", "TinyReader-Android/$currentVersion")
                .header("Accept", "application/json")
                .header("Cache-Control", "no-cache")
                .build()
            http.newCall(request).execute().use { response ->
                val body = response.body ?: throw UpdateRejected("检查更新失败（空响应）")
                if (body.contentLength() > MAX_MANIFEST_BYTES) throw UpdateRejected("更新清单过大")
                val text = body.byteStream().use { readLimited(it, MAX_MANIFEST_BYTES) }
                if (response.code != 200) throw UpdateRejected("检查更新失败（HTTP ${response.code}）")
                decideUpdate(text, currentVersion)
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: UpdateRejected) {
            throw e
        } catch (e: Exception) {
            throw UpdateRejected("检查更新失败：${e.message ?: "网络错误"}")
        }
    }

    suspend fun downloadAndInstall(
        offer: UpdateOffer,
        onProgress: (downloaded: Long, total: Long) -> Unit,
    ): InstallStart {
        val file = download(offer, onProgress)
        return startInstall(file)
    }

    fun installPendingIfReady(): Boolean {
        val apk = pendingApk ?: return false
        if (!apk.isFile || !canRequestInstall()) return false
        pendingApk = null
        launchInstaller(apk)
        return true
    }

    private suspend fun download(
        offer: UpdateOffer,
        onProgress: (downloaded: Long, total: Long) -> Unit,
    ): File {
        if (!gate.tryLock()) throw UpdateRejected("正在下载更新")
        try {
            return downloadLocked(offer, onProgress)
        } finally {
            gate.unlock()
        }
    }

    private suspend fun downloadLocked(
        offer: UpdateOffer,
        onProgress: (downloaded: Long, total: Long) -> Unit,
    ): File = withContext(Dispatchers.IO) {
            if (offer.url != expectedApkUrl(offer.version)) throw UpdateRejected("更新包地址不可信")
            // 远端对象名是 .bin（COS 默认域名不分发 .apk），本地落盘仍叫 .apk，
            // 交给系统安装器时按 MIME + 扩展名识别，不受远端名字影响。
            val file = File(File(context.cacheDir, "updates").apply { mkdirs() }, "tiny-reader-update.apk")
            if (file.exists() && !file.delete()) throw UpdateRejected("无法清理上次的安装包")
            val job = coroutineContext[Job]
            try {
                file.outputStream().use { out ->
                    val request = Request.Builder()
                        .url(offer.url)
                        .header("User-Agent", "TinyReader-Android/${offer.version}")
                        .header("Accept-Encoding", "identity")
                        .build()
                    http.newCall(request).execute().use { response ->
                        if (response.code != 200) throw UpdateRejected("下载失败（HTTP ${response.code}）")
                        val body = response.body ?: throw UpdateRejected("下载失败（空响应）")
                        body.byteStream().use { input ->
                            copyAndVerify(
                                input = input,
                                output = out,
                                expectedSha256 = offer.sha256,
                                contentLength = body.contentLength(),
                                shouldContinue = { job?.isActive != false },
                                onProgress = { got, total ->
                                    mainHandler.post { onProgress(got, total) }
                                },
                            )
                        }
                    }
                }
                file
            } catch (t: Throwable) {
                file.delete()
                when (t) {
                    is CancellationException, is UpdateRejected -> throw t
                    else -> throw UpdateRejected("下载失败：${t.message ?: "网络错误"}")
                }
            }
    }

    private fun startInstall(apk: File): InstallStart {
        if (!canRequestInstall()) {
            pendingApk = apk
            try {
                context.startActivity(
                    Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                        data = Uri.parse("package:${context.packageName}")
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    },
                )
            } catch (_: ActivityNotFoundException) {
                pendingApk = null
                throw UpdateRejected("无法打开安装权限设置")
            } catch (_: SecurityException) {
                pendingApk = null
                throw UpdateRejected("无法打开安装权限设置")
            }
            return InstallStart.NeedPermission
        }
        pendingApk = null
        launchInstaller(apk)
        return InstallStart.Opened
    }

    private fun canRequestInstall(): Boolean = context.packageManager.canRequestPackageInstalls()

    private fun launchInstaller(apk: File) {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", apk)
        val intent = Intent(Intent.ACTION_VIEW).apply {
            setDataAndType(uri, APK_MIME)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        try {
            context.startActivity(intent)
        } catch (_: ActivityNotFoundException) {
            throw UpdateRejected("无法打开系统安装器")
        } catch (_: SecurityException) {
            throw UpdateRejected("无法打开系统安装器")
        }
    }

    private fun readLimited(input: java.io.InputStream, maxBytes: Int): String {
        val out = java.io.ByteArrayOutputStream()
        val buf = ByteArray(4096)
        var total = 0
        while (true) {
            val n = input.read(buf)
            if (n < 0) break
            total += n
            if (total > maxBytes) throw UpdateRejected("更新清单过大")
            out.write(buf, 0, n)
        }
        return out.toString(Charsets.UTF_8.name())
    }

    companion object {
        private const val APK_MIME = "application/vnd.android.package-archive"
        private const val MAX_MANIFEST_BYTES = 64 * 1024

        @Volatile
        private var pendingApkPath: String? = null

        private val http: OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(15, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            .followRedirects(false)
            .followSslRedirects(false)
            .retryOnConnectionFailure(true)
            .build()
    }
}
