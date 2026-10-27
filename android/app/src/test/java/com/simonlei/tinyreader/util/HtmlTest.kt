package com.simonlei.tinyreader.util

import com.simonlei.tinyreader.data.Article
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class HtmlTest {

    @Test
    fun `图片补上懒加载与异步解码`() {
        assertEquals(
            """<img src="a.jpg" loading="lazy" decoding="async">""",
            decorateImages("""<img src="a.jpg">"""),
        )
        // 自闭合写法要插在斜杠前面
        assertEquals(
            """<img src="a.jpg" loading="lazy" decoding="async"/>""",
            decorateImages("""<img src="a.jpg"/>"""),
        )
        assertEquals("""<p>没有图片</p>""", decorateImages("""<p>没有图片</p>"""))
    }

    @Test
    fun `源里已经写了 loading 或 decoding 时不覆盖`() {
        assertEquals(
            """<img loading="eager" src="x" decoding="async">""",
            decorateImages("""<img loading="eager" src="x">"""),
        )
        assertEquals(
            """<img decoding="sync" src="z" loading="lazy">""",
            decorateImages("""<img decoding="sync" src="z">"""),
        )
        // data-loading 这类自定义属性不算
        assertTrue(decorateImages("""<img data-loading="x" src="y">""").contains("""loading="lazy""""))
    }

    @Test
    fun `正文渲染出来的图片带懒加载`() {
        val html = buildArticleHtml(
            Article(id = 1, title = "t", content = """<img src="https://example.com/a.jpg">"""),
        )
        assertTrue(
            html.contains("""<img src="https://example.com/a.jpg" loading="lazy" decoding="async">"""),
        )
    }
}
