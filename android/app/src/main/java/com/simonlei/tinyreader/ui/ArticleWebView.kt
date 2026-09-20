package com.simonlei.tinyreader.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color as AndroidColor
import android.view.MotionEvent
import android.view.ViewConfiguration
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import kotlin.math.abs

/**
 * 正文 WebView。
 *
 * 为什么要自定义：WebView 会吞掉所有触摸事件，直接把它放进 Compose 的
 * HorizontalPager / 手势容器里，左右滑动是收不到的。这里自己识别横向拖动，
 * 一旦判定为横向手势就给 WebView 发一个 ACTION_CANCEL 并接管事件，
 * 从而做到「竖向滚动交给 WebView，横向滑动用来切换上下篇」。
 */
@SuppressLint("ViewConstructor", "SetJavaScriptEnabled")
class ArticleWebView(context: Context) : WebView(context) {

    /** 向左滑（内容左移）= 下一篇 */
    var onSwipeNext: (() -> Unit)? = null

    /** 向右滑 = 上一篇 */
    var onSwipePrev: (() -> Unit)? = null

    /** 点击正文里的链接 */
    var onLinkClick: ((String) -> Unit)? = null

    private val touchSlop = ViewConfiguration.get(context).scaledTouchSlop

    /** 判定为横向手势所需的最小位移 */
    private val horizontalThreshold = touchSlop * 3

    private var downX = 0f
    private var downY = 0f
    private var tracking = false
    private var triggered = false

    init {
        setBackgroundColor(AndroidColor.parseColor("#14161A"))
        isVerticalScrollBarEnabled = true
        isHorizontalScrollBarEnabled = false
        overScrollMode = OVER_SCROLL_NEVER

        settings.apply {
            // 正文只是静态 HTML，关掉 JS 更安全（桌面端同样只是 v-html 渲染）
            javaScriptEnabled = false
            loadsImagesAutomatically = true
            blockNetworkImage = false
            useWideViewPort = true
            loadWithOverviewMode = true
            builtInZoomControls = true
            displayZoomControls = false
            domStorageEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            // 允许 https 页面里的 http 图片，否则局域网/老站点的图会挂
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
        }

        webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(
                view: WebView?,
                request: WebResourceRequest?,
            ): Boolean {
                val url = request?.url?.toString() ?: return false
                // 站内锚点交给 WebView 自己处理，其余一律走系统浏览器
                if (url.startsWith("#")) return false
                onLinkClick?.invoke(url)
                return true
            }
        }
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        when (event.actionMasked) {
            MotionEvent.ACTION_DOWN -> {
                downX = event.x
                downY = event.y
                tracking = true
                triggered = false
            }

            MotionEvent.ACTION_MOVE -> {
                if (tracking && !triggered) {
                    val dx = event.x - downX
                    val dy = event.y - downY
                    // 横向位移足够大，且明显比纵向大，才认定是翻页手势
                    if (abs(dx) > horizontalThreshold && abs(dx) > abs(dy) * 2f) {
                        triggered = true
                        cancelWebViewGesture(event)
                        if (dx < 0) onSwipeNext?.invoke() else onSwipePrev?.invoke()
                        return true
                    }
                }
            }

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                tracking = false
            }
        }

        if (triggered) return true
        return super.onTouchEvent(event)
    }

    /** 让 WebView 结束自己的手势跟踪，避免残留的滚动/长按状态 */
    private fun cancelWebViewGesture(event: MotionEvent) {
        val cancel = MotionEvent.obtain(event)
        cancel.action = MotionEvent.ACTION_CANCEL
        super.onTouchEvent(cancel)
        cancel.recycle()
    }
}
