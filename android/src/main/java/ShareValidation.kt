package plugin.vnidrop.share

import java.net.URI

object ShareValidation {
    const val MAX_FILES = 16
    const val MAX_FILE_BYTES = 50 * 1024 * 1024
    const val MAX_TOTAL_FILE_BYTES = 100 * 1024 * 1024
    const val MAX_TEXT_BYTES = 64 * 1024
    const val MAX_TITLE_BYTES = 1024
    const val MAX_URL_BYTES = 4096
    const val MAX_FILE_NAME_BYTES = 255
    const val MAX_MIME_TYPE_BYTES = 255
    const val MAX_FILE_PATH_BYTES = 4096

    @Throws(SecurityException::class)
    fun validateShareOptions(args: ShareOptions) {
        validateStringLength("text", args.text, MAX_TEXT_BYTES)
        validateStringLength("title", args.title, MAX_TITLE_BYTES)
        validateStringLength("url", args.url, MAX_URL_BYTES)

        args.url?.let {
            if (it.isNotEmpty()) {
                validateWebUrl(it)
            }
        }

        val files = args.files ?: emptyList()
        val filePaths = args.filePaths ?: emptyList()
        val fileCount = files.size + filePaths.size
        if (fileCount > MAX_FILES) {
            throw SecurityException("Too many files provided. Maximum is $MAX_FILES.")
        }

        var totalEstimatedBytes = 0L
        for (file in files) {
            validateStringLength("file name", file.name, MAX_FILE_NAME_BYTES)
            validateStringLength("mime type", file.mimeType, MAX_MIME_TYPE_BYTES)

            val estimatedBytes = estimateBase64DecodedSize(file.data)
            if (estimatedBytes > MAX_FILE_BYTES) {
                throw SecurityException("File '${file.name}' exceeds the maximum size of $MAX_FILE_BYTES bytes.")
            }
            totalEstimatedBytes += estimatedBytes
            if (totalEstimatedBytes > MAX_TOTAL_FILE_BYTES) {
                throw SecurityException("Total shared file size exceeds the maximum of $MAX_TOTAL_FILE_BYTES bytes.")
            }
        }

        for (path in filePaths) {
            validateStringLength("file path", path, MAX_FILE_PATH_BYTES)
        }
    }

    @Throws(SecurityException::class)
    fun validateDecodedFileSize(file: SharedFile, byteCount: Int) {
        if (byteCount > MAX_FILE_BYTES) {
            throw SecurityException("File '${file.name}' exceeds the maximum size of $MAX_FILE_BYTES bytes.")
        }
    }

    @Throws(SecurityException::class)
    fun estimateBase64DecodedSize(data: String): Long {
        val normalized = data.filterNot { it.isWhitespace() }
        if (normalized.isEmpty()) return 0
        if (normalized.length % 4 != 0) {
            throw SecurityException("Invalid Base64 data.")
        }

        val padding = normalized.takeLastWhile { it == '=' }.length
        if (padding > 2) {
            throw SecurityException("Invalid Base64 data.")
        }

        return (normalized.length / 4L) * 3L - padding
    }

    @Throws(SecurityException::class)
    private fun validateStringLength(field: String, value: String?, maxBytes: Int) {
        if (value != null && value.toByteArray(Charsets.UTF_8).size > maxBytes) {
            throw SecurityException("$field exceeds the maximum length of $maxBytes bytes.")
        }
    }

    @Throws(SecurityException::class)
    private fun validateWebUrl(url: String) {
        if (url.trim() != url || url.any { it.isWhitespace() || Character.isISOControl(it) }) {
            throw SecurityException("Only well-formed http:// and https:// URLs can be shared as URLs.")
        }

        val uri = try {
            URI(url)
        } catch (_: Exception) {
            null
        }
        val scheme = uri?.scheme?.lowercase()
        if (scheme != "http" && scheme != "https") {
            throw SecurityException("Only http:// and https:// URLs can be shared as URLs.")
        }
        if (uri.host.isNullOrEmpty()) {
            throw SecurityException("Only http:// and https:// URLs can be shared as URLs.")
        }
    }
}
