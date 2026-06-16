import XCTest
@testable import ShareCore

final class ShareValidationTests: XCTestCase {
    func testAcceptsHttpAndHttpsURLs() {
        XCTAssertNil(validateShareOptions(options(url: "https://example.com")))
        XCTAssertNil(validateShareOptions(options(url: "http://example.com")))
    }

    func testRejectsNonWebURLSchemes() {
        XCTAssertNotNil(validateShareOptions(options(url: "file:///tmp/secret.db")))
        XCTAssertNotNil(validateShareOptions(options(url: "content://provider/item")))
        XCTAssertNotNil(validateShareOptions(options(url: "custom://value")))
        XCTAssertNotNil(validateShareOptions(options(url: "https:///missing-host")))
        XCTAssertNotNil(validateShareOptions(options(url: " https://example.com")))
        XCTAssertNotNil(validateShareOptions(options(url: "https://example.com\nhttps://evil.example")))
    }

    func testEstimatesBase64DecodedSize() {
        XCTAssertEqual(estimateBase64DecodedSize("aGVsbG8="), 5)
        XCTAssertEqual(estimateBase64DecodedSize("aGVs\nbG8="), 5)
    }

    func testRejectsInvalidBase64Shape() {
        XCTAssertNil(estimateBase64DecodedSize("abc"))
        XCTAssertNil(estimateBase64DecodedSize("abcd==="))
    }

    func testRejectsTooManyFilesAndOversizedText() {
        let files = (0...maxFiles).map {
            SharedFile(data: "aGVsbG8=", name: "report-\($0).txt", mimeType: "text/plain")
        }

        XCTAssertNotNil(validateShareOptions(options(files: files)))
        XCTAssertNotNil(validateShareOptions(options(text: String(repeating: "a", count: maxTextBytes + 1))))
        XCTAssertNotNil(validateShareOptions(options(files: [
            SharedFile(
                data: "aGVsbG8=",
                name: String(repeating: "a", count: maxFileNameBytes + 1),
                mimeType: "text/plain"
            )
        ])))
    }

    private func options(
        text: String? = nil,
        title: String? = nil,
        url: String? = nil,
        files: [SharedFile]? = nil
    ) -> ShareOptions {
        ShareOptions(text: text, title: title, url: url, files: files)
    }
}
