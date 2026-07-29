package plugin.vnidrop.share

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ShareIntentPayloadUnitTest {
    @Test
    fun usesUrlAsBodyAndTextAsFallbackTitle() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Read this article",
                url = "https://example.com/article",
            )
        )

        assertEquals("Read this article", payload.title)
        assertEquals("https://example.com/article", payload.body)
    }

    @Test
    fun explicitTitleTakesPrecedenceOverText() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Description",
                title = "Article title",
                url = "https://example.com/article",
            )
        )

        assertEquals("Article title", payload.title)
        assertEquals("https://example.com/article", payload.body)
    }

    @Test
    fun textOnlyPayloadUsesTextAsTitleAndBody() {
        val payload = ShareIntentPayload.from(options(text = "Hello"))

        assertEquals("Hello", payload.title)
        assertEquals("Hello", payload.body)
    }

    @Test
    fun emptyValuesDoNotHideShareableText() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Hello",
                title = "",
                url = "",
            )
        )

        assertEquals("Hello", payload.title)
        assertEquals("Hello", payload.body)
    }

    @Test
    fun emptyPayloadHasNoTitleOrBody() {
        val payload = ShareIntentPayload.from(options())

        assertNull(payload.title)
        assertNull(payload.body)
    }

    private fun options(
        text: String? = null,
        title: String? = null,
        url: String? = null,
    ): ShareOptions {
        return ShareOptions().apply {
            this.text = text
            this.title = title
            this.url = url
        }
    }
}
