package app.tauri.plugin

class Invoke {
    fun <T> parseArgs(clazz: Class<T>): T {
        return clazz.getDeclaredConstructor().newInstance()
    }

    fun resolve() {}

    fun resolve(value: Any?) {
        @Suppress("UNUSED_VARIABLE")
        val ignored = value
    }

    fun reject(message: String) {
        @Suppress("UNUSED_VARIABLE")
        val ignored = message
    }

    fun reject(message: String, throwable: Throwable) {
        @Suppress("UNUSED_VARIABLE")
        val ignoredMessage = message
        @Suppress("UNUSED_VARIABLE")
        val ignoredThrowable = throwable
    }
}
