use serde::{Deserialize, Serialize};

use crate::Error;

pub const MAX_FILES: usize = 16;
pub const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
pub const MAX_TOTAL_FILE_BYTES: usize = 100 * 1024 * 1024;
pub const MAX_TEXT_BYTES: usize = 64 * 1024;
pub const MAX_TITLE_BYTES: usize = 1024;
pub const MAX_URL_BYTES: usize = 4096;
pub const MAX_FILE_NAME_BYTES: usize = 255;
pub const MAX_MIME_TYPE_BYTES: usize = 255;
pub const MAX_FILE_PATH_BYTES: usize = 4096;

/// Represents a rectangle in web-viewport coordinates used to anchor a share popover.
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct ShareAnchor {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Represents a file to be shared, including its content, name, and MIME type.
///
/// The `data` field holds the Base64 encoded content of the file. This approach
/// allows files to be easily passed from the frontend to the Rust backend
/// without needing to manage local file paths directly.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SharedFile {
    pub data: String,
    pub name: String,
    pub mime_type: String,
}

/// Defines the content and options for a native sharing dialog.
///
/// This struct can be used to share text, a title, a URL, and a list of files.
/// All fields are optional, allowing for flexible sharing payloads.
///
/// ## Examples
///
/// To share a simple message and URL:
///
/// ```json
/// {
///   "title": "My Tauri App",
///   "text": "Check out this great app built with Tauri!",
///   "url": "https://tauri.app"
/// }
/// ```
///
/// To share a file (e.g., an image in Base64 format):
///
/// ```json
/// {
///   "files": [
///     {
///       "data": "iVBORw0KGgo...",
///       "name": "my-image.png",
///       "mimeType": "image/png"
///     }
///   ]
/// }
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShareOptions {
    /// Optional text content to include in the share dialog.
    pub text: Option<String>,
    /// Optional title for the share UI. On Android this is the chooser label (`EXTRA_TITLE`);
    /// on iOS, macOS, and Windows it is also used as the subject or share title where those
    /// platforms support one. For Android `EXTRA_SUBJECT` (for example a Drive default file name),
    /// use [`ShareOptions::subject`] instead; when `subject` is omitted on Android, `EXTRA_SUBJECT`
    /// falls back to this field.
    pub title: Option<String>,
    /// Optional subject separate from [`ShareOptions::title`]. Required for correct Android
    /// behavior when `title` must stay a human-readable chooser label: maps to `Intent.EXTRA_SUBJECT`
    /// (for example Google Drive's default file name). When omitted on Android, `EXTRA_SUBJECT`
    /// falls back to `title`. Other platforms ignore this field.
    pub subject: Option<String>,
    /// Optional URL to include in the share dialog.
    pub url: Option<String>,
    /// A list of files to share, each represented by a `SharedFile` struct.
    pub files: Option<Vec<SharedFile>>,
    /// A list of absolute local file paths to share directly from disk, preserving the
    /// original filename instead of creating a temporary copy from Base64 data.
    pub file_paths: Option<Vec<String>>,
    /// Optional source rectangle used to anchor the share popover on iPadOS and macOS.
    pub anchor: Option<ShareAnchor>,
}

impl ShareOptions {
    /// Returns a copy where empty `text`, `title`, `subject` and `url` values are treated as absent.
    pub fn normalized(mut self) -> Self {
        fn non_empty(value: Option<String>) -> Option<String> {
            value.filter(|value| !value.is_empty())
        }
        self.text = non_empty(self.text);
        self.title = non_empty(self.title);
        self.subject = non_empty(self.subject);
        self.url = non_empty(self.url);
        self
    }

    /// Returns true when the payload contains at least one shareable value.
    pub fn has_shareable_content(&self) -> bool {
        self.text.as_ref().is_some_and(|value| !value.is_empty())
            || self.url.as_ref().is_some_and(|value| !value.is_empty())
            || self.files.as_ref().is_some_and(|files| !files.is_empty())
            || self
                .file_paths
                .as_ref()
                .is_some_and(|paths| !paths.is_empty())
    }

    /// Combines text and URL for platforms that expose one plain-text field.
    pub fn combined_text(&self) -> Option<String> {
        match (self.text.as_deref(), self.url.as_deref()) {
            (Some(text), Some(url)) if !text.is_empty() && !url.is_empty() => {
                Some(format!("{text}\n{url}"))
            }
            (Some(text), _) if !text.is_empty() => Some(text.to_string()),
            (_, Some(url)) if !url.is_empty() => Some(url.to_string()),
            _ => None,
        }
    }

    /// Validates caller-provided payload size and URL scheme before native sharing.
    pub fn validate(&self) -> Result<(), Error> {
        validate_optional_string("text", self.text.as_deref(), MAX_TEXT_BYTES)?;
        validate_optional_string("title", self.title.as_deref(), MAX_TITLE_BYTES)?;
        validate_optional_string("subject", self.subject.as_deref(), MAX_TITLE_BYTES)?;

        if let Some(url) = self.url.as_deref().filter(|value| !value.is_empty()) {
            validate_optional_string("url", Some(url), MAX_URL_BYTES)?;
            validate_web_url(url)?;
        }

        let file_count = self.files.as_ref().map(|f| f.len()).unwrap_or(0)
            + self.file_paths.as_ref().map(|p| p.len()).unwrap_or(0);
        if file_count > MAX_FILES {
            return Err(Error::InvalidArgs(format!(
                "Too many files provided. Maximum is {MAX_FILES}."
            )));
        }

        if let Some(files) = self.files.as_ref() {
            let mut total_estimated_bytes = 0usize;
            for file in files {
                validate_optional_string("file name", Some(&file.name), MAX_FILE_NAME_BYTES)?;
                validate_optional_string("mime type", Some(&file.mime_type), MAX_MIME_TYPE_BYTES)?;

                let estimated_bytes = estimate_base64_decoded_len(&file.data)?;
                if estimated_bytes > MAX_FILE_BYTES {
                    return Err(Error::InvalidArgs(format!(
                        "File '{}' exceeds the maximum size of {} bytes.",
                        file.name, MAX_FILE_BYTES
                    )));
                }
                total_estimated_bytes = total_estimated_bytes
                    .checked_add(estimated_bytes)
                    .ok_or_else(|| {
                        Error::InvalidArgs("Total shared file size is too large.".to_string())
                    })?;
            }

            if total_estimated_bytes > MAX_TOTAL_FILE_BYTES {
                return Err(Error::InvalidArgs(format!(
                    "Total shared file size exceeds the maximum of {} bytes.",
                    MAX_TOTAL_FILE_BYTES
                )));
            }
        }

        if let Some(file_paths) = self.file_paths.as_ref() {
            for path in file_paths {
                validate_optional_string("file path", Some(path), MAX_FILE_PATH_BYTES)?;
                validate_file_path(path)?;
            }
        }

        if let Some(anchor) = self.anchor {
            if ![anchor.x, anchor.y, anchor.width, anchor.height]
                .iter()
                .all(|value| value.is_finite())
            {
                return Err(Error::InvalidArgs(
                    "anchor values must be finite numbers.".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Checks that every entry in `file_paths` points to an existing regular file.
    pub fn ensure_file_paths_exist(&self) -> Result<(), Error> {
        for path in self.file_paths.iter().flatten() {
            if !std::path::Path::new(path).is_file() {
                return Err(Error::InvalidArgs(format!("File not found: {path}")));
            }
        }
        Ok(())
    }
}

/// Normalizes and validates options before they reach a native share implementation.
pub(crate) fn prepare_share_options(options: ShareOptions) -> Result<ShareOptions, Error> {
    let options = options.normalized();
    if !options.has_shareable_content() {
        return Err(Error::InvalidArgs(
            "No content provided to share.".to_string(),
        ));
    }
    options.validate()?;
    options.ensure_file_paths_exist()?;
    Ok(options)
}

/// Removes the ASCII whitespace that Base64 encoders commonly insert as line breaks.
pub(crate) fn strip_base64_whitespace(data: &str) -> String {
    data.chars()
        .filter(|char| !matches!(char, ' ' | '\t' | '\n' | '\r' | '\x0C'))
        .collect()
}

fn validate_file_path(path: &str) -> Result<(), Error> {
    if path.is_empty() || !std::path::Path::new(path).is_absolute() {
        return Err(Error::InvalidArgs(
            "file path must be an absolute path.".to_string(),
        ));
    }
    Ok(())
}

fn validate_optional_string(
    field: &str,
    value: Option<&str>,
    max_bytes: usize,
) -> Result<(), Error> {
    if let Some(value) = value {
        if value.len() > max_bytes {
            return Err(Error::InvalidArgs(format!(
                "{field} exceeds the maximum length of {max_bytes} bytes."
            )));
        }
    }
    Ok(())
}

fn validate_web_url(url: &str) -> Result<(), Error> {
    if url.trim() != url
        || url
            .chars()
            .any(|char| char.is_control() || char.is_whitespace())
    {
        return Err(Error::InvalidArgs(
            "Only well-formed http:// and https:// URLs can be shared as URLs.".to_string(),
        ));
    }

    let Some((scheme, rest)) = url.split_once("://") else {
        return Err(Error::InvalidArgs(
            "Only http:// and https:// URLs can be shared as URLs.".to_string(),
        ));
    };

    if scheme.eq_ignore_ascii_case("https") || scheme.eq_ignore_ascii_case("http") {
        let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
        let host_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = if let Some(ipv6_end) = host_port.find(']') {
            host_port.get(..=ipv6_end).unwrap_or_default()
        } else {
            host_port.split(':').next().unwrap_or_default()
        };
        if !host.is_empty() && host != "[]" && !host.starts_with(':') {
            return Ok(());
        }
    }

    Err(Error::InvalidArgs(
        "Only http:// and https:// URLs can be shared as URLs.".to_string(),
    ))
}

fn estimate_base64_decoded_len(data: &str) -> Result<usize, Error> {
    let data = strip_base64_whitespace(data);
    let len = data.len();
    if len == 0 {
        return Ok(0);
    }
    if !len.is_multiple_of(4) {
        return Err(Error::InvalidArgs("Invalid Base64 data.".to_string()));
    }

    let padding = data
        .as_bytes()
        .iter()
        .rev()
        .take_while(|&&b| b == b'=')
        .count();
    if padding > 2 {
        return Err(Error::InvalidArgs("Invalid Base64 data.".to_string()));
    }

    Ok((len / 4) * 3 - padding)
}

/// The result type for the `can_share` command.
///
/// A `true` value indicates that the current platform supports native sharing.
/// The `can_share` command returns `true` on Windows, macOS, and mobile platforms,
/// and `false` on other desktop platforms (such as Linux), which have no native sharing dialog.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanShareResult {
    pub value: bool,
}

#[cfg(test)]
mod tests {
    use super::{
        estimate_base64_decoded_len, validate_web_url, ShareAnchor, ShareOptions, SharedFile,
        MAX_FILES, MAX_TEXT_BYTES, MAX_TITLE_BYTES,
    };

    /// Shared with the TypeScript, Kotlin and Swift test suites.
    const VALIDATION_VECTORS: &str = include_str!("../tests/fixtures/validation-vectors.json");

    #[test]
    fn shared_validation_vectors() {
        let vectors: serde_json::Value = serde_json::from_str(VALIDATION_VECTORS).unwrap();

        for case in vectors["urls"].as_array().unwrap() {
            let url = case["url"].as_str().unwrap();
            let valid = case["valid"].as_bool().unwrap();
            assert_eq!(validate_web_url(url).is_ok(), valid, "url {url:?}");
        }

        for case in vectors["base64"].as_array().unwrap() {
            let data = case["data"].as_str().unwrap();
            match case["decodedBytes"].as_u64() {
                Some(bytes) => assert_eq!(
                    estimate_base64_decoded_len(data).ok(),
                    Some(bytes as usize),
                    "base64 {data:?}"
                ),
                None => assert!(
                    estimate_base64_decoded_len(data).is_err(),
                    "base64 {data:?}"
                ),
            }
        }

        #[cfg(unix)]
        for case in vectors["filePaths"].as_array().unwrap() {
            let path = case["path"].as_str().unwrap();
            let valid = case["posixValid"].as_bool().unwrap();
            assert_eq!(
                super::validate_file_path(path).is_ok(),
                valid,
                "path {path:?}"
            );
        }
    }

    #[test]
    fn normalized_treats_empty_strings_as_absent() {
        let data = ShareOptions {
            text: Some(String::new()),
            title: Some(String::new()),
            subject: Some(String::new()),
            url: Some(String::new()),
            files: None,
            file_paths: None,
            anchor: None,
        }
        .normalized();
        assert!(
            data.text.is_none()
                && data.title.is_none()
                && data.subject.is_none()
                && data.url.is_none()
        );
    }

    #[test]
    fn validation_rejects_non_finite_anchor() {
        let mut data = options(Some("hello"), None, None);
        data.anchor = Some(ShareAnchor {
            x: f64::NAN,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        });
        assert!(data.validate().is_err());
    }

    #[test]
    fn missing_file_paths_are_reported() {
        let mut data = options(None, None, None);
        data.file_paths = Some(vec!["/definitely/not/here.txt".to_string()]);
        assert!(data.ensure_file_paths_exist().is_err());

        let file = tempfile::NamedTempFile::new().unwrap();
        data.file_paths = Some(vec![file.path().to_string_lossy().into_owned()]);
        assert!(data.ensure_file_paths_exist().is_ok());
    }

    fn options(
        text: Option<&str>,
        url: Option<&str>,
        files: Option<Vec<SharedFile>>,
    ) -> ShareOptions {
        ShareOptions {
            text: text.map(ToString::to_string),
            title: None,
            subject: None,
            url: url.map(ToString::to_string),
            files,
            file_paths: None,
            anchor: None,
        }
    }

    #[test]
    fn empty_options_are_not_shareable() {
        assert!(!options(None, None, None).has_shareable_content());
        assert!(!options(Some(""), Some(""), Some(Vec::new())).has_shareable_content());
    }

    #[test]
    fn text_url_or_files_are_shareable() {
        let file = SharedFile {
            data: "aGVsbG8=".to_string(),
            name: "hello.txt".to_string(),
            mime_type: "text/plain".to_string(),
        };

        assert!(options(Some("hello"), None, None).has_shareable_content());
        assert!(options(None, Some("https://example.com"), None).has_shareable_content());
        assert!(options(None, None, Some(vec![file])).has_shareable_content());
    }

    #[test]
    fn combined_text_preserves_text_and_url() {
        let data = options(Some("hello"), Some("https://example.com"), None);
        assert_eq!(
            data.combined_text().as_deref(),
            Some("hello\nhttps://example.com")
        );
    }

    #[test]
    fn validation_accepts_http_and_https_urls() {
        assert!(options(None, Some("https://example.com"), None)
            .validate()
            .is_ok());
        assert!(options(None, Some("http://example.com"), None)
            .validate()
            .is_ok());
    }

    #[test]
    fn validation_rejects_non_web_url_schemes() {
        assert!(options(None, Some("file:///etc/passwd"), None)
            .validate()
            .is_err());
        assert!(options(None, Some("custom-scheme://value"), None)
            .validate()
            .is_err());
        assert!(options(None, Some("https:///missing-host"), None)
            .validate()
            .is_err());
        assert!(options(
            None,
            Some("https://example.com\nhttps://evil.example"),
            None
        )
        .validate()
        .is_err());
    }

    #[test]
    fn validation_rejects_oversized_text_and_too_many_files() {
        let oversized_text = "a".repeat(MAX_TEXT_BYTES + 1);
        assert!(ShareOptions {
            text: Some(oversized_text),
            title: None,
            subject: None,
            url: None,
            files: None,
            file_paths: None,
            anchor: None,
        }
        .validate()
        .is_err());

        let oversized_subject = "a".repeat(MAX_TITLE_BYTES + 1);
        assert!(ShareOptions {
            text: None,
            title: None,
            subject: Some(oversized_subject),
            url: None,
            files: None,
            file_paths: None,
            anchor: None,
        }
        .validate()
        .is_err());

        let files = (0..=MAX_FILES)
            .map(|index| SharedFile {
                data: "aGVsbG8=".to_string(),
                name: format!("hello-{index}.txt"),
                mime_type: "text/plain".to_string(),
            })
            .collect();
        assert!(options(None, None, Some(files)).validate().is_err());
    }
}
