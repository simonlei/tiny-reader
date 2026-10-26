package com.simonlei.tinyreader.util

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import java.io.InputStream
import java.io.OutputStream
import java.security.MessageDigest

/**
 * 安卓更新通道。清单和安装包都在桌面端同一个 CDN 上。
 * 地址写死在这里，发布脚本 `.github/workflows/release.yml` 必须传到同一个 host，
 * 安装包文件名必须是 `tiny-reader-android-vX.Y.Z.apk`。
 */
const val UPDATE_CDN_HOST = "tiny-reader-1251477527.cos.ap-guangzhou.myqcloud.com"
const val UPDATE_MANIFEST_URL = "https://$UPDATE_CDN_HOST/android.json"
const val MAX_APK_BYTES = 80L * 1024 * 1024

data class UpdateOffer(
    val version: String,
    val notes: String,
    val url: String,
    val sha256: String,
)

sealed interface UpdateDecision {
    data object UpToDate : UpdateDecision
    data class Available(val offer: UpdateOffer) : UpdateDecision
}

class UpdateRejected(message: String) : Exception(message)

private const val MAX_NOTES = 4000

private val manifestJson = Json { ignoreUnknownKeys = true }

@Serializable
private data class ManifestDto(
    val version: String,
    val notes: String? = null,
    val url: String,
    val sha256: String,
)

private data class SemVer(val major: Long, val minor: Long, val patch: Long) : Comparable<SemVer> {
    override fun compareTo(other: SemVer): Int {
        if (major != other.major) return major.compareTo(other.major)
        if (minor != other.minor) return minor.compareTo(other.minor)
        return patch.compareTo(other.patch)
    }

    override fun toString(): String = "$major.$minor.$patch"
}

fun expectedApkUrl(version: String): String {
    val ver = version.trim().removePrefix("v").trim()
    return "https://$UPDATE_CDN_HOST/tiny-reader-android-v$ver.apk"
}

/**
 * 读发布通道上的 android.json。远程版本不比当前新时返回 [UpdateDecision.UpToDate]，
 * 更高时只接受钉死在 CDN 上、且文件名与版本号一致的安装包。
 */
fun decideUpdate(manifestJsonText: String, currentVersion: String): UpdateDecision {
    val dto = try {
        manifestJson.decodeFromString<ManifestDto>(manifestJsonText)
    } catch (_: Exception) {
        throw UpdateRejected("更新清单无法解析")
    }
    val remote = parseSemver(dto.version) ?: throw UpdateRejected("无法识别的版本号")
    val current = parseSemver(currentVersion) ?: throw UpdateRejected("无法识别的版本号")
    if (remote <= current) return UpdateDecision.UpToDate
    val sha256 = normalizeSha256(dto.sha256) ?: throw UpdateRejected("更新包缺少 sha256")
    val version = remote.toString()
    val url = dto.url.trim()
    if (url != expectedApkUrl(version)) throw UpdateRejected("更新包地址不可信")
    val notes = dto.notes.orEmpty().trim().take(MAX_NOTES)
    return UpdateDecision.Available(UpdateOffer(version, notes, url, sha256))
}

/**
 * 把安装包写入 [output]，边下边算 sha256。长度、摘要或调用方取消都不留可用结果给安装器。
 * [contentLength] 未知时传负数。
 */
fun copyAndVerify(
    input: InputStream,
    output: OutputStream,
    expectedSha256: String,
    contentLength: Long,
    maxBytes: Long = MAX_APK_BYTES,
    shouldContinue: () -> Boolean = { true },
    onProgress: (downloaded: Long, total: Long) -> Unit = { _, _ -> },
) {
    if (contentLength > maxBytes) throw UpdateRejected("更新包过大")
    if (contentLength == 0L) throw UpdateRejected("更新包是空的")
    val expected = normalizeSha256(expectedSha256) ?: throw UpdateRejected("更新包校验失败")
    val digest = MessageDigest.getInstance("SHA-256")
    val buf = ByteArray(64 * 1024)
    var downloaded = 0L
    while (true) {
        if (!shouldContinue()) throw UpdateRejected("下载已取消")
        val n = input.read(buf)
        if (n < 0) break
        if (n == 0) continue
        if (downloaded + n > maxBytes) throw UpdateRejected("更新包过大")
        output.write(buf, 0, n)
        digest.update(buf, 0, n)
        downloaded += n
        onProgress(downloaded, contentLength)
    }
    if (downloaded == 0L) throw UpdateRejected("更新包是空的")
    if (contentLength >= 0 && downloaded != contentLength) {
        throw UpdateRejected("下载不完整：$downloaded/$contentLength")
    }
    val actual = digest.digest().toHex()
    if (actual != expected) throw UpdateRejected("更新包校验失败")
}

private fun parseSemver(raw: String): SemVer? {
    val parts = raw.trim().removePrefix("v").trim().split('.')
    if (parts.size != 3) return null
    val nums = parts.map { part ->
        if (part.isEmpty() || part.any { !it.isDigit() }) return null
        part.toLongOrNull() ?: return null
    }
    return SemVer(nums[0], nums[1], nums[2])
}

private fun normalizeSha256(raw: String): String? {
    var hex = raw.trim()
    if (hex.startsWith("sha256:", ignoreCase = true)) {
        hex = hex.substring("sha256:".length).trim()
    }
    if (hex.length != 64 || hex.any { it !in '0'..'9' && it !in 'a'..'f' && it !in 'A'..'F' }) {
        return null
    }
    return hex.lowercase()
}

private fun ByteArray.toHex(): String =
    joinToString("") { "%02x".format(it.toInt() and 0xff) }
