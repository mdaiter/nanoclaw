// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "AppAutomation",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "app-automation", targets: ["AppAutomationCLI"]),
        .executable(name: "app-agent", targets: ["AppAutomationAgent"]),
    ],
    targets: [
        .target(name: "AppAutomationCore"),
        .target(name: "AppAutomationAX", dependencies: ["AppAutomationCore"]),
        .executableTarget(name: "AppAutomationCLI", dependencies: ["AppAutomationCore", "AppAutomationAX"]),
        .executableTarget(
            name: "AppAutomationAgent",
            dependencies: ["AppAutomationCore", "AppAutomationAX"],
            swiftSettings: [.unsafeFlags(["-parse-as-library"])]
        ),
    ]
)
