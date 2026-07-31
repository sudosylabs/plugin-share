use tauri_plugin_vnidrop_share::{
    ShareOptions, SharedFile, MAX_FILES, MAX_FILE_NAME_BYTES, MAX_TEXT_BYTES, MAX_URL_BYTES,
};

#[test]
fn public_model_validation_accepts_local_file_bytes_without_url() {
    let options = ShareOptions {
        text: None,
        title: Some("Local file".to_string()),
        url: None,
        files: Some(vec![SharedFile {
            data: "aGVsbG8=".to_string(),
            name: "hello.txt".to_string(),
            mime_type: "text/plain".to_string(),
        }]),
        file_paths: None,
        anchor: None,
    };

    assert!(options.has_shareable_content());
    assert!(options.validate().is_ok());
}

#[test]
fn public_model_validation_rejects_non_web_url_schemes() {
    for url in [
        "file:///tmp/secret.txt",
        "content://provider/item",
        "https:///missing-host",
        " https://example.com",
        "https://example.com\nhttps://evil.example",
    ] {
        assert!(ShareOptions {
            text: None,
            title: None,
            url: Some(url.to_string()),
            files: None,
            file_paths: None,
            anchor: None,
        }
        .validate()
        .is_err());
    }
}

#[test]
fn public_model_validation_enforces_limits() {
    let oversized_text = "a".repeat(MAX_TEXT_BYTES + 1);
    assert!(ShareOptions {
        text: Some(oversized_text),
        title: None,
        url: None,
        files: None,
        file_paths: None,
        anchor: None,
    }
    .validate()
    .is_err());

    let oversized_url = format!("https://example.com/{}", "a".repeat(MAX_URL_BYTES));
    assert!(ShareOptions {
        text: None,
        title: None,
        url: Some(oversized_url),
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
    assert!(ShareOptions {
        text: None,
        title: None,
        url: None,
        files: Some(files),
        file_paths: None,
        anchor: None,
    }
    .validate()
    .is_err());

    assert!(ShareOptions {
        text: None,
        title: None,
        url: None,
        files: Some(vec![SharedFile {
            data: "aGVsbG8=".to_string(),
            name: "a".repeat(MAX_FILE_NAME_BYTES + 1),
            mime_type: "text/plain".to_string(),
        }]),
        file_paths: None,
        anchor: None,
    }
    .validate()
    .is_err());
}
