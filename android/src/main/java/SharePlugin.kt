package plugin.vnidrop.share

import android.app.Activity
import android.content.ClipData
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Base64
import java.io.File
import java.io.FileOutputStream
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.IOException
import java.net.URLConnection
import java.util.UUID

@InvokeArg
class Anchor {
    var x: Double? = null
    var y: Double? = null
    var width: Double? = null
    var height: Double? = null
}

@InvokeArg
class SharedFile {
    lateinit var data: String
    lateinit var name: String
    lateinit var mimeType: String
}

@InvokeArg
class ShareOptions {
    var text: String? = null
    var title: String? = null
    var url: String? = null
    var files: List<SharedFile>? = null
    /** A list of local file paths to share directly from disk, preserving the original filename. */
    var filePaths: List<String>? = null
    var anchor: Anchor? = null
}

@TauriPlugin
class SharePlugin(private val activity: Activity): Plugin(activity) {
    private var pendingShareInvoke: Invoke? = null
    private var pendingCleanupFiles: List<File> = emptyList()
    private val shareSession = ShareSessionState()
    private val cleanupHandler = Handler(Looper.getMainLooper())

    companion object {
        private const val CLEANUP_DELAY_MS = 5 * 60 * 1000L
    }

    @Command
    fun canShare(invoke: Invoke) {
        // The native share sheet is almost always available on Android.
        // This command primarily serves to confirm the plugin is installed and responsive.
        val result = JSObject()
        result.put("value", true)
        invoke.resolve(result)
    }

    @Command
    fun share(invoke: Invoke) {
        if (shareSession.isInProgress) {
            invoke.reject("Share already in progress.")
            return
        }

        val filesForShare = ArrayList<File>()
        try {
            val args = invoke.parseArgs(ShareOptions::class.java)
            ShareValidation.validateShareOptions(args)
            val fileUris = ArrayList<Uri>()
            val mimeTypes = ArrayList<String>()
            val authority = "${activity.packageName}.fileprovider"

            args.files?.let {
                if (it.isNotEmpty()) {
                    for (file in it) {
                        val decodedBytes = Base64.decode(file.data, Base64.DEFAULT)
                        ShareValidation.validateDecodedFileSize(file, decodedBytes.size)
                        val tempFile = createSafeFile(file.name)
                        FileOutputStream(tempFile).use { outputStream ->
                            outputStream.write(decodedBytes)
                        }
                        filesForShare.add(tempFile)

                        fileUris.add(
                            FileProvider.getUriForFile(activity, authority, tempFile)
                        )
                        mimeTypes.add(file.mimeType)
                    }
                }
            }

            args.filePaths?.let {
                if (it.isNotEmpty()) {
                    for (path in it) {
                        val file = File(path)
                        if (!file.exists() || !file.isFile) {
                            throw SecurityException("File does not exist or is not a regular file: $path")
                        }

                        fileUris.add(
                            FileProvider.getUriForFile(activity, authority, file)
                        )
                        mimeTypes.add(
                            URLConnection.guessContentTypeFromName(file.name)
                                ?: "application/octet-stream"
                        )
                    }
                }
            }

            var determinedMimeType = "text/plain"
            if (mimeTypes.isNotEmpty()) {
                determinedMimeType = determineMimeType(mimeTypes)
            }

            if (fileUris.isEmpty() && args.text.isNullOrEmpty() && args.url.isNullOrEmpty()) {
                invoke.reject("No content provided to share.")
                return
            }

            val shareIntent = Intent()
            if (fileUris.isNotEmpty()) {
                shareIntent.action = if (fileUris.size > 1) Intent.ACTION_SEND_MULTIPLE else Intent.ACTION_SEND
                if (fileUris.size > 1) {
                    shareIntent.putParcelableArrayListExtra(Intent.EXTRA_STREAM, fileUris)
                } else {
                    shareIntent.putExtra(Intent.EXTRA_STREAM, fileUris[0])
                }

                shareIntent.clipData = createShareClipData(fileUris)
            } else {
                shareIntent.action = Intent.ACTION_SEND
            }

            shareIntent.type = determinedMimeType

            val payload = ShareIntentPayload.from(args)
            // Only include text extras for text-only shares. When a file is attached,
            // EXTRA_TITLE / EXTRA_SUBJECT can be misused by target apps (e.g. Google Drive)
            // as the filename, and EXTRA_TEXT can shadow the file stream.
            if (fileUris.isEmpty()) {
                payload.body?.let {
                    shareIntent.putExtra(Intent.EXTRA_TEXT, it)
                }
                payload.title?.let {
                    shareIntent.putExtra(Intent.EXTRA_TITLE, it)
                    shareIntent.putExtra(Intent.EXTRA_SUBJECT, it)
                }
            }

            shareIntent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            val chooser = Intent.createChooser(shareIntent, payload.title)

            pendingShareInvoke = invoke
            pendingCleanupFiles = filesForShare
            shareSession.start()
            startActivityForResult(invoke, chooser, "shareResult")
        } catch (e: Exception) {
            cleanupFiles(filesForShare)
            resetPendingShare()
            invoke.reject("Failed to share content: ${e.message}", e)
        }
    }

    override fun onPause() {
        super.onPause()
        shareSession.markPaused()
    }

    override fun onResume() {
        super.onResume()
        if (shareSession.completeFromResume()) {
            resolvePendingShare()
        }
    }

    @ActivityCallback
    fun shareResult(invoke: Invoke, result: ActivityResult) {
        if (shareSession.completeFromActivityResult()) {
            resolvePendingShare(invoke)
        }
    }

    /**
     * Deletes all temporary files created by this plugin in its dedicated share directory.
     * This should be called by the developer when the files are no longer needed.
     */
    @Command
    fun cleanup(invoke: Invoke) {
        try {
            val shareDir = getSafeShareDir()
            if (shareDir.exists() && shareDir.isDirectory) {
                if (!shareDir.deleteRecursively()) {
                    invoke.reject("Failed to delete all temporary share files.")
                    return
                }
            }
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("Error during cleanup: ${e.message}", e)
        }
    }

    private fun determineMimeType(mimeTypes: List<String>): String {
        if (mimeTypes.isEmpty()) return "*/*"
        val firstMimeType = mimeTypes.first()
        val firstGeneralType = firstMimeType.substringBefore('/')

        val allSame = mimeTypes.all { it == firstMimeType }
        if (allSame) return firstMimeType

        val allSameGeneral = mimeTypes.all { it.startsWith(firstGeneralType) }
        if (allSameGeneral) return "$firstGeneralType/*"

        return "*/*"
    }

    private fun createShareClipData(fileUris: List<Uri>): ClipData {
        val clipData = ClipData.newUri(activity.contentResolver, "Shared Files", fileUris.first())
        fileUris.drop(1).forEach { uri ->
            clipData.addItem(ClipData.Item(uri))
        }
        return clipData
    }

    private fun resolvePendingShare(invoke: Invoke? = pendingShareInvoke) {
        val filesToClean = pendingCleanupFiles
        pendingShareInvoke = null
        pendingCleanupFiles = emptyList()
        scheduleCleanup(filesToClean)
        invoke?.resolve()
    }

    private fun resetPendingShare() {
        pendingShareInvoke = null
        pendingCleanupFiles = emptyList()
        shareSession.reset()
    }

    private fun scheduleCleanup(files: List<File>) {
        if (files.isEmpty()) return

        cleanupHandler.postDelayed({
            cleanupFiles(files)
        }, CLEANUP_DELAY_MS)
    }

    private fun cleanupFiles(files: List<File>) {
        files.forEach { file ->
            try {
                if (file.exists()) {
                    file.delete()
                }
            } catch (_: Exception) {
                // Best-effort cleanup only.
            }
        }
    }

    /**
     * Returns the dedicated, secure directory for storing temporary share files.
     * Creates it if it doesn't exist.
     */
    private fun getSafeShareDir(): File {
        val shareDir = File(activity.cacheDir, "shares")
        if (!shareDir.exists()) {
            shareDir.mkdirs()
        }
        return shareDir
    }

    /**
     * Creates a safe File object within the dedicated share directory.
     * It sanitizes the filename and performs path traversal checks.
     */
    @Throws(IOException::class, SecurityException::class)
    private fun createSafeFile(untrustedFileName: String): File {
        val safeDir = getSafeShareDir()
        val safeDirCanonicalPath = safeDir.canonicalPath

        // Sanitize the filename to prevent malicious characters.
        // A robust approach is to allow only a whitelist of characters.
        // Here, we also add a UUID to prevent name collisions.
        val sanitizedBaseName = untrustedFileName.replace(Regex("[^a-zA-Z0-9._-]"), "")
        if (sanitizedBaseName.isEmpty()) {
            throw SecurityException("Invalid filename: sanitized name is empty.")
        }

        val finalFileName = "${UUID.randomUUID()}-${sanitizedBaseName}"

        val intendedFile = File(safeDir, finalFileName)

        // CRITICAL: Path Traversal Check
        // Ensure the final resolved path is still inside our secure directory.
        if (!intendedFile.canonicalPath.startsWith(safeDirCanonicalPath + File.separator)) {
            throw SecurityException("Path Traversal Attack Detected. Malicious filename: '$untrustedFileName'")
        }

        return intendedFile
    }
}
