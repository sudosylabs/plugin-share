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
    var subject: String? = null
    var url: String? = null
    var files: List<SharedFile>? = null
    /** A list of local file paths to share directly from disk, preserving the original filename. */
    var filePaths: List<String>? = null
    var anchor: Anchor? = null
}

@TauriPlugin
class SharePlugin(private val activity: Activity): Plugin(activity) {
    // Tauri invokes commands on a background thread while lifecycle callbacks
    // (onPause/onResume) and activity results arrive on the main thread, so all
    // share state below is guarded by `lock`.
    private val lock = Any()
    private var isStaging = false
    private var pendingShareInvoke: Invoke? = null
    private var pendingCleanupFiles: List<File> = emptyList()
    private val shareSession = ShareSessionState()
    private val cleanupHandler = Handler(Looper.getMainLooper())

    companion object {
        private const val CLEANUP_DELAY_MS = 5 * 60 * 1000L
        private const val DEFAULT_MIME_TYPE = "application/octet-stream"
    }

    private class StagedFiles {
        val uris = ArrayList<Uri>()
        val mimeTypes = ArrayList<String>()
        /** Files created by the plugin that must be deleted once sharing finishes. */
        val ownedFiles = ArrayList<File>()
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
        synchronized(lock) {
            if (isStaging || shareSession.isInProgress) {
                invoke.reject("Share already in progress.")
                return
            }
            isStaging = true
        }

        val staged = StagedFiles()
        val chooser: Intent
        try {
            val args = invoke.parseArgs(ShareOptions::class.java)
            ShareValidation.validateShareOptions(args)
            // Decoding and copying run on the calling (non-main) thread.
            stageFiles(args, staged)

            val payload = ShareIntentPayload.from(args)
            if (staged.uris.isEmpty() && payload.body == null) {
                cleanupFiles(staged.ownedFiles)
                synchronized(lock) { isStaging = false }
                invoke.reject("No content provided to share.")
                return
            }

            chooser = buildChooserIntent(staged, payload)
        } catch (e: Exception) {
            cleanupFiles(staged.ownedFiles)
            synchronized(lock) { isStaging = false }
            invoke.reject("Failed to share content: ${e.message}", e)
            return
        }

        synchronized(lock) {
            isStaging = false
            pendingShareInvoke = invoke
            pendingCleanupFiles = staged.ownedFiles
            shareSession.start()
        }
        launchChooser(invoke, chooser)
    }

    override fun onPause() {
        super.onPause()
        synchronized(lock) {
            shareSession.markPaused()
        }
    }

    override fun onResume() {
        super.onResume()
        val completed = synchronized(lock) { shareSession.completeFromResume() }
        if (completed) {
            resolvePendingShare()
        }
    }

    @ActivityCallback
    fun shareResult(invoke: Invoke, result: ActivityResult) {
        val completed = synchronized(lock) { shareSession.completeFromActivityResult() }
        if (completed) {
            resolvePendingShare(invoke)
        }
    }

    /**
     * Deletes all temporary files created by this plugin in its dedicated share directory.
     * This should be called by the developer when the files are no longer needed.
     */
    @Command
    fun cleanup(invoke: Invoke) {
        synchronized(lock) {
            if (isStaging || shareSession.isInProgress) {
                invoke.reject("Cannot cleanup while sharing is in progress.")
                return
            }
            try {
                val shareDir = getSafeShareDir()
                if (shareDir.exists() && shareDir.isDirectory) {
                    if (!shareDir.deleteRecursively()) {
                        invoke.reject("Failed to delete all temporary share files.")
                        return
                    }
                }
            } catch (e: Exception) {
                invoke.reject("Error during cleanup: ${e.message}", e)
                return
            }
        }
        invoke.resolve()
    }

    /** Writes Base64 files and resolves file paths into shareable content URIs. */
    @Throws(IOException::class, SecurityException::class)
    private fun stageFiles(args: ShareOptions, staged: StagedFiles) {
        val authority = ShareFileProvider.authority(activity)

        args.files?.forEach { file ->
            val decodedBytes = Base64.decode(
                ShareValidation.stripBase64Whitespace(file.data),
                Base64.DEFAULT
            )
            ShareValidation.validateDecodedFileSize(file, decodedBytes.size)
            val tempFile = createSafeFile(file.name)
            staged.ownedFiles.add(tempFile)
            FileOutputStream(tempFile).use { outputStream ->
                outputStream.write(decodedBytes)
            }

            staged.uris.add(FileProvider.getUriForFile(activity, authority, tempFile))
            staged.mimeTypes.add(file.mimeType.ifEmpty { DEFAULT_MIME_TYPE })
        }

        args.filePaths?.forEach { path ->
            val file = File(path)
            if (!file.exists() || !file.isFile) {
                throw SecurityException("File does not exist or is not a regular file: $path")
            }

            // Prefer sharing directly through the plugin's FileProvider.
            // If the file is outside a declared root, copy it to the plugin's
            // cache share directory and share from there.
            val shareUri = try {
                FileProvider.getUriForFile(activity, authority, file)
            } catch (e: IllegalArgumentException) {
                val safeFile = copyToSafeShareDir(file)
                staged.ownedFiles.add(safeFile)
                FileProvider.getUriForFile(activity, authority, safeFile)
            }

            staged.uris.add(shareUri)
            staged.mimeTypes.add(
                URLConnection.guessContentTypeFromName(file.name) ?: DEFAULT_MIME_TYPE
            )
        }
    }

    private fun buildChooserIntent(staged: StagedFiles, payload: ShareIntentPayload): Intent {
        val fileUris = staged.uris
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

        shareIntent.type = if (staged.mimeTypes.isNotEmpty()) {
            determineMimeType(staged.mimeTypes)
        } else {
            "text/plain"
        }

        payload.body?.let {
            shareIntent.putExtra(Intent.EXTRA_TEXT, it)
        }
        payload.title?.let {
            shareIntent.putExtra(Intent.EXTRA_TITLE, it)
        }
        payload.intentSubject()?.let {
            shareIntent.putExtra(Intent.EXTRA_SUBJECT, it)
        }

        shareIntent.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        return Intent.createChooser(shareIntent, payload.title)
    }

    /** Launches the chooser on the main thread; commands arrive on a background thread. */
    private fun launchChooser(invoke: Invoke, chooser: Intent) {
        activity.runOnUiThread {
            try {
                startActivityForResult(invoke, chooser, "shareResult")
            } catch (e: Exception) {
                val filesToClean = synchronized(lock) {
                    val files = pendingCleanupFiles
                    resetPendingShareLocked()
                    files
                }
                cleanupFiles(filesToClean)
                invoke.reject("Failed to share content: ${e.message}", e)
            }
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

    private fun resolvePendingShare(invoke: Invoke? = null) {
        val (pendingInvoke, filesToClean) = synchronized(lock) {
            val pending = pendingShareInvoke
            val files = pendingCleanupFiles
            pendingShareInvoke = null
            pendingCleanupFiles = emptyList()
            Pair(pending, files)
        }
        scheduleCleanup(filesToClean)
        (invoke ?: pendingInvoke)?.resolve()
    }

    /** Must be called while holding [lock]. */
    private fun resetPendingShareLocked() {
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
        val shareDir = File(activity.cacheDir, "shares")
        files.forEach { file ->
            try {
                if (file.exists()) {
                    file.delete()
                }
                // Copied files live in a per-file UUID subdirectory; remove it once empty.
                val parent = file.parentFile
                if (parent != null && parent.canonicalPath != shareDir.canonicalPath) {
                    ensureInside(shareDir, parent)
                    parent.delete()
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
     * Path traversal check: ensures the resolved path of [file] is strictly inside [dir].
     */
    @Throws(IOException::class, SecurityException::class)
    private fun ensureInside(dir: File, file: File) {
        if (!file.canonicalPath.startsWith(dir.canonicalPath + File.separator)) {
            throw SecurityException("Path Traversal Attack Detected. Malicious filename: '${file.name}'")
        }
    }

    /**
     * Creates a safe File object within the dedicated share directory.
     * Only the final path component of the name is kept, and each file gets its
     * own UUID subdirectory so the original filename is preserved without collisions.
     */
    @Throws(IOException::class, SecurityException::class)
    private fun createSafeFile(untrustedFileName: String): File {
        val safeDir = getSafeShareDir()

        val baseName = File(untrustedFileName).name
        if (baseName.isEmpty() || baseName == "." || baseName == "..") {
            throw SecurityException("Invalid filename: '$untrustedFileName'.")
        }

        val uniqueDir = File(safeDir, UUID.randomUUID().toString())
        if (!uniqueDir.mkdirs()) {
            throw IOException("Failed to create share subdirectory.")
        }
        val intendedFile = File(uniqueDir, baseName)
        ensureInside(safeDir, intendedFile)
        return intendedFile
    }

    /**
     * Copies an existing file into the dedicated share directory in the app's cache.
     * A unique subdirectory is used so the original filename is preserved while
     * avoiding name collisions, and the plugin's FileProvider (which only exposes
     * cache/shares/) can grant URIs for the copy.
     */
    @Throws(IOException::class, SecurityException::class)
    private fun copyToSafeShareDir(source: File): File {
        val safeDir = getSafeShareDir()

        // Use a unique subdirectory for each copied file to avoid collisions
        // and to keep the original filename intact for target apps.
        val uniqueDir = File(safeDir, UUID.randomUUID().toString())
        if (!uniqueDir.mkdirs()) {
            throw IOException("Failed to create share subdirectory.")
        }

        val dest = File(uniqueDir, source.name)
        try {
            ensureInside(safeDir, dest)
            source.copyTo(dest, overwrite = true)
        } catch (e: Exception) {
            dest.delete()
            uniqueDir.delete()
            throw e
        }
        return dest
    }
}
