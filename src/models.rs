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
///   "url": "[https://tauri.app](https://tauri.app)"
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
    /// Optional title for the share dialog. (This is mainly used on Android)
    pub title: Option<String>,
    /// Optional URL to include in the share dialog.
    pub url: Option<String>,
    /// A list of files to share, each represented by a `SharedFile` struct.
    pub files: Option<Vec<SharedFile>>,
}

impl ShareOptions {
    /// Returns true when the payload contains at least one shareable value.
    pub fn has_shareable_content(&self) -> bool {
        self.text.as_ref().is_some_and(|value| !value.is_empty())
            || self.url.as_ref().is_some_and(|value| !value.is_empty())
            || self.files.as_ref().is_some_and(|files| !files.is_empty())
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

        if let Some(url) = self.url.as_deref().filter(|value| !value.is_empty()) {
            validate_optional_string("url", Some(url), MAX_URL_BYTES)?;
            validate_web_url(url)?;
        }

        if let Some(files) = self.files.as_ref() {
            if files.len() > MAX_FILES {
                return Err(Error::InvalidArgs(format!(
                    "Too many files provided. Maximum is {MAX_FILES}."
                )));
            }

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

        Ok(())
    }
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
    let len = data.len();
    if len == 0 {
        return Ok(0);
    }
    if len % 4 != 0 {
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
/// The [`crate::commands::can_share`] command will return `true` on Windows, macOS, and mobile platforms,
/// and `false` on Linux since there is no native sharing dialog available.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CanShareResult {
    pub value: bool,
}

#[cfg(test)]
mod tests {
    use super::{ShareOptions, SharedFile, MAX_FILES, MAX_TEXT_BYTES};

    fn options(
        text: Option<&str>,
        url: Option<&str>,
        files: Option<Vec<SharedFile>>,
    ) -> ShareOptions {
        ShareOptions {
            text: text.map(ToString::to_string),
            title: None,
            url: url.map(ToString::to_string),
            files,
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
            url: None,
            files: None,
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
