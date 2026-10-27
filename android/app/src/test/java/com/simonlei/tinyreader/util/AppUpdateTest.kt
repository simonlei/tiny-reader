package com.simonlei.tinyreader.util

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream

class AppUpdateTest {
    private val sha = "ab".repeat(32)
    private val apkSha = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"

    @Test
    fun `清单地址和安装包地址钉在发布通道上`() {
        assertEquals(
            "https://tiny-reader-1251477527.cos.ap-guangzhou.myqcloud.com/android.json",
            UPDATE_MANIFEST_URL,
        )
        assertEquals(
            "https://tiny-reader-1251477527.cos.ap-guangzhou.myqcloud.com/tiny-reader-android-v0.2.0.bin",
            expectedApkUrl("0.2.0"),
        )
    }

    @Test
    fun `远程版本更高时给出可安装的更新`() {
        val decision = decideUpdate(manifest(), "0.1.0")
        assertEquals(
            UpdateDecision.Available(
                UpdateOffer(
                    version = "0.2.0",
                    notes = "修复阅读页",
                    url = expectedApkUrl("0.2.0"),
                    sha256 = sha,
                ),
            ),
            decision,
        )
    }

    @Test
    fun `版本号按数字比较并接受 v 前缀`() {
        val newer = decideUpdate(manifest(version = "v0.1.10"), "0.1.9")
        assertTrue(newer is UpdateDecision.Available)
        assertEquals("0.1.10", (newer as UpdateDecision.Available).offer.version)

        assertEquals(UpdateDecision.UpToDate, decideUpdate(manifest(version = "0.1.0"), "0.1.0"))
        assertEquals(UpdateDecision.UpToDate, decideUpdate(manifest(version = "0.1.0"), "0.2.0"))
        assertEquals(UpdateDecision.UpToDate, decideUpdate(manifest(version = "v0.9.0"), "0.9.0"))
    }

    @Test
    fun `说明可以省略，过长时截断`() {
        val omitted = decideUpdate(
            """{"version":"0.2.0","url":"${expectedApkUrl("0.2.0")}","sha256":"$sha"}""",
            "0.1.0",
        ) as UpdateDecision.Available
        assertEquals("", omitted.offer.notes)

        val long = decideUpdate(manifest(notes = "a".repeat(4001)), "0.1.0") as UpdateDecision.Available
        assertEquals(4000, long.offer.notes.length)
    }

    @Test
    fun `sha256 接受前缀和大小写`() {
        val decision = decideUpdate(
            manifest(sha256 = "SHA256:" + "AB".repeat(32)),
            "0.1.0",
        ) as UpdateDecision.Available
        assertEquals(sha, decision.offer.sha256)
    }

    @Test
    fun `不可信的地址或残缺清单会被拒绝`() {
        assertRejected("不可信") {
            decideUpdate(manifest(url = "https://example.com/tiny-reader-android-v0.2.0.bin"), "0.1.0")
        }
        assertRejected("不可信") {
            decideUpdate(manifest(url = expectedApkUrl("0.3.0")), "0.1.0")
        }
        assertRejected("不可信") {
            decideUpdate(
                manifest(url = expectedApkUrl("0.2.0") + "?dl=1"),
                "0.1.0",
            )
        }
        assertRejected("版本号") { decideUpdate(manifest(version = "0.2"), "0.1.0") }
        assertRejected("版本号") { decideUpdate(manifest(), "dev") }
        assertRejected("sha256") { decideUpdate(manifest(sha256 = "abc"), "0.1.0") }
        assertRejected("无法解析") { decideUpdate("{", "0.1.0") }
    }

    @Test
    fun `下载时校验 sha256 并报告进度`() {
        val progress = mutableListOf<Long>()
        val out = ByteArrayOutputStream()
        copyAndVerify(
            input = ByteArrayInputStream("abc".toByteArray()),
            output = out,
            expectedSha256 = "SHA256:$apkSha".uppercase(),
            contentLength = 3L,
            onProgress = { got, total ->
                assertEquals(3L, total)
                progress += got
            },
        )
        assertEquals(listOf(3L), progress)
        assertEquals("abc", out.toString(Charsets.UTF_8.name()))
    }

    @Test
    fun `校验失败、不完整、过大或取消时拒绝安装`() {
        assertRejected("校验失败") {
            copyAndVerify(
                ByteArrayInputStream("abc".toByteArray()),
                ByteArrayOutputStream(),
                sha,
                3L,
            )
        }
        assertRejected("不完整") {
            copyAndVerify(
                ByteArrayInputStream("abc".toByteArray()),
                ByteArrayOutputStream(),
                apkSha,
                4L,
            )
        }
        assertRejected("过大") {
            copyAndVerify(
                ByteArrayInputStream("abc".toByteArray()),
                ByteArrayOutputStream(),
                apkSha,
                contentLength = 11L,
                maxBytes = 10L,
            )
        }
        val overflow = ByteArrayOutputStream()
        assertRejected("过大") {
            copyAndVerify(
                ByteArrayInputStream(ByteArray(11) { 'a'.code.toByte() }),
                overflow,
                apkSha,
                contentLength = -1L,
                maxBytes = 10L,
            )
        }
        assertEquals(0, overflow.size())
        assertRejected("空的") {
            copyAndVerify(ByteArrayInputStream(ByteArray(0)), ByteArrayOutputStream(), apkSha, 0L)
        }
        assertRejected("取消") {
            copyAndVerify(
                ByteArrayInputStream("abc".toByteArray()),
                ByteArrayOutputStream(),
                apkSha,
                3L,
                shouldContinue = { false },
            )
        }
    }

    private fun manifest(
        version: String = "0.2.0",
        notes: String = "修复阅读页",
        url: String = expectedApkUrl(version.removePrefix("v")),
        sha256: String = sha,
    ): String =
        """{"version":"$version","notes":"$notes","url":"$url","sha256":"$sha256"}"""

    private fun assertRejected(snippet: String, block: () -> Unit) {
        val err = runCatching(block).exceptionOrNull()
        assertTrue(err is UpdateRejected)
        assertTrue(err!!.message.orEmpty().contains(snippet))
    }
}
