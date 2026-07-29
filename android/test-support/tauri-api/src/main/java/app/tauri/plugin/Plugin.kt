package app.tauri.plugin

import android.app.Activity
import android.content.Intent

open class Plugin(activity: Activity) {
    @Suppress("UNUSED_VARIABLE")
    private val ignoredActivity = activity

    open fun onPause() {}

    open fun onResume() {}

    fun startActivityForResult(invoke: Invoke, intent: Intent, callback: String) {
        @Suppress("UNUSED_VARIABLE")
        val ignoredInvoke = invoke
        @Suppress("UNUSED_VARIABLE")
        val ignoredIntent = intent
        @Suppress("UNUSED_VARIABLE")
        val ignoredCallback = callback
    }
}
