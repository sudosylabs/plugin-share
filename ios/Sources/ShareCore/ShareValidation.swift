import Foundation

public let maxFiles = 16
public let maxFileBytes = 50 * 1024 * 1024
public let maxTotalFileBytes = 100 * 1024 * 1024
public let maxTextBytes = 64 * 1024
public let maxTitleBytes = 1024
public let maxURLBytes = 4096
public let maxFileNameBytes = 255
public let maxMimeTypeBytes = 255
public let maxFilePathBytes = 4096

public func validateShareOptions(_ args: ShareOptions) -> String? {
    if let error = validateStringLength("text", args.text, maxBytes: maxTextBytes) {
        return error
    }
    if let error = validateStringLength("title", args.title, maxBytes: maxTitleBytes) {
        return error
    }
    if let error = validateStringLength("url", args.url, maxBytes: maxURLBytes) {
        return error
    }

    if let urlString = args.url, !urlString.isEmpty {
        if let error = validateWebURL(urlString) {
            return error
        }
    }

    let fileCount = (args.files ?? []).count + (args.filePaths ?? []).count
    if fileCount > maxFiles {
        return "Too many files provided. Maximum is \(maxFiles)."
    }

    var totalEstimatedBytes = 0
    if let files = args.files {
        for file in files {
            if let error = validateStringLength("file name", file.name, maxBytes: maxFileNameBytes) {
                return error
            }
            if let error = validateStringLength("mime type", file.mimeType, maxBytes: maxMimeTypeBytes) {
                return error
            }

            guard let estimatedBytes = estimateBase64DecodedSize(file.data) else {
                return "Invalid Base64 data."
            }
            if estimatedBytes > maxFileBytes {
                return "File '\(file.name)' exceeds the maximum size of \(maxFileBytes) bytes."
            }
            totalEstimatedBytes += estimatedBytes
            if totalEstimatedBytes > maxTotalFileBytes {
                return "Total shared file size exceeds the maximum of \(maxTotalFileBytes) bytes."
            }
        }
    }

    if let filePaths = args.filePaths {
        for path in filePaths {
            if let error = validateStringLength("file path", path, maxBytes: maxFilePathBytes) {
                return error
            }
        }
    }

    return nil
}

public func estimateBase64DecodedSize(_ data: String) -> Int? {
    let normalized = data.filter { !$0.isWhitespace }
    if normalized.isEmpty {
        return 0
    }
    if normalized.count % 4 != 0 {
        return nil
    }

    let padding = normalized.reversed().prefix { $0 == "=" }.count
    if padding > 2 {
        return nil
    }

    return (normalized.count / 4) * 3 - padding
}

func validateWebURL(_ urlString: String) -> String? {
    if urlString.trimmingCharacters(in: .whitespacesAndNewlines) != urlString ||
        urlString.unicodeScalars.contains(where: { CharacterSet.whitespacesAndNewlines.contains($0) || CharacterSet.controlCharacters.contains($0) }) {
        return "Only well-formed http:// and https:// URLs can be shared as URLs."
    }

    guard let components = URLComponents(string: urlString),
          let scheme = components.scheme?.lowercased(),
          scheme == "http" || scheme == "https",
          let host = components.host,
          !host.isEmpty else {
        return "Only http:// and https:// URLs can be shared as URLs."
    }

    return nil
}

func validateStringLength(_ field: String, _ value: String?, maxBytes: Int) -> String? {
    guard let value = value else {
        return nil
    }
    if value.lengthOfBytes(using:.utf8) > maxBytes {
        return "\(field) exceeds the maximum length of \(maxBytes) bytes."
    }
    return nil
}
