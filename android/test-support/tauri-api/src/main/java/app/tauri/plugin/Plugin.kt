package app.tauri.plugin

import android.app.Activity

open class Plugin(activity: Activity) {
    @Suppress("UNUSED_VARIABLE")
    private val ignoredActivity = activity

    open fun onPause() {}

    open fun onResume() {}
}
