use serde::{Deserialize, Serialize};

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
///       "data": "data:image/png;base64,iVBORw0KGgo...",
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
    use super::{ShareOptions, SharedFile};

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
}
