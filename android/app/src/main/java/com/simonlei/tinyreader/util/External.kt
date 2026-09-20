package com.simonlei.tinyreader.util

import android.content.Context
import android.content.Intent
import android.net.Uri

/**
 * 在系统浏览器里打开链接，对应桌面端 `src/lib/tauri.ts` 的 openExternal
 * （Tauri 侧同样只允许 http / https）。
 */
fun openExternal(context: Context, url: String?) {
    val target = url?.trim().orEmpty()
    if (!target.startsWith("http://") && !target.startsWith("https://")) return
    runCatching {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(target)).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        context.startActivity(intent)
    }
}
