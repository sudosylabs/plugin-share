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
   * Optional array of local file paths to share. The file is shared directly
   * from disk, preserving its original filename, instead of being copied from
   * base64 content. Mutually exclusive with `files` in most cases.
   */
  filePaths?: string[];
  /** Optional text content to be shared. */
  text?: string;
  /** Optional title describing the shared content. */
  title?: string;
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

function validateWebUrl(url: string): boolean {
  if (url.trim() !== url || /[\s\u0000-\u001f\u007f]/u.test(url)) {
    return false;
  }
  const schemeSeparator = url.indexOf("://");
  if (schemeSeparator === -1) {
    return false;
  }
  const authority = url
    .slice(schemeSeparator + 3)
    .split(/[/?#]/u, 1)[0]
    .split("@")
    .pop() ?? "";
  if (!authority || authority.startsWith(":")) {
    return false;
  }
  try {
    const parsedUrl = new URL(url);
    return (
      (parsedUrl.protocol === "http:" || parsedUrl.protocol === "https:") &&
      parsedUrl.host.length > 0
    );
  } catch {
    return false;
  }
}

function validateShareData(data: ShareData): void {
  if (data.text && byteLength(data.text) > MAX_TEXT_BYTES) {
    throw new TypeError(`text exceeds the maximum length of ${MAX_TEXT_BYTES} bytes.`);
  }
  if (data.title && byteLength(data.title) > MAX_TITLE_BYTES) {
    throw new TypeError(`title exceeds the maximum length of ${MAX_TITLE_BYTES} bytes.`);
  }
  if (data.url !== undefined) {
    if (byteLength(data.url) > MAX_URL_BYTES) {
      throw new TypeError(`url exceeds the maximum length of ${MAX_URL_BYTES} bytes.`);
    }
    if (data.url.length > 0 && !validateWebUrl(data.url)) {
      throw new TypeError("Only http:// and https:// URLs can be shared as URLs.");
    }
  }

  const fileCount = (data.files?.length ?? 0) + (data.filePaths?.length ?? 0);
  if (fileCount === 0) {
    return;
  }

  if (fileCount > MAX_FILES) {
    throw new TypeError(`Too many files provided. Maximum is ${MAX_FILES}.`);
  }

  if (data.files && data.files.length > 0) {
    const totalBytes = data.files.reduce((total, file) => total + file.size, 0);
    if (totalBytes > MAX_TOTAL_FILE_BYTES) {
      throw new TypeError(
        `Total shared file size exceeds the maximum of ${MAX_TOTAL_FILE_BYTES} bytes.`
      );
    }

    const oversizedFile = data.files.find((file) => file.size > MAX_FILE_BYTES);
    if (oversizedFile) {
      throw new TypeError(
        `File '${oversizedFile.name}' exceeds the maximum size of ${MAX_FILE_BYTES} bytes.`
      );
    }

    const oversizedName = data.files.find(
      (file) => byteLength(file.name) > MAX_FILE_NAME_BYTES
    );
    if (oversizedName) {
      throw new TypeError(
        `File name exceeds the maximum length of ${MAX_FILE_NAME_BYTES} bytes.`
      );
    }

    const oversizedMimeType = data.files.find((file) => {
      const type = file.type || "application/octet-stream";
      return byteLength(type) > MAX_MIME_TYPE_BYTES;
    });
    if (oversizedMimeType) {
      throw new TypeError(
        `mime type exceeds the maximum length of ${MAX_MIME_TYPE_BYTES} bytes.`
      );
    }
  }

  if (data.filePaths && data.filePaths.length > 0) {
    const oversizedPath = data.filePaths.find(
      (path) => byteLength(path) > MAX_FILE_PATH_BYTES
    );
    if (oversizedPath) {
      throw new TypeError(
        `file path exceeds the maximum length of ${MAX_FILE_PATH_BYTES} bytes.`
      );
    }
  }
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

  const result = (await invoke("plugin:vnidrop-share|can_share")) as {
    value: any;
  };
  return result.value === true || result.value === "true";
}

/**
 * Manually triggers cleanup of temporary files created by the plugin.
 *
 * Useful when files are generated during sharing but you want to remove them
 * immediately after to save storage space.
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
 * @param data Content to share.
 * @returns Promise resolving when the share dialog is closed.
 */
export async function share(data: ShareData): Promise<void> {
  if (!hasShareableContent(data)) {
    throw new TypeError("No content provided to share.");
  }
  validateShareData(data);

  const payload: any = {
    text: data.text,
    title: data.title,
    url: data.url,
  };

  if (data.files && data.files.length > 0) {
    payload.files = await Promise.all(
      data.files.map(async (file) => ({
        data: await fileToBase64(file),
        name: file.name,
        mimeType: file.type || "application/octet-stream",
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
