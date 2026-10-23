import { invoke } from "@tauri-apps/api/core";

const MAX_FILES = 16;
const MAX_FILE_BYTES = 50 * 1024 * 1024;
const MAX_TOTAL_FILE_BYTES = 100 * 1024 * 1024;
const MAX_TEXT_BYTES = 64 * 1024;
const MAX_TITLE_BYTES = 1024;
const MAX_URL_BYTES = 4096;
const MAX_FILE_NAME_BYTES = 255;
const MAX_MIME_TYPE_BYTES = 255;
const MAX_FILE_PATH_BYTES = 4096;
const DEFAULT_MIME_TYPE = "application/octet-stream";

// Unicode control characters (Cc) and White_Space, matching Rust's
// `char::is_control` and `char::is_whitespace` used by the native validation.
const URL_FORBIDDEN_CHARS =
  /[\u0000-\u0020\u007f-\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]/u;

/**
 * Represents the content to be shared, similar to the Web Share API's ShareData dictionary.
 *
 * Example:
 * ```ts
 * const shareData: ShareData = {
 *   title: "Check this out!",
 *   text: "Here's an interesting article.",
 *   url: "https://example.com/article",
 *   files: [myFile]
 * };
 * ```
 */
export interface ShareRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface ShareData {
  /** Optional array of File objects to share (e.g., images, PDFs). */
  files?: File[];
  /**
   * Optional array of absolute local file paths to share. The file is shared
   * directly from disk, preserving its original filename, instead of being
   * copied from base64 content. Each path must exist when `share` is called.
   */
  filePaths?: string[];
  /** Optional text content to be shared. */
  text?: string;
  /** Optional title describing the shared content. */
  title?: string;
  /**
   * Optional subject separate from `title`. On Android this is required when `title` must
   * differ from the value share targets use as a file name or subject line: it maps to
   * `EXTRA_SUBJECT` (e.g. Google Drive's default file name). If omitted on Android,
   * `EXTRA_SUBJECT` falls back to `title`. Other platforms ignore this field.
   */
  subject?: string;
  /** Optional URL to be shared. */
  url?: string;
  /**
   * Optional source rectangle for iPadOS and macOS popovers, in web-viewport
   * coordinates (pixels from the top-left).
   */
  anchor?: ShareRect;
}

function hasShareableContent(data: ShareData): boolean {
  return Boolean(
    data.text ||
      data.url ||
      (data.files && data.files.length > 0) ||
      (data.filePaths && data.filePaths.length > 0)
  );
}

function byteLength(value: string): number {
  return new TextEncoder().encode(value).length;
}

/**
 * Mirrors the native URL check: an http(s) URL with a non-empty host and no
 * whitespace or control characters.
 */
function validateWebUrl(url: string): boolean {
  if (URL_FORBIDDEN_CHARS.test(url)) {
    return false;
  }
  const schemeSeparator = url.indexOf("://");
  if (schemeSeparator === -1) {
    return false;
  }
  const scheme = url.slice(0, schemeSeparator).toLowerCase();
  if (scheme !== "http" && scheme !== "https") {
    return false;
  }
  const authority = url.slice(schemeSeparator + 3).split(/[/?#]/u, 1)[0];
  const hostPort = authority.slice(authority.lastIndexOf("@") + 1);
  const ipv6End = hostPort.indexOf("]");
  const host = ipv6End === -1 ? hostPort.split(":", 1)[0] : hostPort.slice(0, ipv6End + 1);
  return host.length > 0 && host !== "[]" && !host.startsWith(":");
}

function isAbsolutePath(path: string): boolean {
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/u.test(path) || path.startsWith("\\\\");
}

function validateShareData(data: ShareData): void {
  if (data.text && byteLength(data.text) > MAX_TEXT_BYTES) {
    throw new TypeError(`text exceeds the maximum length of ${MAX_TEXT_BYTES} bytes.`);
  }
  if (data.title && byteLength(data.title) > MAX_TITLE_BYTES) {
    throw new TypeError(`title exceeds the maximum length of ${MAX_TITLE_BYTES} bytes.`);
  }
  if (data.subject && byteLength(data.subject) > MAX_TITLE_BYTES) {
    throw new TypeError(`subject exceeds the maximum length of ${MAX_TITLE_BYTES} bytes.`);
  }
  if (data.url) {
    if (byteLength(data.url) > MAX_URL_BYTES) {
      throw new TypeError(`url exceeds the maximum length of ${MAX_URL_BYTES} bytes.`);
    }
    if (!validateWebUrl(data.url)) {
      throw new TypeError("Only http:// and https:// URLs can be shared as URLs.");
    }
  }

  const fileCount = (data.files?.length ?? 0) + (data.filePaths?.length ?? 0);
  if (fileCount > MAX_FILES) {
    throw new TypeError(`Too many files provided. Maximum is ${MAX_FILES}.`);
  }

  // Same checks, order and messages as the native validation.
  let totalBytes = 0;
  for (const file of data.files ?? []) {
    if (byteLength(file.name) > MAX_FILE_NAME_BYTES) {
      throw new TypeError(
        `file name exceeds the maximum length of ${MAX_FILE_NAME_BYTES} bytes.`
      );
    }
    if (byteLength(file.type || DEFAULT_MIME_TYPE) > MAX_MIME_TYPE_BYTES) {
      throw new TypeError(
        `mime type exceeds the maximum length of ${MAX_MIME_TYPE_BYTES} bytes.`
      );
    }
    if (file.size > MAX_FILE_BYTES) {
      throw new TypeError(
        `File '${file.name}' exceeds the maximum size of ${MAX_FILE_BYTES} bytes.`
      );
    }
    totalBytes += file.size;
  }
  if (totalBytes > MAX_TOTAL_FILE_BYTES) {
    throw new TypeError(
      `Total shared file size exceeds the maximum of ${MAX_TOTAL_FILE_BYTES} bytes.`
    );
  }

  for (const path of data.filePaths ?? []) {
    if (byteLength(path) > MAX_FILE_PATH_BYTES) {
      throw new TypeError(
        `file path exceeds the maximum length of ${MAX_FILE_PATH_BYTES} bytes.`
      );
    }
    if (!isAbsolutePath(path)) {
      throw new TypeError("file path must be an absolute path.");
    }
  }

  if (data.anchor) {
    const { x, y, width, height } = data.anchor;
    if (![x, y, width, height].every(Number.isFinite)) {
      throw new TypeError("anchor values must be finite numbers.");
    }
  }
}

/** The options object sent to the native `share` command. */
interface SharePayload {
  text?: string;
  title?: string;
  subject?: string;
  url?: string;
  files?: { data: string; name: string; mimeType: string }[];
  filePaths?: string[];
  anchor?: ShareRect;
}

/**
 * Checks whether the native sharing capability is available for the given data.
 *
 * On mobile platforms, this will typically return `true`.
 * This is useful for feature detection before attempting to share.
 *
 * Example:
 * ```ts
 * if (await canShare({ text: "Hello World" })) {
 *   console.log("Sharing is supported!");
 * } else {
 *   console.log("Sharing is not available on this platform.");
 * }
 * ```
 *
 * @param data Optional ShareData to check shareability for.
 * @returns Promise resolving to `true` if sharing is possible.
 */
export async function canShare(data?: ShareData): Promise<boolean> {
  if (data && !hasShareableContent(data)) {
    return false;
  }
  if (data) {
    try {
      validateShareData(data);
    } catch {
      return false;
    }
  }

  const result = await invoke<{ value: boolean }>("plugin:vnidrop-share|can_share");
  return result.value === true;
}

/**
 * Manually triggers cleanup of temporary files created by the plugin.
 *
 * Useful when files are generated during sharing but you want to remove them
 * immediately after to save storage space. Rejects while a share is still in
 * progress.
 *
 * Example:
 * ```ts
 * await cleanup();
 * console.log("Temporary share files removed.");
 * ```
 *
 * @returns Promise resolving when cleanup is complete.
 */
export async function cleanup(): Promise<void> {
  await invoke("plugin:vnidrop-share|cleanup");
}

/**
 * Converts a `File` object to a Base64-encoded string (without the Data URL prefix).
 *
 * Example:
 * ```ts
 * const base64Data = await fileToBase64(myFile);
 * console.log(base64Data.slice(0, 50)); // preview first 50 chars
 * ```
 *
 * @param file File to convert.
 * @returns Promise resolving to Base64 string.
 */
async function fileToBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => {
      const base64String = (reader.result as string).split(",")[1];
      resolve(base64String);
    };
    reader.onerror = (error) => reject(error);
  });
}

/**
 * Opens the native share dialog to share text, URLs, and/or files.
 *
 * Example:
 * ```ts
 * await share({
 *   title: "My Photo",
 *   text: "Check out this picture!",
 *   files: [myImageFile]
 * });
 * console.log("Share dialog closed.");
 * ```
 *
 * The promise also resolves when the user dismisses the dialog without
 * sharing; platforms do not report cancellation consistently.
 *
 * @param data Content to share.
 * @returns Promise resolving when the share dialog is closed.
 */
export async function share(data: ShareData): Promise<void> {
  if (!hasShareableContent(data)) {
    throw new TypeError("No content provided to share.");
  }
  validateShareData(data);

  // Empty strings are treated as absent, as they are natively.
  const payload: SharePayload = {
    text: data.text || undefined,
    title: data.title || undefined,
    subject: data.subject || undefined,
    url: data.url || undefined,
  };

  if (data.files && data.files.length > 0) {
    payload.files = await Promise.all(
      data.files.map(async (file) => ({
        data: await fileToBase64(file),
        name: file.name,
        mimeType: file.type || DEFAULT_MIME_TYPE,
      }))
    );
  }

  if (data.filePaths && data.filePaths.length > 0) {
    payload.filePaths = data.filePaths;
  }

  if (data.anchor) {
    payload.anchor = data.anchor;
  }

  await invoke("plugin:vnidrop-share|share", { options: payload });
}
