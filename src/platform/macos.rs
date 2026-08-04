use crate::models::CanShareResult;
use crate::state::PluginTempFileManager;
use crate::{Error, ShareOptions, SharedFile, MAX_FILE_BYTES};
use base64::{engine::general_purpose, Engine as _};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::Message;
use objc2::{
    define_class, msg_send,
    rc::{autoreleasepool, Retained},
    AnyThread, DefinedClass, MainThreadOnly,
};
use objc2_app_kit::{
    NSSharingService, NSSharingServiceDelegate, NSSharingServicePicker,
    NSSharingServicePickerDelegate, NSView,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSError, NSObject, NSObjectProtocol, NSString, NSURL,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle, WindowHandle};
use std::cell::RefCell;
use std::time::Duration;
use std::{io::Write, path::Path, sync::mpsc};
use tauri::{Runtime, State, Window};
use tempfile::{Builder, NamedTempFile};

const SHARE_COMPLETION_TIMEOUT: Duration = Duration::from_secs(60);

thread_local! {
    static ACTIVE_DELEGATES: RefCell<Vec<Retained<SharePickerDelegate>>> = RefCell::new(Vec::new());
}

#[derive(Default)]
struct ShareDelegateIvars {
    completion: RefCell<Option<mpsc::Sender<Result<(), Error>>>>,
    subject: RefCell<Option<Retained<NSString>>>,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = ShareDelegateIvars]
    struct SharePickerDelegate;

    unsafe impl NSObjectProtocol for SharePickerDelegate {}

    unsafe impl NSSharingServicePickerDelegate for SharePickerDelegate {
        #[unsafe(method_id(sharingServicePicker:delegateForSharingService:))]
        fn sharing_service_picker_delegate_for_sharing_service(
            &self,
            _picker: &NSSharingServicePicker,
            service: &NSSharingService,
        ) -> Option<Retained<ProtocolObject<dyn NSSharingServiceDelegate>>> {
            if let Some(subject) = self.ivars().subject.borrow().as_ref() {
                service.setSubject(Some(&**subject));
            }
            Some(ProtocolObject::from_retained(self.retain()))
        }

        #[unsafe(method(sharingServicePicker:didChooseSharingService:))]
        fn sharing_service_picker_did_choose_sharing_service(
            &self,
            _picker: &NSSharingServicePicker,
            service: Option<&NSSharingService>,
        ) {
            if service.is_none() {
                self.complete(Ok(()));
            }
        }
    }

    unsafe impl NSSharingServiceDelegate for SharePickerDelegate {
        #[unsafe(method(sharingService:didShareItems:))]
        fn sharing_service_did_share_items(&self, _service: &NSSharingService, _items: &NSArray) {
            self.complete(Ok(()));
        }

        #[unsafe(method(sharingService:didFailToShareItems:error:))]
        fn sharing_service_did_fail_to_share_items_error(
            &self,
            _service: &NSSharingService,
            _items: &NSArray,
            error: &NSError,
        ) {
            let message = autoreleasepool(|pool| unsafe {
                error.localizedDescription().to_str(pool).to_string()
            });
            self.complete(Err(Error::NativeApi(format!(
                "Sharing failed: {}",
                message
            ))));
        }
    }
);

impl SharePickerDelegate {
    fn new(
        mtm: MainThreadMarker,
        completion: mpsc::Sender<Result<(), Error>>,
        subject: Option<Retained<NSString>>,
    ) -> Retained<Self> {
        let ivars = ShareDelegateIvars {
            completion: RefCell::new(Some(completion)),
            subject: RefCell::new(subject),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }

    fn complete(&self, result: Result<(), Error>) {
        if let Some(tx) = self.ivars().completion.borrow_mut().take() {
            let _ = tx.send(result);
        }
        remove_active_delegate(self);
    }
}

fn remove_active_delegate(delegate: &SharePickerDelegate) {
    let delegate_ptr = delegate as *const SharePickerDelegate;
    ACTIVE_DELEGATES.with(|delegates| {
        let mut list = delegates.borrow_mut();
        if let Some(pos) = list
            .iter()
            .position(|item| Retained::as_ptr(item) == delegate_ptr)
        {
            list.remove(pos);
        }
    });
}

pub fn cleanup() -> Result<(), Error> {
    // macOS file cleanup is handled by the shared `PluginTempFileManager` in desktop::Share::cleanup
    // and on plugin drop. Nothing additional is required here.
    Ok(())
}

pub fn can_share() -> Result<CanShareResult, Error> {
    // On macOS, we can always share as long as the sharing service is available.
    Ok(CanShareResult { value: true })
}

/// Shares content using the native macOS sharing service.
pub fn share<R: Runtime>(
    window: Window<R>,
    options: ShareOptions,
    state: State<'_, PluginTempFileManager>,
) -> Result<(), Error> {
    let (setup_tx, setup_rx) = mpsc::channel();
    let (completion_tx, completion_rx) = mpsc::channel();
    let window_clone = window.clone();

    let managed_files = state.inner().managed_files.clone();

    if let Err(e) = window.run_on_main_thread(move || {
        let result = (|| -> Result<(), Error> {
            let ns_view = get_ns_view(&window_clone)?;
            let mut items_to_share: Vec<Retained<NSObject>> = Vec::new();

            let temp_file_manager_clone = managed_files.clone();

            let has_files = options.files.as_ref().is_some_and(|f| !f.is_empty())
                || options.file_paths.as_ref().is_some_and(|p| !p.is_empty());

            let subject = if has_files {
                options
                    .title
                    .as_ref()
                    .or(options.text.as_ref())
                    .map(|s| NSString::from_str(s))
            } else {
                None
            };

            if !has_files {
                if let Some(url) = options.url.as_deref().filter(|value| !value.is_empty()) {
                    if let Some(url_obj) = unsafe { NSURL::URLWithString(&NSString::from_str(url)) } {
                        items_to_share.push(unsafe { Retained::cast_unchecked(url_obj) });
                    }
                }

                if let Some(text) = options.text.as_deref().filter(|value| !value.is_empty()) {
                    items_to_share.push(unsafe {
                        Retained::cast_unchecked(NSString::from_str(text))
                    });
                }
            }

            if let Some(files) = options.files {
                for file in files {
                    let temp_file_named = create_temp_file_for_data(&file)?;
                    let temp_path = temp_file_named.into_temp_path();
                    let path_buf = temp_path.keep()?;

                    let path_str = path_buf.to_string_lossy().to_string();
                    let url = unsafe { NSURL::fileURLWithPath(&NSString::from_str(&path_str)) };
                    items_to_share.push(unsafe { Retained::cast_unchecked(url) });

                    if let Err(e) = temp_file_manager_clone
                        .lock()
                        .map_err(|e| format!("Failed to lock mutex: {}", e))
                        .and_then(|mut files| {
                            files.push(path_buf);
                            Ok(())
                        })
                    {
                        log::error!("Failed to add file to managed list: {}", e);
                    }
                }
            }

            // Share local files directly from their original paths. This preserves the
            // original filename and avoids creating temporary copies.
            if let Some(file_paths) = options.file_paths {
                for path in file_paths {
                    let url = NSURL::fileURLWithPath(&NSString::from_str(&path));
                    items_to_share.push(unsafe { Retained::cast_unchecked(url) });
                }
            }

            if items_to_share.is_empty() {
                return Err(Error::InvalidArgs(
                    "No content provided to share.".to_string(),
                ));
            }

            autoreleasepool(|_pool| {
                let objects_refs: Vec<&AnyObject> = items_to_share
                    .iter()
                    .map(|obj| obj.as_ref() as &AnyObject)
                    .collect();
                let items_array = NSArray::from_slice(&objects_refs);
                let picker = unsafe {
                    NSSharingServicePicker::initWithItems(
                        NSSharingServicePicker::alloc(),
                        &*items_array,
                    )
                };

                let mtm = MainThreadMarker::new().expect("Main thread marker");
                let delegate = SharePickerDelegate::new(mtm, completion_tx, subject);
                ACTIVE_DELEGATES.with(|delegates| delegates.borrow_mut().push(delegate.retain()));
                unsafe { picker.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };

                let bounds = ns_view.bounds();
                // Web-viewport coordinates are top-left origin; AppKit bounds use a bottom-left origin.
                // Flip the y-axis so the anchor rect describes the same visual area in NSView space.
                let rect = options.anchor.map_or(
                    CGRect {
                        origin: CGPoint {
                            x: bounds.size.width / 2.0,
                            y: bounds.size.height / 2.0,
                        },
                        size: CGSize {
                            width: 0.0,
                            height: 0.0,
                        },
                    },
                    |anchor| CGRect {
                        origin: CGPoint {
                            x: anchor.x,
                            y: bounds.size.height - anchor.y - anchor.height,
                        },
                        size: CGSize {
                            width: anchor.width,
                            height: anchor.height,
                        },
                    },
                );
                unsafe {
                    picker.showRelativeToRect_ofView_preferredEdge(
                        rect,
                        &ns_view,
                        objc2_foundation::NSRectEdge::NSMinYEdge,
                    );
                }
            });
            Ok(())
        })();
        let _ = setup_tx.send(result);
    }) {
        return Err(e.into());
    }

    let share_result = setup_rx.recv()?;
    if let Err(err) = share_result {
        return Err(err);
    }

    match completion_rx.recv_timeout(SHARE_COMPLETION_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
        Err(mpsc::RecvTimeoutError::Disconnected) => Ok(()),
    }
}

/// Retrieves the native `NSView` pointer from the Tauri window, compatible with `raw-window-handle`.
fn get_ns_view<R: Runtime>(window: &Window<R>) -> Result<Retained<NSView>, Error> {
    let window_handle: WindowHandle<'_> = window.window_handle()?;
    if let RawWindowHandle::AppKit(handle) = window_handle.as_raw() {
        let ns_view_ptr = handle.ns_view.as_ptr();
        let ns_view: Retained<NSView> = unsafe { Retained::retain(ns_view_ptr.cast()) }.unwrap();
        Ok(ns_view)
    } else {
        Err(Error::NativeApi(
            "Unsupported window handle type on macOS.".to_string(),
        ))
    }
}

/// Creates a secure temporary file from Base64 data.
fn create_temp_file_for_data(options: &SharedFile) -> Result<NamedTempFile, Error> {
    let decoded_bytes = general_purpose::STANDARD
        .decode(&options.data)
        .map_err(|_| Error::InvalidArgs("Invalid Base64 data.".to_string()))?;
    if decoded_bytes.len() > MAX_FILE_BYTES {
        return Err(Error::InvalidArgs(format!(
            "File '{}' exceeds the maximum size of {} bytes.",
            options.name, MAX_FILE_BYTES
        )));
    }
    // Security: Sanitize the filename to prevent path traversal attacks.
    let sanitized_name = Path::new(&options.name)
        .file_name()
        .ok_or_else(|| Error::InvalidArgs("Invalid file name.".to_string()))?
        .to_str()
        .ok_or_else(|| {
            Error::InvalidArgs("File name contains invalid UTF-8 characters.".to_string())
        })?;
    let temp_dir = std::env::temp_dir();
    let mut temp_file = Builder::new()
        .prefix(&format!("{}-", uuid::Uuid::new_v4()))
        .suffix(&format!("-{}", sanitized_name))
        .tempfile_in(temp_dir)
        .map_err(|e| Error::TempFile(format!("Failed to create temp file: {}", e)))?;
    temp_file
        .write_all(&decoded_bytes)
        .map_err(|e| Error::TempFile(format!("Failed to write to temp file: {}", e)))?;
    Ok(temp_file)
}
