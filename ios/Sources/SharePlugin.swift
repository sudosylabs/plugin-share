#if canImport(UIKit)
import Tauri
import ShareCore
import UIKit
import WebKit

@_cdecl("init_plugin_share") 
func initPlugin() -> Plugin {
    return SharePlugin()
}

public class SharePlugin: Plugin {

    private var webview: WKWebView?
    private var temporaryFileURLs: [URL] = []
    private var shareInProgress = false

    open override func load(webview: WKWebView) {
        self.webview = webview
    }

    @objc func canShare(_ invoke: Invoke) throws {
        // The native share sheet is always available on iOS.
        invoke.resolve(["value": true])
    }

    /**
     * Deletes all temporary files created by this plugin.
     * This is a manual cleanup utility for the developer.
     */
    @objc func cleanup(_ invoke: Invoke) {
        if shareInProgress {
            invoke.reject("Cannot cleanup while sharing is in progress.")
            return
        }

        do {
            let shareDir = try getSafeShareDir()
            if FileManager.default.fileExists(atPath: shareDir.path) {
                try FileManager.default.removeItem(at: shareDir)
            }
            temporaryFileURLs.removeAll()
            invoke.resolve()
        } catch {
            invoke.reject("Error during cleanup: \(error.localizedDescription)")
        }
    }

    @objc func share(_ invoke: Invoke) throws {
        if shareInProgress {
            invoke.reject("Share already in progress.")
            return
        }

        let args = try invoke.parseArgs(ShareOptions.self)
        if let validationError = validateShareOptions(args) {
            invoke.reject(validationError)
            return
        }

        var activityItems: [Any] = []
        var createdFileURLs: [URL] = []
        shareInProgress = true

        if let urlString = args.url, !urlString.isEmpty, let url = URL(string: urlString) {
            activityItems.append(url)
        }
        if let text = args.text {
            activityItems.append(text)
        }

        if let files = args.files {
            for file in files {
                guard let decodedData = Data(base64Encoded: file.data) else {
                    cleanupTemporaryFiles(createdFileURLs)
                    _ = resetShareState()
                    invoke.reject("Invalid Base64 data for file: \(file.name)")
                    return
                }
                if decodedData.count > maxFileBytes {
                    cleanupTemporaryFiles(createdFileURLs)
                    _ = resetShareState()
                    invoke.reject("File '\(file.name)' exceeds the maximum size of \(maxFileBytes) bytes.")
                    return
                }

                do {
                    let tempFileURL = try createSafeTempFile(for: file.name)
                    try decodedData.write(to: tempFileURL, options:.atomic)
                    activityItems.append(tempFileURL)
                    createdFileURLs.append(tempFileURL)
                } catch {
                    cleanupTemporaryFiles(createdFileURLs)
                    _ = resetShareState()
                    invoke.reject("Failed to create temporary file: \(error.localizedDescription)")
                    return
                }
            }
        }

        // Share local files directly from their original paths. This preserves the
        // original filename and avoids creating temporary copies.
        if let filePaths = args.filePaths {
            for path in filePaths {
                let fileURL = URL(fileURLWithPath: path)
                activityItems.append(fileURL)
            }
        }

        if activityItems.isEmpty {
            _ = resetShareState()
            invoke.reject("No content provided to share.")
            return
        }

        temporaryFileURLs.append(contentsOf: createdFileURLs)
        presentShareSheet(invoke: invoke, activityItems: activityItems, anchor: args.anchor)
    }
    
    /**
     * Returns the URL for a dedicated, secure directory for storing temporary share files.
     * Creates it if it doesn't exist.
     */
    private func getSafeShareDir() throws -> URL {
        let tempDir = FileManager.default.temporaryDirectory
        let shareDir = tempDir.appendingPathComponent("share")
        
        if !FileManager.default.fileExists(atPath: shareDir.path) {
            try FileManager.default.createDirectory(
                at: shareDir,
                withIntermediateDirectories: true
            )
        }
        
        return shareDir
    }

    /**
     * Creates a safe temporary file URL within the dedicated share directory.
     * It sanitizes the filename and prepends a UUID to guarantee uniqueness.
     */
    private func createSafeTempFile(for untrustedFileName: String) throws -> URL {
        let safeDir = try getSafeShareDir()
        
        let sanitizedBaseName = URL(fileURLWithPath: untrustedFileName).lastPathComponent
        if sanitizedBaseName.isEmpty {
            throw NSError(domain: "SharePlugin", code: 1, userInfo: [NSLocalizedDescriptionKey: "Invalid filename: sanitized name is empty."])
        }
        
        let finalFileName = "\(UUID().uuidString)-\(sanitizedBaseName)"
        
        return safeDir.appendingPathComponent(finalFileName)
    }

    private func presentShareSheet(invoke: Invoke, activityItems: [Any], anchor: ShareAnchor? = nil) {
        DispatchQueue.main.async {
            guard let viewController = self.manager.viewController else {
                _ = self.resetShareState()
                invoke.reject("Could not find root view controller.")
                return
            }

            guard viewController.presentedViewController == nil else {
                let filesToClean = self.resetShareState()
                self.cleanupTemporaryFiles(filesToClean)
                invoke.reject("Another view controller is already being presented.")
                return
            }

            let activityViewController = UIActivityViewController(activityItems: activityItems, applicationActivities: nil)
            if UIDevice.current.userInterfaceIdiom == .pad {
                activityViewController.modalPresentationStyle = .popover
            }
            
            activityViewController.completionWithItemsHandler = { _, _, _, error in
                DispatchQueue.main.async {
                    let filesToClean = self.resetShareState()
                    self.cleanupTemporaryFiles(filesToClean)

                    if let anError = error {
                        invoke.reject("Sharing failed: \(anError.localizedDescription)")
                    } else {
                        invoke.resolve()
                    }
                }
            }

            // iPad presentation logic
            if let popoverController = activityViewController.popoverPresentationController {
                guard let sourceView = self.webview ?? viewController.view else {
                    let filesToClean = self.resetShareState()
                    self.cleanupTemporaryFiles(filesToClean)
                    invoke.reject("Could not find a source view for the share popover.")
                    return
                }
                popoverController.sourceView = sourceView

                if let anchor = anchor {
                    popoverController.sourceRect = CGRect(
                        x: anchor.x,
                        y: anchor.y,
                        width: anchor.width,
                        height: anchor.height
                    )
                    popoverController.permittedArrowDirections = [.up, .down]
                } else {
                    popoverController.sourceRect = CGRect(x: sourceView.bounds.midX, y: sourceView.bounds.midY, width: 0, height: 0)
                    popoverController.permittedArrowDirections = []
                }
            }

            viewController.present(activityViewController, animated: true, completion: nil)
        }
    }

    private func resetShareState() -> [URL] {
        let filesToClean = temporaryFileURLs
        temporaryFileURLs.removeAll()
        shareInProgress = false
        return filesToClean
    }

    private func cleanupTemporaryFiles(_ urls: [URL]) {
        DispatchQueue.global(qos:.utility).async {
            for url in urls {
                try? FileManager.default.removeItem(at: url)
            }
        }
    }
}
#endif
