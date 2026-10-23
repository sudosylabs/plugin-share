package plugin.vnidrop.share

internal data class ShareIntentPayload(
    val title: String?,
    val subject: String?,
    val body: String?,
) {
    fun intentSubject(): String? = subject ?: title

    companion object {
        fun from(options: ShareOptions): ShareIntentPayload {
            val text = options.text.nonEmptyOrNull()
            val title = options.title.nonEmptyOrNull()
            val subject = options.subject.nonEmptyOrNull()
            val url = options.url.nonEmptyOrNull()

            val body = if (text != null && url != null) "$text\n$url" else text ?: url

            return ShareIntentPayload(
                title = title,
                subject = subject,
                body = body,
            )
        }

        private fun String?.nonEmptyOrNull(): String? = this?.takeIf { it.isNotEmpty() }
    }
}
