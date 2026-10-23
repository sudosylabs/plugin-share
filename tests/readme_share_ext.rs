//! Compiles the `ShareExt` example from the README so it cannot drift from the API.

use tauri::{command, AppHandle, Manager, Runtime, Window};
use tauri_plugin_vnidrop_share::{ShareExt, ShareOptions};

#[allow(dead_code)]
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

#[test]
fn readme_share_ext_example_compiles() {}
