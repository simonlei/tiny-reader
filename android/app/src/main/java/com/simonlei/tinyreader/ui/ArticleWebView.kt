package com.simonlei.tinyreader.ui

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Color as AndroidColor
import android.view.MotionEvent
import android.view.VelocityTracker
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

    private val density = context.resources.displayMetrics.density

    /**
     * 判定为翻页的最小横向位移（dp）。
     * 早先用 touchSlop * 3（约 24dp），读长文时手指稍微飘一点就翻页了；
     * 这里放宽到接近 ViewPager 的翻页距离，并叠加下面的速度和方向条件。
     */
    private val swipeDistance = 64f * density

    /** 纵向漂移上限（dp）：超过就认定用户在竖着滚正文，不翻页 */
    private val maxVerticalDrift = 40f * density

    /** 最小横向速度（dp/s）：慢慢横拖不算翻页 */
    private val minSwipeVelocity = 200f * density

    private var velocityTracker: VelocityTracker? = null

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
            // 页面模板自带 width=device-width。关掉宽视口后布局宽度就等于屏幕宽度，
            // WebView 不再对整页做一次缩放光栅：正文里若混进了源站的 viewport meta
            // （HTML 解析器会把它塞进 head），开启时会被拉成上千 px 再缩小，图片一多滚动就很卡。
            useWideViewPort = false
            loadWithOverviewMode = false
            builtInZoomControls = true
            displayZoomControls = false
            domStorageEnabled = false
            allowFileAccess = false
            allowContentAccess = false
            // 允许 https 页面里的 http 图片，否则局域网/老站点的图会挂
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
            // 离屏预光栅（minSdk 26 > 23）：滚动时提前把相邻区域画好，减少多图长文的滚动空白与卡顿
            offscreenPreRaster = true
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
                velocityTracker?.recycle()
                velocityTracker = VelocityTracker.obtain().also { it.addMovement(event) }
            }

            MotionEvent.ACTION_MOVE -> {
                velocityTracker?.addMovement(event)
                if (tracking && !triggered) {
                    val dx = event.x - downX
                    val dy = event.y - downY
                    // 三个条件同时成立才算翻页：横向走够了、基本没竖着走、横向速度占优。
                    // 只比位移时，「慢慢竖着滚 + 手指横飘」也会被判成翻页，这是误触的主因。
                    if (abs(dx) >= swipeDistance && abs(dy) <= maxVerticalDrift && isHorizontalSwipe()) {
                        triggered = true
                        cancelWebViewGesture(event)
                        if (dx < 0) onSwipeNext?.invoke() else onSwipePrev?.invoke()
                        return true
                    }
                }
            }

            // 第二根手指落下 = 双指缩放，不再当翻页处理
            MotionEvent.ACTION_POINTER_DOWN -> tracking = false

            MotionEvent.ACTION_UP, MotionEvent.ACTION_CANCEL -> {
                tracking = false
                velocityTracker?.recycle()
                velocityTracker = null
            }
        }

        if (triggered) return true
        return super.onTouchEvent(event)
    }

    /** 横向速度够快且压过纵向速度：竖着读文章时纵向速度会明显更大 */
    private fun isHorizontalSwipe(): Boolean {
        val tracker = velocityTracker ?: return false
        tracker.computeCurrentVelocity(1000)
        val vx = abs(tracker.xVelocity)
        val vy = abs(tracker.yVelocity)
        return vx >= minSwipeVelocity && vx > vy
    }

    /** 让 WebView 结束自己的手势跟踪，避免残留的滚动/长按状态 */
    private fun cancelWebViewGesture(event: MotionEvent) {
        val cancel = MotionEvent.obtain(event)
        cancel.action = MotionEvent.ACTION_CANCEL
        super.onTouchEvent(cancel)
        cancel.recycle()
    }
}
