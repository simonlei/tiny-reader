package com.simonlei.tinyreader.util

import com.simonlei.tinyreader.data.Article
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.net.HttpURLConnection
import java.net.URL
import java.net.URLEncoder
import java.util.concurrent.atomic.AtomicInteger

class LocalMediaProxyTest {
    private val png = byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1)

    private fun policy() = MediaPolicy(listOf("img.wangmoyu.com", "*.cdn.dfyun.com.cn"), "https://jandan.net/")

    @Test
    fun `明文 http 会改成 https 再带上 Referer`() {
        val hits = mutableListOf<Pair<String, String>>()
        val image = pullImage("http://img.wangmoyu.com:80/a.png?q=1", policy()) { url, referer ->
            hits += url to referer
            Upstream(200, null, "image/png", png)
        }
        assertEquals(listOf("https://img.wangmoyu.com/a.png?q=1" to "https://jandan.net/"), hits)
        assertEquals("image/png", image.contentType)
        assertEquals(0x89.toByte(), image.body[0])
    }

    @Test
    fun `跳转必须仍在白名单里`() {
        val hits = mutableListOf<String>()
        pullImage("https://img.wangmoyu.com/a.png", policy()) { url, _ ->
            hits += url
            if (hits.size == 1) {
                Upstream(302, "http://node7-27.cdn.dfyun.com.cn/f/a.png", "", ByteArray(0))
            } else {
                Upstream(200, null, "image/png", png)
            }
        }
        assertEquals("https://node7-27.cdn.dfyun.com.cn/f/a.png", hits[1])

        var rejected = false
        try {
            pullImage("https://img.wangmoyu.com/a.png", policy()) { _, _ ->
                Upstream(302, "https://example.com/a.png", "", ByteArray(0))
            }
        } catch (_: ProxyRejected) {
            rejected = true
        }
        assertTrue(rejected)
    }

    @Test
    fun `名单外的地址不会发出请求`() {
        val calls = AtomicInteger()
        var rejected = false
        try {
            pullImage("https://example.com/a.png", policy()) { _, _ ->
                calls.incrementAndGet()
                Upstream(200, null, "image/png", png)
            }
        } catch (_: ProxyRejected) {
            rejected = true
        }
        assertTrue(rejected)
        assertEquals(0, calls.get())
        assertRejected { refererFor("https://user:pass@img.wangmoyu.com/a.png", policy()) }
        assertRejected { refererFor("https://127.0.0.1/a.png", policy()) }
    }

    @Test
    fun `本机代理只听回环地址并给白名单补 Referer`() {
        val calls = AtomicInteger()
        val proxy = LocalMediaProxy { url, referer ->
            calls.incrementAndGet()
            assertEquals("https://img.wangmoyu.com/a.png", url)
            assertEquals("https://jandan.net/", referer)
            Upstream(200, null, "image/png", png)
        }
        try {
            val base = proxy.setPolicy(listOf("img.wangmoyu.com", "*.cdn.dfyun.com.cn"), "https://jandan.net/")
            assertTrue(base.startsWith("http://127.0.0.1:"))

            val ok = get("$base/api/media?url=${enc("http://img.wangmoyu.com/a.png")}")
            assertEquals(200, ok.first)
            assertTrue(ok.second.startsWith("image/png"))
            assertEquals(0x89.toByte(), ok.third[0])
            assertEquals(1, calls.get())

            val blocked = get("$base/api/media?url=${enc("https://example.com/secret.png")}")
            assertEquals(400, blocked.first)
            assertEquals(1, calls.get())

            val missing = get("$base/other")
            assertEquals(404, missing.first)
        } finally {
            proxy.close()
        }
    }

    @Test
    fun `正文改写优先用本机代理且不带 token`() {
        val article = Article(
            id = 1,
            title = "t",
            content = """<img src="https://img.wangmoyu.com/a.jpg">""",
        )
        val hosts = listOf("img.wangmoyu.com")
        val local = buildArticleHtml(article, "http://10.0.0.8:8787", "secret", hosts, "http://127.0.0.1:54321")
        assertTrue(local.contains("http://127.0.0.1:54321/api/media?url="))
        assertFalse(local.contains("token="))
        assertFalse(local.contains("10.0.0.8"))

        val remote = buildArticleHtml(article, "http://10.0.0.8:8787", "secret", hosts, "")
        assertTrue(remote.contains("http://10.0.0.8:8787/api/media?url="))
        assertTrue(remote.contains("token="))
    }

    private fun assertRejected(block: () -> Unit) {
        try {
            block()
            throw AssertionError("应该拒绝")
        } catch (_: ProxyRejected) {
        }
    }

    private fun enc(s: String) = URLEncoder.encode(s, "UTF-8")

    private fun get(url: String): Triple<Int, String, ByteArray> {
        val conn = URL(url).openConnection() as HttpURLConnection
        conn.connectTimeout = 3000
        conn.readTimeout = 3000
        conn.instanceFollowRedirects = false
        val code = conn.responseCode
        val type = conn.contentType.orEmpty()
        val stream = if (code in 200..299) conn.inputStream else conn.errorStream
        val body = stream?.use { it.readBytes() } ?: ByteArray(0)
        conn.disconnect()
        return Triple(code, type, body)
    }
}
