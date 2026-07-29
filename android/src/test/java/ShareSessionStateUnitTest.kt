package plugin.vnidrop.share

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ShareSessionStateUnitTest {
    @Test
    fun activityResultCompletesSessionAndAllowsAnotherShare() {
        val state = ShareSessionState()

        state.start()
        assertTrue(state.isInProgress)
        assertTrue(state.completeFromActivityResult())
        assertFalse(state.isInProgress)

        state.start()
        assertTrue(state.isInProgress)
    }

    @Test
    fun resumeRemainsAFallbackAfterPause() {
        val state = ShareSessionState()

        state.start()
        assertFalse(state.completeFromResume())

        state.markPaused()
        assertTrue(state.completeFromResume())
        assertFalse(state.isInProgress)
    }

    @Test
    fun completionOnlyHappensOnce() {
        val state = ShareSessionState()

        state.start()
        state.markPaused()
        assertTrue(state.completeFromActivityResult())
        assertFalse(state.completeFromResume())
        assertFalse(state.completeFromActivityResult())
    }
}
