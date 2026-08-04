# Tauri Plugin share

`tauri-plugin-vnidrop-share`

A Tauri plugin that provides a seamless, cross-platform interface for native sharing. This plugin allows your application to share text, URLs, and files using the native share dialogs on macOS, Windows, and mobile devices.

## Why this plugin?

The web's native [Web Share API](https://developer.mozilla.org/en-US/docs/Web/API/Navigator/share) offers a great way to integrate with a device's sharing capabilities. However, a key limitation is that it only works in a **secure context** (i.e., HTTPS). Since Tauri applications run in a local, non-secure context, the Web Share API is unavailable. This plugin replicates the functionality of the Web Share API, providing a familiar and easy-to-use interface for Tauri developers while leveraging the underlying native APIs to ensure full functionality on all supported platforms.

For file sharing, the plugin intelligently manages the lifecycle of temporary files. It creates secure temporary files from Base64 data, ensuring they persist for the duration of the sharing operation, and automatically cleans them up when the application exits. On mobile platforms like Android and iOS, the native sharing APIs are directly invoked, and temporary files are managed and cleaned up within the native code. Android file cleanup is delayed briefly after the app resumes so receiving apps have time to open granted file URIs.

For release safety, share payloads are bounded on both the JavaScript and native sides. URLs must be well-formed `http://` or `https://` URLs; other schemes should be shared as plain text instead. A single share request may include up to 16 files, each file may be up to 50 MiB, and the total file payload may be up to 100 MiB. Text is limited to 64 KiB, titles to 1 KiB, URLs to 4 KiB, and file names/MIME types to 255 bytes.

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

## Usage

### Frontend (TypeScript/JavaScript)

The frontend API is designed to closely resemble the Web Share API, making it intuitive for developers.

1. **Checking Share Availability**

   Use the `canShare()` function to check if the current platform supports native sharing. This is useful for conditionally displaying a share button. On Linux, this function will return `false`, and calling `share()` will do nothing.

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

   Use the `share()` function with a `ShareData` object to trigger the native dialog. The `files` field requires an array of `File` objects, which the plugin automatically handles by converting them to Base64 and managing their lifecycle in the backend. For content that already exists on disk, use `filePaths` to share the file directly while preserving its original filename. On Android, paths inside a configured `FileProvider` root are shared as-is; paths outside a declared root are copied to the plugin's cache share directory and cleaned up automatically. At least one of `text`, `url`, a non-empty `files` array, or a non-empty `filePaths` array must be provided.
   Note: on Android and Windows, the promise resolves when the app regains focus after the share UI closes (best-effort). On macOS, the share delegate is used to resolve when the share completes. On iOS, the promise is resolved using the native completion handler (`UIActivityViewController.completionWithItemsHandler`), which provides accurate resolution when sharing completes. We may expose a configuration option in the future to let developers choose the resolution behavior (immediate vs. on-focus vs. delayed).

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

   While the plugin automatically handles cleanup when the app exits, you can manually call `cleanup()` to remove temporary files after a share operation is complete to free up disk space. Avoid calling `cleanup()` while a share sheet is still open. The `cleanup` command is not included in the default permission set; enable `vnidrop-share:allow-cleanup` explicitly if your app calls it from the frontend.

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

The plugin registers a `FileProvider` with authority `<your-application-id>.fileprovider`. By default it only exposes the plugin's own cache directory (`cache/shares/`) for temporary files.

If you want `filePaths` to share files directly from your app's directories (without making a temporary copy), add those directories to your app's `file_paths.xml` resource. In a default Tauri v2 project this is `src-tauri/gen/android/app/src/main/res/xml/file_paths.xml`; the `xml` directory and file may need to be created, and the exact path changes if you have customized `TAURI_ANDROID_PROJECT_PATH`. The plugin will try to use the original file first and only copy it if the path is not under a declared `FileProvider` root.

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

   ```rs
   // A Tauri command example
    use tauri::{command, AppHandle, Runtime};
    use tauri_plugin_vnidrop_share::{ShareExt, Result, ShareOptions};

    #[command]
    async fn custom_share_command<R: Runtime>(app: AppHandle<R>) -> Result<()> {
    let share_options = ShareOptions {
        text: Some("Hello from Rust!".to_string()),
        title: Some("Rust Share".to_string()),
        url: None,
        files: None,
        file_paths: None,
        anchor: None,
    };

    // Use the extension trait to access the plugin's API
    app.share().share(app.get_webview_window("main").unwrap(), share_options, app.state())?;
    Ok(())
    }
   ```

## Testing

The repository includes Rust, JavaScript, Android JVM, iOS Swift, and example-app checks:

```sh
bun run build
bun run test
bun run test:types
cargo test
cd android && gradle testDebugUnitTest
cd ios && VNIDROP_SHARE_USE_TAURI_STUB=1 swift test
cd examples/tauri-app && npm install && npm run build && npm audit
cd examples/tauri-app/src-tauri && cargo check
```
