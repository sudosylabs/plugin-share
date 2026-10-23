package plugin.vnidrop.share

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ShareIntentPayloadUnitTest {
    @Test
    fun combinesTextAndUrlIntoBodyWithoutTitleFallback() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Read this article",
                url = "https://example.com/article",
            )
        )

        assertNull(payload.title)
        assertEquals("Read this article\nhttps://example.com/article", payload.body)
    }

    @Test
    fun explicitTitleIsUsedAsTitleOnly() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Description",
                title = "Article title",
                url = "https://example.com/article",
            )
        )

        assertEquals("Article title", payload.title)
        assertEquals("Description\nhttps://example.com/article", payload.body)
    }

    @Test
    fun textOnlyPayloadUsesTextAsBody() {
        val payload = ShareIntentPayload.from(options(text = "Hello"))

        assertNull(payload.title)
        assertEquals("Hello", payload.body)
    }

    @Test
    fun urlOnlyPayloadUsesUrlAsBody() {
        val payload = ShareIntentPayload.from(options(url = "https://example.com"))

        assertNull(payload.title)
        assertEquals("https://example.com", payload.body)
    }

    @Test
    fun emptyValuesAreTreatedAsAbsent() {
        val payload = ShareIntentPayload.from(
            options(
                text = "Hello",
                title = "",
                url = "",
            )
        )

        assertNull(payload.title)
        assertEquals("Hello", payload.body)

        val emptyText = ShareIntentPayload.from(options(text = "", url = "https://example.com"))
        assertEquals("https://example.com", emptyText.body)
    }

    @Test
    fun emptyPayloadHasNoTitleOrBody() {
        val payload = ShareIntentPayload.from(options())

        assertNull(payload.title)
        assertNull(payload.body)
    }

    @Test
    fun explicitSubjectIsPreserved() {
        val payload = ShareIntentPayload.from(
            options(
                title = "Chooser title",
                subject = "report.pdf",
            )
        )

        assertEquals("Chooser title", payload.title)
        assertEquals("report.pdf", payload.subject)
        assertEquals("report.pdf", payload.intentSubject())
    }

    @Test
    fun intentSubjectFallsBackToTitleWhenSubjectAbsent() {
        val payload = ShareIntentPayload.from(options(title = "Mail subject"))

        assertEquals("Mail subject", payload.intentSubject())
    }

    @Test
    fun emptySubjectIsTreatedAsAbsent() {
        val payload = ShareIntentPayload.from(
            options(
                title = "Chooser title",
                subject = "",
            )
        )

        assertNull(payload.subject)
        assertEquals("Chooser title", payload.intentSubject())
    }

    private fun options(
        text: String? = null,
        title: String? = null,
        subject: String? = null,
        url: String? = null,
    ): ShareOptions {
        return ShareOptions().apply {
            this.text = text
            this.title = title
            this.subject = subject
            this.url = url
        }
    }
}
