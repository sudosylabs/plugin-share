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
    if let error = validateStringLength("subject", args.subject, maxBytes: maxTitleBytes) {
        return error
    }
    if let error = validateStringLength("url", args.url, maxBytes: maxURLBytes) {
        return error
    }

    // Empty strings are treated as absent.
    if let urlString = nonEmpty(args.url) {
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
            if !isValidFilePath(path) {
                return "File paths must be non-empty absolute paths."
            }
        }
    }

    return nil
}

/// Returns `nil` for `nil` or empty strings, which the share contract treats as absent.
public func nonEmpty(_ value: String?) -> String? {
    guard let value = value, !value.isEmpty else {
        return nil
    }
    return value
}

/// Removes ASCII whitespace (space, \t, \n, \r, \f) from Base64 input.
public func normalizeBase64(_ data: String) -> String {
    var scalars = String.UnicodeScalarView()
    scalars.append(contentsOf: data.unicodeScalars.filter { !isASCIIWhitespace($0) })
    return String(scalars)
}

public func estimateBase64DecodedSize(_ data: String) -> Int? {
    let normalized = normalizeBase64(data)
    let length = normalized.utf8.count
    if length == 0 {
        return 0
    }
    if length % 4 != 0 {
        return nil
    }

    let padding = normalized.utf8.reversed().prefix { $0 == UInt8(ascii: "=") }.count
    if padding > 2 {
        return nil
    }

    return (length / 4) * 3 - padding
}

/// Decodes Base64 data after stripping ASCII whitespace, matching `estimateBase64DecodedSize`.
public func decodeBase64(_ data: String) -> Data? {
    return Data(base64Encoded: normalizeBase64(data))
}

/// A file path is valid when it is non-empty and absolute.
public func isValidFilePath(_ path: String) -> Bool {
    return path.hasPrefix("/")
}

/// Returns true when `path` exists and is a regular file (symlinks are resolved).
public func isExistingRegularFile(atPath path: String) -> Bool {
    let url = URL(fileURLWithPath: path)
    guard let values = try? url.resolvingSymlinksInPath().resourceValues(forKeys: [.isRegularFileKey]) else {
        return false
    }
    return values.isRegularFile == true
}

public func isValidWebURL(_ urlString: String) -> Bool {
    return validateWebURL(urlString) == nil
}

func validateWebURL(_ urlString: String) -> String? {
    let scalars = urlString.unicodeScalars
    if scalars.contains(where: { isControlScalar($0) || isWhitespaceScalar($0) }) {
        return "Only well-formed http:// and https:// URLs can be shared as URLs."
    }

    let invalid = "Only http:// and https:// URLs can be shared as URLs."
    let all = Array(scalars)
    let separator: [Unicode.Scalar] = [":", "/", "/"]
    guard let separatorIndex = all.indices.first(where: { all[$0...].starts(with: separator) }) else {
        return invalid
    }

    let scheme = String(String.UnicodeScalarView(all[..<separatorIndex]))
    guard asciiCaseInsensitiveEquals(scheme, "http") || asciiCaseInsensitiveEquals(scheme, "https") else {
        return invalid
    }

    let rest = all[(separatorIndex + separator.count)...]
    let authority = rest.prefix { $0 != "/" && $0 != "?" && $0 != "#" }

    let hostPort: ArraySlice<Unicode.Scalar>
    if let lastAt = authority.lastIndex(of: "@") {
        hostPort = authority[(lastAt + 1)...]
    } else {
        hostPort = authority
    }

    let host: ArraySlice<Unicode.Scalar>
    if let closingBracket = hostPort.firstIndex(of: "]") {
        host = hostPort[...closingBracket]
    } else {
        host = hostPort.prefix { $0 != ":" }
    }

    if host.isEmpty || Array(host) == ["[", "]"] || host.first == ":" {
        return invalid
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

private func isASCIIWhitespace(_ scalar: Unicode.Scalar) -> Bool {
    switch scalar.value {
    case 0x20, 0x09, 0x0A, 0x0D, 0x0C:
        return true
    default:
        return false
    }
}

/// Unicode general category Cc.
private func isControlScalar(_ scalar: Unicode.Scalar) -> Bool {
    let value = scalar.value
    return value <= 0x1F || (0x7F...0x9F).contains(value)
}

/// Unicode White_Space property.
private func isWhitespaceScalar(_ scalar: Unicode.Scalar) -> Bool {
    switch scalar.value {
    case 0x09...0x0D, 0x20, 0x85, 0xA0, 0x1680, 0x2000...0x200A, 0x2028, 0x2029, 0x202F, 0x205F, 0x3000:
        return true
    default:
        return false
    }
}

private func asciiCaseInsensitiveEquals(_ value: String, _ expected: String) -> Bool {
    let lhs = Array(value.unicodeScalars)
    let rhs = Array(expected.unicodeScalars)
    guard lhs.count == rhs.count else {
        return false
    }
    return zip(lhs, rhs).allSatisfy { asciiLowercased($0) == asciiLowercased($1) }
}

private func asciiLowercased(_ scalar: Unicode.Scalar) -> Unicode.Scalar {
    if (0x41...0x5A).contains(scalar.value), let lowered = Unicode.Scalar(scalar.value + 0x20) {
        return lowered
    }
    return scalar
}
