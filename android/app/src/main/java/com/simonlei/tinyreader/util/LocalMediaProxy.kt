package com.simonlei.tinyreader.util

import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.URI
import java.net.URLDecoder
import java.util.concurrent.Executors
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/**
 * 本机图片代理。只监听 127.0.0.1 的随机端口，给白名单图床补上 Referer。
 * 白名单之外直接拒绝，避免正文里的脚本把它当成开放代理。
 */
class LocalMediaProxy(
    private val exchange: (String, String) -> Upstream = ::exchangeOkHttp,
) {
    @Volatile
    private var policy = MediaPolicy(emptyList(), "")

    @Volatile
    var baseUrl: String = ""
        private set

    private var server: ServerSocket? = null
    private val workers = Executors.newFixedThreadPool(6) { runnable ->
        Thread(runnable, "media-proxy").apply { isDaemon = true }
    }

    @Synchronized
    fun setPolicy(hosts: List<String>, referer: String): String {
        policy = MediaPolicy(hosts.toList(), referer.trim())
        if (server == null) {
            val ss = ServerSocket()
            ss.reuseAddress = true
            ss.bind(InetSocketAddress(InetAddress.getByName("127.0.0.1"), 0))
            server = ss
            baseUrl = "http://127.0.0.1:${ss.localPort}"
            thread(isDaemon = true, name = "media-proxy-accept") {
                while (true) {
                    val sock = try {
                        ss.accept()
                    } catch (_: Exception) {
                        break
                    }
                    try {
                        workers.execute { handle(sock) }
                    } catch (_: RejectedExecutionException) {
                        runCatching { sock.close() }
                    }
                }
            }
        }
        return baseUrl
    }

    fun close() {
        runCatching { server?.close() }
        workers.shutdown()
    }

    private fun handle(sock: Socket) {
        sock.use { socket ->
            socket.soTimeout = 20_000
            socket.tcpNoDelay = true
            val head = runCatching { readHeaders(socket) }.getOrNull() ?: return
            val (method, target) = runCatching { requestLine(head) }.getOrElse {
                writeResponse(socket, 400, "Bad Request", "text/plain; charset=utf-8", it.message.orEmpty().toByteArray())
                return
            }
            when (method) {
                "OPTIONS" -> {
                    writeResponse(socket, 204, "No Content", "text/plain", ByteArray(0))
                    return
                }
                "GET" -> Unit
                else -> {
                    writeResponse(socket, 405, "Method Not Allowed", "text/plain; charset=utf-8", "只接受 GET".toByteArray())
                    return
                }
            }
            val imageUrl = when (val parsed = imageUrlFromTarget(target)) {
                is Target.Image -> parsed.url
                is Target.Missing -> {
                    writeResponse(socket, 404, "Not Found", "text/plain; charset=utf-8", "不是图片代理路径".toByteArray())
                    return
                }
                is Target.Bad -> {
                    writeResponse(socket, 400, "Bad Request", "text/plain; charset=utf-8", parsed.message.toByteArray())
                    return
                }
            }
            try {
                val image = pullImage(imageUrl, policy, exchange)
                writeResponse(socket, 200, "OK", safeContentType(image.contentType), image.body, cache = true)
            } catch (e: ProxyRejected) {
                writeResponse(socket, 400, "Bad Request", "text/plain; charset=utf-8", e.message.orEmpty().toByteArray())
            } catch (_: Exception) {
                writeResponse(socket, 502, "Bad Gateway", "text/plain; charset=utf-8", "图片请求失败".toByteArray())
            }
        }
    }
}

/** 进程内唯一的本机代理，阅读页和启动流程共用。 */
object MediaProxy {
    private val proxy = LocalMediaProxy()

    fun setPolicy(hosts: List<String>, referer: String): String = proxy.setPolicy(hosts, referer)
}

data class MediaPolicy(val patterns: List<String>, val referer: String)

data class Upstream(
    val status: Int,
    val location: String?,
    val contentType: String,
    val body: ByteArray,
)

data class ImagePayload(val contentType: String, val body: ByteArray)

class ProxyRejected(message: String) : Exception(message)

private const val MAX_BYTES = 15 * 1024 * 1024
private const val MAX_HOPS = 4
private const val IMAGE_UA =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/148.0.0.0 Safari/537.36"

private sealed class Target {
    data class Image(val url: String) : Target()
    data object Missing : Target()
    data class Bad(val message: String) : Target()
}

internal fun upgradeHttpToHttps(raw: String): String {
    val trimmed = raw.trim()
    val uri = try {
        URI(trimmed)
    } catch (_: Exception) {
        throw ProxyRejected("图片地址不合法")
    }
    val scheme = uri.scheme?.lowercase() ?: throw ProxyRejected("图片地址不合法")
    if (scheme == "https") return trimmed
    if (scheme != "http") throw ProxyRejected("图片地址不合法")
    val host = uri.host ?: throw ProxyRejected("图片地址不合法")
    val port = when (uri.port) {
        -1, 80 -> -1
        else -> uri.port
    }
    return try {
        URI("https", uri.rawUserInfo, host, port, uri.rawPath, uri.rawQuery, uri.rawFragment).toASCIIString()
    } catch (_: Exception) {
        throw ProxyRejected("图片地址不合法")
    }
}

internal fun refererFor(url: String, policy: MediaPolicy): String {
    val uri = try {
        URI(url)
    } catch (_: Exception) {
        throw ProxyRejected("图片地址不合法")
    }
    if (!uri.scheme.equals("https", ignoreCase = true)) throw ProxyRejected("只代理 https 图片")
    if (!uri.rawUserInfo.isNullOrEmpty()) throw ProxyRejected("图片地址不合法")
    if (uri.port != -1 && uri.port != 443) throw ProxyRejected("图片地址端口不被允许")
    val host = uri.host ?: throw ProxyRejected("图片地址不合法")
    if (!policy.patterns.any { hostMatches(it, host) }) throw ProxyRejected("这个图片域名不走代理")
    if (policy.referer.any { it == '\r' || it == '\n' }) throw ProxyRejected("Referer 不合法")
    return policy.referer
}

internal fun resolveNext(current: String, location: String, policy: MediaPolicy): String {
    val next = try {
        URI(current).resolve(location.trim()).toString()
    } catch (_: Exception) {
        throw ProxyRejected("图片重定向地址不合法")
    }
    val upgraded = upgradeHttpToHttps(next)
    refererFor(upgraded, policy)
    return upgraded
}

internal fun pullImage(
    rawUrl: String,
    policy: MediaPolicy,
    exchange: (String, String) -> Upstream,
): ImagePayload {
    var current = upgradeHttpToHttps(rawUrl)
    refererFor(current, policy)
    repeat(MAX_HOPS) {
        val referer = refererFor(current, policy)
        val up = exchange(current, referer)
        if (up.status in 300..399) {
            val loc = up.location?.trim().orEmpty()
            if (loc.isEmpty()) throw ProxyRejected("图片重定向缺少地址（HTTP ${up.status}）")
            current = resolveNext(current, loc, policy)
            return@repeat
        }
        if (up.status !in 200..299) throw ProxyRejected("图片 HTTP ${up.status}")
        if (up.body.size > MAX_BYTES) throw ProxyRejected("图片过大")
        if (!isImage(up.contentType, up.body)) throw ProxyRejected("返回的不是图片")
        return ImagePayload(normalizeContentType(up.contentType, up.body), up.body)
    }
    throw ProxyRejected("图片重定向次数过多")
}

private fun imageUrlFromTarget(target: String): Target {
    val path = target.substringBefore('?')
    if (path != "/api/media") return Target.Missing
    val query = target.substringAfter('?', "")
    if (query.isEmpty()) return Target.Bad("缺少图片地址")
    val url = query.split('&').firstNotNullOfOrNull { part ->
        val key = URLDecoder.decode(part.substringBefore('='), "UTF-8")
        val value = URLDecoder.decode(part.substringAfter('=', ""), "UTF-8")
        if (key == "url" && value.isNotBlank()) value else null
    }
    return if (url == null) Target.Bad("缺少图片地址") else Target.Image(url)
}

private fun requestLine(head: String): Pair<String, String> {
    val line = head.lineSequence().firstOrNull()?.trim().orEmpty()
    val parts = line.split(' ')
    if (parts.size < 2) throw ProxyRejected("请求不合法")
    return parts[0].uppercase() to parts[1]
}

private fun readHeaders(socket: Socket): String {
    val input = socket.getInputStream()
    val out = ByteArrayOutputStream()
    var seen = 0
    while (out.size() < 16 * 1024) {
        val b = input.read()
        if (b < 0) break
        out.write(b)
        val byte = b.toByte()
        seen = when {
            byte == '\r'.code.toByte() && seen % 2 == 0 -> seen + 1
            byte == '\n'.code.toByte() && seen % 2 == 1 -> seen + 1
            byte == '\r'.code.toByte() -> 1
            else -> 0
        }
        if (seen == 4) break
    }
    return out.toString(Charsets.ISO_8859_1)
}

private fun writeResponse(
    socket: Socket,
    status: Int,
    reason: String,
    contentType: String,
    body: ByteArray,
    cache: Boolean = false,
) {
    val cacheLine = if (cache) "Cache-Control: private, max-age=86400\r\n" else ""
    val head = "HTTP/1.1 $status $reason\r\n" +
        "Content-Type: $contentType\r\n" +
        "Content-Length: ${body.size}\r\n" +
        cacheLine +
        "Connection: close\r\n" +
        "Access-Control-Allow-Origin: *\r\n" +
        "Access-Control-Allow-Private-Network: true\r\n" +
        "\r\n"
    val out = socket.getOutputStream()
    out.write(head.toByteArray(Charsets.ISO_8859_1))
    out.write(body)
    out.flush()
}

private fun safeContentType(ct: String): String {
    val token = ct.substringBefore(';').trim()
    return if (token.startsWith("image/") && token.all { it.code in 33..126 }) token else "application/octet-stream"
}

private fun isImage(contentType: String, body: ByteArray): Boolean {
    if (contentType.lowercase().startsWith("image/")) return true
    return looksLikeImage(body)
}

private fun looksLikeImage(body: ByteArray): Boolean {
    if (body.size >= 4 && body[0] == 0x89.toByte() && body[1] == 0x50.toByte() && body[2] == 0x4E.toByte() && body[3] == 0x47.toByte()) {
        return true
    }
    if (body.size >= 2 && body[0] == 0xFF.toByte() && body[1] == 0xD8.toByte()) return true
    if (body.size >= 4 && body[0] == 'G'.code.toByte() && body[1] == 'I'.code.toByte() && body[2] == 'F'.code.toByte() && body[3] == '8'.code.toByte()) {
        return true
    }
    return body.size >= 12 &&
        body.copyOfRange(0, 4).toString(Charsets.US_ASCII) == "RIFF" &&
        body.copyOfRange(8, 12).toString(Charsets.US_ASCII) == "WEBP"
}

private fun normalizeContentType(contentType: String, body: ByteArray): String {
    val ct = contentType.substringBefore(';').trim().lowercase()
    if (ct.startsWith("image/")) return ct
    return when {
        body.size >= 4 && body[0] == 0x89.toByte() && body[1] == 0x50.toByte() -> "image/png"
        body.size >= 2 && body[0] == 0xFF.toByte() && body[1] == 0xD8.toByte() -> "image/jpeg"
        body.size >= 4 && body[0] == 'G'.code.toByte() && body[1] == 'I'.code.toByte() -> "image/gif"
        body.size >= 12 && body.copyOfRange(0, 4).toString(Charsets.US_ASCII) == "RIFF" -> "image/webp"
        else -> "application/octet-stream"
    }
}

private val httpClient: OkHttpClient by lazy {
    OkHttpClient.Builder()
        .followRedirects(false)
        .followSslRedirects(false)
        .callTimeout(20, TimeUnit.SECONDS)
        .build()
}

private fun exchangeOkHttp(url: String, referer: String): Upstream {
    val builder = Request.Builder()
        .url(url)
        .header("Accept", "image/avif,image/webp,image/apng,image/*,*/*;q=0.8")
        .header("User-Agent", IMAGE_UA)
    if (referer.isNotEmpty()) builder.header("Referer", referer)
    try {
        httpClient.newCall(builder.build()).execute().use { resp ->
            val len = resp.body?.contentLength() ?: -1
            if (len > MAX_BYTES) throw ProxyRejected("图片过大")
            val body = resp.body?.byteStream()?.use { input ->
                val buf = ByteArrayOutputStream()
                val tmp = ByteArray(8192)
                var total = 0
                while (true) {
                    val n = input.read(tmp)
                    if (n < 0) break
                    total += n
                    if (total > MAX_BYTES) throw ProxyRejected("图片过大")
                    buf.write(tmp, 0, n)
                }
                buf.toByteArray()
            } ?: ByteArray(0)
            return Upstream(resp.code, resp.header("Location"), resp.header("Content-Type").orEmpty(), body)
        }
    } catch (e: ProxyRejected) {
        throw e
    } catch (e: IOException) {
        throw ProxyRejected("图片请求失败: ${e.message}")
    }
}
