use super::focus;
use crate::state::PluginTempFileManager;
use crate::{CanShareResult, Error, ShareOptions, SharedFile, MAX_FILE_BYTES};
use base64::{engine::general_purpose, Engine as _};
use log::error;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::cell::RefCell;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tauri::{Runtime, State, Window};
use windows::core::{Interface, HSTRING};
use windows::ApplicationModel::DataTransfer::{
    DataPackageOperation, DataRequestDeferral, DataRequestedEventArgs, DataTransferManager,
};
use windows::Foundation::{TypedEventHandler, Uri};
use windows::Storage::{IStorageItem, StorageFile};
use windows::Win32::{
    Foundation::HWND,
    System::WinRT::{RoInitialize, RO_INIT_SINGLETHREADED},
    UI::Shell::IDataTransferManagerInterop,
};
use windows_collections::IIterable;

// This thread-local holds the DataTransferManager and its event registration token, keeping them
// alive for the duration of the asynchronous share operation. It's only accessible
// on the main thread, which is safe for these non-thread-safe WinRT types.
thread_local! {
    static SHARE_STATE: RefCell<Option<(DataTransferManager, i64)>> = RefCell::new(None);
}

// A helper to map the detailed windows::core::Error into our plugin's simpler error type.
impl From<windows::core::Error> for Error {
    fn from(err: windows::core::Error) -> Self {
        Error::NativeApi(err.message().to_string())
    }
}

/// Ensures a `DataRequestDeferral` is always completed when the handler scope ends.
struct DeferralGuard(Option<DataRequestDeferral>);

impl DeferralGuard {
    fn new(deferral: DataRequestDeferral) -> Self {
        Self(Some(deferral))
    }
}

impl Drop for DeferralGuard {
    fn drop(&mut self) {
        if let Some(deferral) = self.0.take() {
            let _ = deferral.Complete();
        }
    }
}

pub fn cleanup() -> Result<(), Error> {
    let temp_dir = get_plugin_temp_dir()?;
    if temp_dir.exists() {
        std::fs::remove_dir_all(temp_dir)
            .map_err(|e| Error::TempFile(format!("Failed to cleanup temp dir: {}", e)))?;
    }
    Ok(())
}

pub fn can_share() -> Result<CanShareResult, Error> {
    Ok(CanShareResult { value: true })
}

pub fn share<R: Runtime>(
    window: Window<R>,
    options: ShareOptions,
    state: State<'_, PluginTempFileManager>,
) -> Result<(), Error> {
    let focus_wait = focus::begin_focus_wait(&window)?;
    let (tx, rx) = mpsc::channel();
    let win_clone = window.clone();

    let managed_files_arc = state.inner().managed_files.clone();
    let tx_handler = tx.clone();

    let options = options;

    if let Err(e) = window.run_on_main_thread(move || {
        let tx_for_handler = tx_handler;
        let result = (|| -> Result<(), Error> {
            initialize_winrt_thread()?;
            let hwnd = get_hwnd(&win_clone)?;
            let (dtm, interop) = get_data_transfer_manager(hwnd)?;

            let data_requested_handler = TypedEventHandler::new({
                let options_clone = std::sync::Arc::new(options.clone());
                let managed_files_arc_clone_for_handler = managed_files_arc.clone();
                move |_, args: windows::core::Ref<'_, DataRequestedEventArgs>| -> windows::core::Result<()> {
                    let handler_result = (|| -> Result<(), Error> {
                        let request_args = (*args).as_ref().ok_or_else(|| Error::NativeApi("Missing DataRequestedEventArgs".to_string()))?;
                        let request = request_args.Request()?;
                        let data = request.Data()?;
                        let properties = data.Properties()?;

                        let title = options_clone.title.clone().unwrap_or_else(|| {
                            options_clone
                                .file_paths
                                .as_ref()
                                .and_then(|paths| paths.first())
                                .and_then(|path| Path::new(path).file_name())
                                .map(|name| name.to_string_lossy().into_owned())
                                .or_else(|| {
                                    options_clone
                                        .files
                                        .as_ref()
                                        .and_then(|files| files.first())
                                        .map(|file| file.name.clone())
                                })
                                .unwrap_or_else(|| "Shared content".to_string())
                        });

                        properties.SetTitle(&HSTRING::from(&title))?;

                        if let (Some(t), Some(u)) = (&options_clone.text, &options_clone.url) {
                            // Set the plain text content.
                            data.SetText(&HSTRING::from(t))?;

                            // Attempt to parse the URL string into a Windows Uri object.
                            // It is crucial to validate the URL string to ensure it forms a valid Uri.
                            if let Ok(uri) = Uri::CreateUri(&HSTRING::from(u)) {
                                // For web URLs (HTTP/HTTPS), SetWebLink is the preferred method.
                                // For application-specific URIs, SetApplicationLink would be used.
                                // Here, we assume it's a web URL for demonstration.
                                data.SetWebLink(&uri)?;
                            } else {
                                // If the URL string cannot be parsed into a valid Uri object,
                                // a warning is logged. In such cases, the URL might still be
                                // valuable as part of the plain text.
                                error!("Warning: Could not parse URL '{}' for DataPackage::SetWebLink. Setting as part of text.", u);
                                // Optionally, if it's critical for the URL to be present in some form,
                                // even if not semantically, it could be appended to the plain text.
                                let combined_text_fallback = format!("{}\n{}", t, u);
                                data.SetText(&HSTRING::from(combined_text_fallback))?;
                                // However, the primary goal remains semantic separation.
                            }
                        }
                        // If only text is provided, simply set the plain text content.
                        else if let Some(t) = &options_clone.text {
                            if !t.is_empty() {
                                data.SetText(&HSTRING::from(t))?;
                            }
                        }
                        // If only a URL is provided, attempt to set it semantically.
                        else if let Some(u) = &options_clone.url {
                            if let Ok(uri) = Uri::CreateUri(&HSTRING::from(u)) {
                                data.SetWebLink(&uri)?;
                            } else {
                                // If URL parsing fails, fall back to setting it as plain text.
                                // This ensures the URL string is still transferred, even without its semantic type.
                                error!("Warning: Could not parse URL '{}' for DataPackage::SetWebLink. Setting as plain text.", u);
                                data.SetText(&HSTRING::from(u))?;
                            }
                        }

                        if options_clone.files.is_some() || options_clone.file_paths.is_some() {
                            let deferral = request.GetDeferral()?;
                            let _guard = DeferralGuard::new(deferral);
                            let mut storage_items: Vec<Option<IStorageItem>> = Vec::new();

                            if let Some(files) = &options_clone.files {
                                let temp_dir = get_plugin_temp_dir()?;
                                for file in files {
                                    let path_buf = create_temp_file_for_data(file, &temp_dir)?;
                                    let path_str = path_buf.to_string_lossy().to_string();
                                    let mut files = managed_files_arc_clone_for_handler
                                        .lock()
                                        .map_err(|e| Error::NativeApi(format!("Failed to lock temp file manager: {}", e)))?;
                                    files.push(path_buf);

                                    let storage_file = StorageFile::GetFileFromPathAsync(&HSTRING::from(path_str))
                                        .map_err(|e| Error::NativeApi(format!("GetFileFromPathAsync failed for temp file: {}", e)))?
                                        .get()
                                        .map_err(|e| Error::NativeApi(format!("GetFileFromPathAsync result failed for temp file: {}", e)))?;
                                    let item: IStorageItem = storage_file.cast()
                                        .map_err(|e| Error::NativeApi(format!("Failed to cast temp StorageFile to IStorageItem: {}", e)))?;
                                    storage_items.push(Some(item));
                                }
                            }

                            if let Some(file_paths) = &options_clone.file_paths {
                                for path in file_paths {
                                    if path.is_empty() {
                                        continue;
                                    }

                                    // Normalize any forward slashes to Windows backslashes. The
                                    // frontend may pass absolute paths with either separator, but
                                    // WinRT's StorageFile API is stricter about backslash separators.
                                    let normalized_path = path.replace('/', "\\");

                                    let storage_file = StorageFile::GetFileFromPathAsync(&HSTRING::from(normalized_path))
                                        .map_err(|e| Error::NativeApi(format!("GetFileFromPathAsync failed for '{}': {}", path, e)))?
                                        .get()
                                        .map_err(|e| Error::NativeApi(format!("GetFileFromPathAsync result failed for '{}': {}", path, e)))?;
                                    let item: IStorageItem = storage_file.cast()
                                        .map_err(|e| Error::NativeApi(format!("Failed to cast StorageFile to IStorageItem for '{}': {}", path, e)))?;
                                    storage_items.push(Some(item));
                                }
                            }

                            if !storage_items.is_empty() {
                                let iterable_items: IIterable<IStorageItem> = storage_items.try_into()
                                    .map_err(|e| Error::NativeApi(format!("Failed to convert storage items to IIterable: {}", e)))?;

                                data.SetRequestedOperation(DataPackageOperation::Copy)
                                    .map_err(|e| Error::NativeApi(format!("Failed to set requested operation: {}", e)))?;
                                data.SetStorageItems(&iterable_items, true)
                                    .map_err(|e| Error::NativeApi(format!("Failed to set storage items on DataPackage: {}", e)))?;
                            }
                        }

                        Ok(())
                    })();

                    if let Err(e) = &handler_result {
                        error!("[share] DataRequested handler failed: {}", e);
                    }
                    let _ = tx_for_handler.send(handler_result);

                    SHARE_STATE.with(|state| {
                        if let Some((manager, token)) = state.borrow_mut().take() {
                            let _ = manager.RemoveDataRequested(token);
                        }
                    });

                    Ok(())
                }
            });

            let token = dtm.DataRequested(&data_requested_handler)?;

            SHARE_STATE.with(|state| {
                *state.borrow_mut() = Some((dtm, token));
            });

            unsafe { interop.ShowShareUIForWindow(hwnd) }?;
            Ok(())
        })();

        if let Err(err) = result {
            let _ = tx.send(Err(err));
        }
    }) {
        focus_wait.cancel();
        return Err(e.into());
    }

    let share_result = match rx.recv_timeout(std::time::Duration::from_secs(30)) {
        Ok(result) => result,
        Err(err) => {
            focus_wait.cancel();
            return Err(Error::NativeApi(format!(
                "Share timed out waiting for DataRequested event: {}",
                err
            )));
        }
    };
    if let Err(err) = share_result {
        focus_wait.cancel();
        return Err(err);
    }

    focus_wait.wait()?;
    Ok(())
}

/// Initializes the Windows Runtime on the current thread.
fn initialize_winrt_thread() -> Result<(), Error> {
    // RoInitialize can be called multiple times on the same thread.
    // It will return S_FALSE if already initialized, which is not an error.
    unsafe { RoInitialize(RO_INIT_SINGLETHREADED) }
        .map_err(|e| Error::NativeApi(format!("Failed to initialize WinRT: {}", e)))
}

/// Retrieves the native window handle (HWND) from the Tauri window.
fn get_hwnd<R: Runtime>(window: &Window<R>) -> Result<HWND, Error> {
    let handle = window
        .window_handle()
        .map_err(|e| Error::NativeApi(e.to_string()))?;

    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(HWND(handle.hwnd.get() as *mut std::ffi::c_void)),
        _ => Err(Error::NativeApi(
            "Unsupported window handle type".to_string(),
        )),
    }
}

/// Gets an instance of the DataTransferManager associated with the window's HWND.
/// This is the required method for desktop (non-UWP) applications. [1]
fn get_data_transfer_manager(
    hwnd: HWND,
) -> Result<(DataTransferManager, IDataTransferManagerInterop), Error> {
    let interop = windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()?;
    let dtm = unsafe { interop.GetForWindow(hwnd) }?;
    Ok((dtm, interop))
}

/// Returns the path to a dedicated, secure directory for this plugin's temporary files.
fn get_plugin_temp_dir() -> Result<PathBuf, Error> {
    let dir = std::env::temp_dir().join("tauri-plugin-share");
    if !dir.exists() {
        std::fs::create_dir_all(&dir)
            .map_err(|e| Error::TempFile(format!("Failed to create temp dir: {}", e)))?;
    }
    Ok(dir)
}

/// Creates a secure temporary file from Base64 data inside `temp_dir`.
fn create_temp_file_for_data(file: &SharedFile, temp_dir: &Path) -> Result<PathBuf, Error> {
    let decoded_bytes = general_purpose::STANDARD
        .decode(&file.data)
        .map_err(|_| Error::InvalidArgs("Invalid Base64 data provided".to_string()))?;
    if decoded_bytes.len() > MAX_FILE_BYTES {
        return Err(Error::InvalidArgs(format!(
            "File '{}' exceeds the maximum size of {} bytes.",
            file.name, MAX_FILE_BYTES
        )));
    }

    // Security: Sanitize the filename to prevent path traversal attacks.
    // We only use the filename part and ignore any directory structure.
    let sanitized_name = Path::new(&file.name)
        .file_name()
        .ok_or_else(|| Error::InvalidArgs("Invalid file name provided".to_string()))?
        .to_str()
        .ok_or_else(|| Error::InvalidArgs("File name contains invalid UTF-8".to_string()))?;

    let temp_path = temp_dir.join(format!("{}-{}", uuid::Uuid::new_v4(), sanitized_name));

    let mut file_handle = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)
        .map_err(|e| Error::TempFile(format!("Failed to create temp file: {}", e)))?;

    file_handle
        .write_all(&decoded_bytes)
        .map_err(|e| Error::TempFile(format!("Failed to write to temp file: {}", e)))?;

    Ok(temp_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn sample_shared_file() -> SharedFile {
        SharedFile {
            data: general_purpose::STANDARD.encode(b"hello world"),
            name: "hello.txt".to_string(),
            mime_type: "text/plain".to_string(),
        }
    }

    #[test]
    fn create_temp_file_for_data_creates_expected_file() {
        let temp_dir = TempDir::new().unwrap();
        let file = sample_shared_file();

        let path = create_temp_file_for_data(&file, temp_dir.path()).unwrap();

        assert_eq!(path.parent().unwrap(), temp_dir.path());
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(name.ends_with("-hello.txt"), "unexpected file name: {name}");
        assert_eq!(fs::read(&path).unwrap(), b"hello world");
    }

    #[test]
    fn create_temp_file_for_data_rejects_invalid_base64() {
        let temp_dir = TempDir::new().unwrap();
        let file = SharedFile {
            data: "not-valid-base64!!!".to_string(),
            name: "bad.txt".to_string(),
            mime_type: "text/plain".to_string(),
        };

        let err = create_temp_file_for_data(&file, temp_dir.path()).unwrap_err();

        assert!(
            matches!(err, Error::InvalidArgs(_)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn create_temp_file_for_data_sanitizes_path_traversal_in_name() {
        let temp_dir = TempDir::new().unwrap();
        let file = SharedFile {
            data: general_purpose::STANDARD.encode(b"payload"),
            name: "../etc/secret.txt".to_string(),
            mime_type: "text/plain".to_string(),
        };

        let path = create_temp_file_for_data(&file, temp_dir.path()).unwrap();

        assert_eq!(path.parent().unwrap(), temp_dir.path());
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(
            name.ends_with("-secret.txt"),
            "unexpected file name: {name}"
        );
    }
}
