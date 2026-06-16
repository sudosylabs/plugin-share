// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "Tauri",
    products: [
        .library(name: "Tauri", targets: ["Tauri"])
    ],
    targets: [
        .target(name: "Tauri", path: "Sources/Tauri")
    ]
)
