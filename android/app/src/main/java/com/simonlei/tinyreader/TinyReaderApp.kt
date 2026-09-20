package com.simonlei.tinyreader

import android.app.Application
import com.simonlei.tinyreader.data.SettingsStore

class TinyReaderApp : Application() {
    override fun onCreate() {
        super.onCreate()
        SettingsStore.init(this)
    }
}
