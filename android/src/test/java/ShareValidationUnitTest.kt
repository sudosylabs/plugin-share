package plugin.vnidrop.share

import org.junit.Assert.assertEquals
import org.junit.Test

class ShareValidationUnitTest {
    @Test
    fun acceptsHttpAndHttpsUrls() {
        ShareValidation.validateShareOptions(options(url = "https://example.com"))
        ShareValidation.validateShareOptions(options(url = "http://example.com"))
    }

    @Test
    fun rejectsNonWebUrlSchemes() {
        listOf(
            "file:///data/user/0/app/secret.db",
            "content://provider/item",
            "custom://value",
            "https:///missing-host",
            " https://example.com",
            "https://example.com\nhttps://evil.example",
        ).forEach {
            assertThrowsSecurity {
                ShareValidation.validateShareOptions(options(url = it))
            }
        }
    }

    @Test
    fun estimatesBase64DecodedSize() {
        assertEquals(5, ShareValidation.estimateBase64DecodedSize("aGVsbG8="))
        assertEquals(5, ShareValidation.estimateBase64DecodedSize("aGVs\nbG8="))
    }

    @Test
    fun rejectsInvalidBase64Shape() {
        assertThrowsSecurity {
            ShareValidation.estimateBase64DecodedSize("abc")
        }
        assertThrowsSecurity {
            ShareValidation.estimateBase64DecodedSize("abcd===")
        }
    }

    @Test
    fun rejectsTooManyFilesAndOversizedText() {
        val files = (0..ShareValidation.MAX_FILES).map {
            file("report-$it.txt", "aGVsbG8=")
        }
        assertThrowsSecurity {
            ShareValidation.validateShareOptions(options(files = files))
        }

        val paths = (0..ShareValidation.MAX_FILES).map {
            "/data/file-$it"
        }
        assertThrowsSecurity {
            ShareValidation.validateShareOptions(options(filePaths = paths))
        }

        assertThrowsSecurity {
            ShareValidation.validateShareOptions(options(text = "a".repeat(ShareValidation.MAX_TEXT_BYTES + 1)))
        }

        assertThrowsSecurity {
            ShareValidation.validateShareOptions(
                options(files = listOf(file("a".repeat(ShareValidation.MAX_FILE_NAME_BYTES + 1), "aGVsbG8=")))
            )
        }

        assertThrowsSecurity {
            ShareValidation.validateShareOptions(
                options(filePaths = listOf("/path/".repeat(ShareValidation.MAX_FILE_PATH_BYTES)))
            )
        }
    }

    private fun options(
        text: String? = null,
        title: String? = null,
        url: String? = null,
        files: List<SharedFile>? = null,
        filePaths: List<String>? = null,
    ): ShareOptions {
        return ShareOptions().apply {
            this.text = text
            this.title = title
            this.url = url
            this.files = files
            this.filePaths = filePaths
        }
    }

    private fun file(name: String, data: String): SharedFile {
        return SharedFile().apply {
            this.name = name
            this.data = data
            this.mimeType = "text/plain"
        }
    }

    private fun assertThrowsSecurity(block: () -> Unit) {
        try {
            block()
        } catch (_: SecurityException) {
            return
        }
        throw AssertionError("Expected SecurityException")
    }
}
