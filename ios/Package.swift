// swift-tools-version:5.3
import Foundation
import PackageDescription

let useTauriStub = ProcessInfo.processInfo.environment["VNIDROP_SHARE_USE_TAURI_STUB"] == "1"
let tauriApiPath = !useTauriStub && FileManager.default.fileExists(atPath: "../.tauri/tauri-api/Package.swift")
    ? "../.tauri/tauri-api"
    : "test-support/tauri-api"

let package = Package(
    name: "tauri-plugin-vnidrop-share",
    platforms: [
        .macOS(.v10_13),
        .iOS(.v13),
    ],
    products: [
        // Products define the executables and libraries a package produces, and make them visible to other packages.
        .library(
            name: "tauri-plugin-vnidrop-share",
            type: .static,
            targets: ["tauri-plugin-vnidrop-share"]),
    ],
    dependencies: [
        .package(name: "Tauri", path: tauriApiPath)
    ],
    targets: [
        .target(
            name: "tauri-plugin-vnidrop-share",
            dependencies: [
                .byName(name: "Tauri"),
                .byName(name: "ShareCore")
            ],
            path: "Sources",
            exclude: ["ShareCore"]),
        .target(
            name: "ShareCore",
            path: "Sources/ShareCore"),
        .testTarget(
            name: "SharePluginTests",
            dependencies: [
                .byName(name: "ShareCore")
            ],
            path: "Tests/SharePluginTests")
    ]
)
