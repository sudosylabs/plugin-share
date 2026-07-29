package plugin.vnidrop.share

internal data class ShareIntentPayload(
    val title: String?,
    val body: String?,
) {
    companion object {
        fun from(options: ShareOptions): ShareIntentPayload {
            val text = options.text.nonEmptyOrNull()
            val title = options.title.nonEmptyOrNull()
            val url = options.url.nonEmptyOrNull()

            return ShareIntentPayload(
                title = title ?: text,
                body = url ?: text,
            )
        }

        private fun String?.nonEmptyOrNull(): String? = this?.takeIf { it.isNotEmpty() }
    }
}
