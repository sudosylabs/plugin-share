# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0-rc.5] - 2026-10-23

### Added

- Optional `subject` on `ShareOptions` (Rust) and `ShareData` (TypeScript), mapped to `EXTRA_SUBJECT` on Android. Other platforms ignore it. ([#24], [#25])

### Changed

- **Breaking (Rust):** `ShareOptions` has a new `subject` field, so struct literals must set `subject: None` or `Some(...)`. See [Upgrading from 1.0.0-rc.4](README.md#upgrading-from-100-rc4).
- Android: `EXTRA_TITLE` is now set from `title` only. `EXTRA_SUBJECT` uses `subject` and falls back to `title` when `subject` is not set, so callers that only set `title` behave as before.

### Fixed

- Android: sharing a file to Google Drive and similar targets used the chooser title as the file name instead of the real name; set `subject` to the desired file name. ([#24], [#25])

## [1.0.0-rc.4] - 2026-10-22

### Changed

- Publish to npm with trusted publishing, and allow dispatching the release workflow for an existing tag.

### Fixed

- Add the repository metadata npm provenance requires.

## [1.0.0-rc.3]

### Changed

- CI runs every test suite and gates releases; JS dependencies install with a frozen Bun lockfile.

### Fixed

- iOS: fix a share state race, temp file leaks and Base64 decoding; make the plugin entry point public for Xcode 27. ([#15])
- Android: avoid a FileProvider clash and make share state thread-safe.
- macOS: centre the share picker when no anchor is given.
- JavaScript: validation matches the native implementations and the share payload is typed.
- Rust: unify validation and harden desktop share flows.

## [1.0.0-rc.2]

### Added

- Share local files by path (`filePaths`) to preserve original file names.
- Optional `anchor` for iPadOS and macOS share popovers.
- Manual package publish recovery in the release workflow.

### Fixed

- iOS: present the share sheet as an iPad popover.
- Android: share `filePaths` via FileProvider roots with a cache fallback, and restore text extras.
- Windows: share source file paths directly with a read-only token.
- macOS: stop concatenating text and URL in link-only shares.

## [1.0.0-rc.1]

### Fixed

- Android: fix share completion and link intents.

## [1.0.0-rc]

### Added

- Share plugin example app and release validation test coverage.
- Prerelease release tags are accepted.

### Changed

- Harden share payload validation.

[Unreleased]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc.5...HEAD
[1.0.0-rc.5]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc.4...v1.0.0-rc.5
[1.0.0-rc.4]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc.3...v1.0.0-rc.4
[1.0.0-rc.3]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc.2...v1.0.0-rc.3
[1.0.0-rc.2]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc.1...v1.0.0-rc.2
[1.0.0-rc.1]: https://github.com/sudosylabs/plugin-share/compare/v1.0.0-rc...v1.0.0-rc.1
[1.0.0-rc]: https://github.com/sudosylabs/plugin-share/compare/v0.2.2...v1.0.0-rc
[#15]: https://github.com/sudosylabs/plugin-share/issues/15
[#24]: https://github.com/sudosylabs/plugin-share/issues/24
[#25]: https://github.com/sudosylabs/plugin-share/pull/25
