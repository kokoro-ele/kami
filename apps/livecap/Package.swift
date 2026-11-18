// swift-tools-version: 5.10
import PackageDescription

let package = Package(
    name: "livecap",
    platforms: [.macOS("26.0")],
    targets: [
        .executableTarget(
            name: "livecap",
            path: "Sources/livecap"
        )
    ]
)
