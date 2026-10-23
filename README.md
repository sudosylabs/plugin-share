# Tauri Plugin share

`tauri-plugin-vnidrop-share`

A Tauri plugin that provides a seamless, cross-platform interface for native sharing. This plugin allows your application to share text, URLs, and files using the native share dialogs on macOS, Windows, and mobile devices.

## Why this plugin?

The web's native [Web Share API](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/share) offers a great way to integrate with a device's sharing capabilities. However, a key limitation is that it only works in a **secure context** (i.e., HTTPS). Since Tauri applications run in a local, non-secure context, the Web Share API is unavailable. This plugin replicates the functionality of the Web Share API, providing a familiar and easy-to-use interface for Tauri developers while leveraging the underlying native APIs to ensure full functionality on all supported platforms.

For file sharing, the plugin manages the lifecycle of temporary files. Files passed in `files` are written from Base64 data into their own temporary directory, keeping their original file name, and stay available for the duration of the share:

- **Desktop (macOS, Windows):** temporary files are removed when you call `cleanup()` or when the application exits.
- **iOS:** temporary files are removed when the share sheet completes.
- **Android:** temporary files are removed 5 minutes after the share completes, so receiving apps have time to open the granted file URIs.

For release safety, share payloads are bounded on both the JavaScript and native sides, and every platform applies the same rules:

- URLs must be well-formed `http://` or `https://` URLs with a host and no whitespace or control characters. Other schemes should be shared as plain text instead.
- A single share request may include up to 16 files (`files` and `filePaths` combined).
- Each file in `files` may be up to 50 MiB, and all of `files` together up to 100 MiB. Files shared by path are not size-limited.
- Each `filePaths` entry must be an absolute path to an existing file.
- Text is limited to 64 KiB, titles and subjects to 1 KiB, URLs to 4 KiB, file paths to 4 KiB, and file names/MIME types to 255 bytes.
- Empty `text`, `title`, `subject`, and `url` strings are treated as if they were not provided.

## Installation

### Rust

Add the plugin to your `Cargo.toml`:

```sh
[dependencies]
tauri-plugin-vnidrop-share = "1.0.0-rc"
```

### Frontend

Install the JavaScript package using npm:

```sh
npm install @vnidrop/tauri-plugin-share
```

Prereleases are published under the `next` dist-tag (`npm install @vnidrop/tauri-plugin-share@next`).

## Usage

### Frontend (TypeScript/JavaScript)

The frontend API is designed to closely resemble the Web Share API, making it intuitive for developers.

1. **Checking Share Availability**

   Use the `canShare()` function to check if the current platform supports native sharing. This is useful for conditionally displaying a share button. On Linux, this function will return `false`, and calling `share()` rejects with an "unsupported platform" error.

   ```ts
   import { canShare } from "@vnidrop/tauri-plugin-share";

   async function checkShareSupport() {
     const isShareAvailable = await canShare();
     if (isShareAvailable) {
       console.log("Sharing is supported on this platform!");
       // Show share button
     } else {
       console.log("Sharing is not available.");
       // Hide share button
     }
   }
   ```

2. **Sharing Content**

   Use the `share()` function with a `ShareData` object to trigger the native dialog. The `files` field requires an array of `File` objects, which the plugin automatically handles by converting them to Base64 and managing their lifecycle in the backend. For content that already exists on disk, use `filePaths` to share the file directly while preserving its original filename. On Android, paths inside a configured `FileProvider` root are shared as-is; paths outside a declared root are copied to the plugin's cache share directory and cleaned up automatically. You may also set optional `subject` (primarily for Android; see the platform table below). At least one of `text`, `url`, a non-empty `files` array, or a non-empty `filePaths` array must be provided.
   **When the promise resolves.** It resolves once the share UI is closed. Platforms do not report reliably whether the user actually shared or cancelled, so a dismissed dialog also resolves. It rejects for invalid input, missing files, or a native error.

   - **Android and Windows:** resolves when the app regains focus after the share UI closes (best-effort).
   - **macOS:** resolves when the chosen service reports back, when the picker is dismissed, or after 60 seconds if the service never reports back.
   - **iOS:** resolves from `UIActivityViewController.completionWithItemsHandler`.

   Errors are rejected as plain strings, not `Error` objects.

   **How content is shared on each platform.**

   | | Android | iOS | macOS | Windows |
   | --- | --- | --- | --- | --- |
   | `text` + `url` | Sent together as `text\nurl` | Separate text and URL items | Separate text and URL items | Text plus a web link; text-only targets also see the URL |
   | `title` | Chooser title (`EXTRA_TITLE`) | Subject (for example, the Mail subject) | Subject | Share UI title |
   | `subject` | `EXTRA_SUBJECT` (falls back to `title` when omitted); use for targets such as Google Drive default file name | No effect | No effect | No effect |
   | `text`/`url` alongside files | Included | Included | Included | Included |

   On Android, some share targets (for example Google Drive) use `EXTRA_SUBJECT` as the default file name. Set `subject` to the filename you want saved (often `file.name` or the basename of a `filePaths` entry) while keeping `title` as the human-readable chooser label. `EXTRA_SUBJECT` is a single string; it does not apply per file when sharing multiple files. The system chooser label comes from `title` only (`Intent.createChooser`); setting `subject` without `title` does not set the chooser title.

   On Windows, when no `title` is given, the first file name (or "Shared content") is used, because Windows requires a title.

   ```ts
   import { share, canShare } from "@vnidrop/tauri-plugin-share";

   // Share text and a URL
   async function shareTextAndUrl() {
     if (await canShare()) {
       await share({
         title: "My Project",
         text: "Check out this awesome project built with Tauri!",
         url: "https://github.com/vnidrop/plugin-share",
       });
       console.log("Share dialog closed.");
     }
   }

   // Share with an anchor (iPadOS and macOS only)
   async function shareFromButton(button: HTMLElement) {
     const rect = button.getBoundingClientRect();
     if (await canShare()) {
       await share({
         title: "My Project",
         url: "https://github.com/vnidrop/plugin-share",
         anchor: {
           x: rect.left,
           y: rect.top,
           width: rect.width,
           height: rect.height,
         },
       });
     }
   }

   // Share a file (e.g., an image)
   async function shareFile() {
     const fileInput = document.querySelector(
       'input[type="file"]',
     ) as HTMLInputElement;
     const file = fileInput.files?.[0];

     if (file && (await canShare())) {
       await share({
         title: "Shared File",
         files: [file],
       });
       console.log("File shared successfully.");
     }
   }

   // Share existing files on disk by path (preserves original filename)
   async function shareByPath() {
     if (await canShare()) {
       await share({
         title: "Saved Report",
         filePaths: ["/Users/me/report.zip"],
       });
     }
   }
   ```

3. Manual Cleanup

   While the plugin automatically handles cleanup when the app exits, you can manually call `cleanup()` to remove temporary files after a share operation is complete to free up disk space. `cleanup()` rejects with "Cannot cleanup while sharing is in progress." while a share is still open. The `cleanup` command is not included in the default permission set; enable `vnidrop-share:allow-cleanup` explicitly if your app calls it from the frontend.

   ```ts
   import { cleanup } from "@vnidrop/tauri-plugin-share";

   await cleanup();
   console.log("Temporary files have been cleaned up.");
   ```

### Rust

1. **Plugin Initialization**

   Add the plugin to your `main.rs` file within the `tauri::Builder` to register the commands and enable state management for file cleanup on application exit.

   ```rs
   // src/main.rs
    fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_vnidrop_share::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
   }
   ```

### Android troubleshooting

If Android reports `vnidrop-share.can_share not allowed. Plugin not found`, the APK does not have the share plugin registered in Tauri's runtime/ACL. Verify that your app calls:

```rs
.plugin(tauri_plugin_vnidrop_share::init())
```

and that the active capability includes the plugin permission:

```json
"vnidrop-share:default"
```

After changing plugin permissions or native Android code, fully rebuild and reinstall the Android app. A stale installed APK can keep showing this error even after the source code is corrected.

#### FileProvider and `filePaths`

The plugin registers its own `FileProvider` subclass (`plugin.vnidrop.share.ShareFileProvider`) with authority `<your-application-id>.vnidrop.share.fileprovider`, so it does not clash with a `FileProvider` your app declares under `<your-application-id>.fileprovider`. By default it only exposes the plugin's own cache directory (`cache/shares/`) for temporary files.

If you want `filePaths` to share files directly from your app's directories (without making a temporary copy), add those directories to the `file_paths.xml` resource that the plugin's provider reads. In a default Tauri v2 project this is `src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`; the `xml` directory and file may need to be created, and the exact path changes if you have customized `TAURI_ANDROID_PROJECT_PATH`. The plugin will try to use the original file first and only copy it if the path is not under a declared `FileProvider` root.

```xml
<?xml version="1.0" encoding="utf-8"?>
<paths>
    <!-- Keep a cache-path so the plugin can still share its temporary files -->
    <cache-path name="plugin_cache" path="shares/" />
    <files-path name="movies" path="data/movies" />
    <files-path name="music" path="data/music" />
</paths>
```

Make sure your custom `file_paths.xml` still contains a `cache-path` entry (for example `path="."` or `path="shares/"`) so that files created from the `files` array and fallback copies remain accessible.

If a `filePath` is outside any declared root, the plugin copies it to its cache share directory, shares the copy, and cleans it up automatically.

2. **Using the `ShareExt` Trait**

   The `ShareExt` trait is provided for a more idiomatic way to access the plugin's functionalities directly from an `AppHandle` or `Window`.

   `share()` blocks until the share UI closes, so call it from a blocking task rather than directly on the async runtime or the main thread.

   ```rs
   use tauri::{command, AppHandle, Manager, Runtime, Window};
   use tauri_plugin_vnidrop_share::{ShareExt, ShareOptions};

   #[command]
   async fn custom_share_command<R: Runtime>(
       app: AppHandle<R>,
       window: Window<R>,
   ) -> Result<(), String> {
       let share_options = ShareOptions {
           text: Some("Hello from Rust!".to_string()),
           title: Some("Rust Share".to_string()),
           subject: None,
           url: None,
           files: None,
           file_paths: None,
           anchor: None,
       };

       tauri::async_runtime::spawn_blocking(move || {
           app.share().share(window, share_options, app.state())
       })
       .await
       .map_err(|e| e.to_string())?
       .map_err(|e| e.to_string())
   }
   ```

## Upgrading from 1.0.0-rc.4

The version after `1.0.0-rc.4` contains breaking Rust changes.

- **Rust:** `ShareOptions` includes `subject: Option<String>`. Struct literals must set `subject: None` or `Some(...)` or they will not compile.
- **JavaScript / TypeScript:** `subject` is optional on `ShareData`; existing callers that omit it are unchanged.
- **Android file shares:** When the chooser label must differ from the name a target uses (for example Google Drive), set `subject` to the desired filename (`file.name` or a `filePaths` basename) and use `title` for the chooser label (see the platform table above).
- **Behavior:** If you only set `title`, Android still sets `EXTRA_SUBJECT` from `title` as before.

## Testing

The repository includes Rust, JavaScript, Android JVM, iOS Swift, and example-app checks. All four validation implementations are checked against the shared cases in `tests/fixtures/validation-vectors.json`.

```sh
bun run build
bun run test
bun run test:types
cargo test --all-targets
cd android && gradle testDebugUnitTest
cd ios && VNIDROP_SHARE_USE_TAURI_STUB=1 swift test
cd examples/tauri-app && npm install && npm run build && npm audit
cd examples/tauri-app/src-tauri && cargo check
```
