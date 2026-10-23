package plugin.vnidrop.share

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
        validateStringLength("subject", args.subject, MAX_TITLE_BYTES)
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
            if (!isValidFilePath(path)) {
                throw SecurityException("file path must be an absolute path.")
            }
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
        val normalized = stripBase64Whitespace(data)
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

    /** Removes the ASCII whitespace characters permitted inside Base64 data. */
    fun stripBase64Whitespace(data: String): String =
        data.filterNot { it == ' ' || it == '\t' || it == '\n' || it == '\r' || it == '\u000C' }

    /** File paths must be non-empty absolute POSIX paths; URIs and relative paths are rejected. */
    fun isValidFilePath(path: String): Boolean = path.startsWith("/")

    /**
     * Port of the shared (Rust) web URL check. Intentionally does not use
     * java.net.URI / android.net.Uri so every platform agrees on the same inputs.
     */
    fun isValidWebUrl(url: String): Boolean {
        if (url.any { isControlOrWhitespace(it) }) return false

        val separator = url.indexOf("://")
        if (separator < 0) return false
        val scheme = asciiLowercase(url.substring(0, separator))
        if (scheme != "http" && scheme != "https") return false

        val rest = url.substring(separator + 3)
        val authorityEnd = rest.indexOfFirst { it == '/' || it == '?' || it == '#' }
        val authority = if (authorityEnd < 0) rest else rest.substring(0, authorityEnd)
        val hostPort = authority.substringAfterLast('@')
        val closingBracket = hostPort.indexOf(']')
        val host = if (closingBracket >= 0) {
            hostPort.substring(0, closingBracket + 1)
        } else {
            hostPort.substringBefore(':')
        }

        return host.isNotEmpty() && host != "[]" && !host.startsWith(":")
    }

    @Throws(SecurityException::class)
    private fun validateWebUrl(url: String) {
        if (!isValidWebUrl(url)) {
            throw SecurityException("Only well-formed http:// and https:// URLs can be shared as URLs.")
        }
    }

    /** Unicode Cc (includes U+0080..U+009F) or Unicode White_Space. */
    private fun isControlOrWhitespace(c: Char): Boolean {
        if (Character.isISOControl(c)) return true
        return when (c) {
            ' ', '\u00A0', '\u1680', '\u2028', '\u2029', '\u202F', '\u205F', '\u3000' -> true
            in '\u2000'..'\u200A' -> true
            else -> false
        }
    }

    private fun asciiLowercase(value: String): String =
        String(CharArray(value.length) { i ->
            val c = value[i]
            if (c in 'A'..'Z') c + ('a' - 'A') else c
        })
}
