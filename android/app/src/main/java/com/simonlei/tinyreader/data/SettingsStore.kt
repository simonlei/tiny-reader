package com.simonlei.tinyreader.data

import android.content.Context
import android.content.SharedPreferences

/**
 * 本机偏好（服务端地址 + token），对应桌面端 `src/stores/settings.ts` 的 localStorage。
 * 阅读状态全部在服务端，这里只存连接信息。
 */
object SettingsStore {

    private const val PREFS = "tiny-reader.settings.v1"
    private const val KEY_SERVER_URL = "serverUrl"
    private const val KEY_TOKEN = "token"

    /** 与桌面端默认值保持一致；手机上通常需要改成服务端所在机器的局域网 IP */
    const val DEFAULT_URL = "http://127.0.0.1:8787"

    private lateinit var prefs: SharedPreferences

    fun init(context: Context) {
        prefs = context.applicationContext.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
    }

    data class Snapshot(val serverUrl: String, val token: String) {
        val needsSetup: Boolean get() = serverUrl.isBlank() || token.isBlank()
    }

    fun load(): Snapshot = Snapshot(
        serverUrl = prefs.getString(KEY_SERVER_URL, DEFAULT_URL).orEmpty(),
        token = prefs.getString(KEY_TOKEN, "").orEmpty(),
    )

    fun save(serverUrl: String, token: String) {
        prefs.edit()
            .putString(KEY_SERVER_URL, serverUrl.trim().trimEnd('/'))
            .putString(KEY_TOKEN, token.trim())
            .apply()
    }
}
