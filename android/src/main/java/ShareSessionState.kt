package plugin.vnidrop.share

internal class ShareSessionState {
    var isInProgress: Boolean = false
        private set

    private var awaitingResume: Boolean = false

    fun start() {
        check(!isInProgress) { "Share session already in progress." }
        isInProgress = true
        awaitingResume = false
    }

    fun markPaused() {
        if (isInProgress) {
            awaitingResume = true
        }
    }

    fun completeFromActivityResult(): Boolean = completeIf(isInProgress)

    fun completeFromResume(): Boolean = completeIf(isInProgress && awaitingResume)

    fun reset() {
        isInProgress = false
        awaitingResume = false
    }

    private fun completeIf(shouldComplete: Boolean): Boolean {
        if (!shouldComplete) {
            return false
        }

        reset()
        return true
    }
}
