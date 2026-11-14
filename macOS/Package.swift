// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Snapline",
    platforms: [.macOS(.v15)],
    targets: [
        // Pure logic with no AppKit: signing, OAuth helpers, matchers, geometry. Unit tested.
        .target(name: "SnaplineCore"),
        // The menu bar app itself (AppKit, ScreenCaptureKit, Vision).
        .executableTarget(name: "Snapline", dependencies: ["SnaplineCore"]),
        .testTarget(name: "SnaplineCoreTests", dependencies: ["SnaplineCore"]),
    ]
)
