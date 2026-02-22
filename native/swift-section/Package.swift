// swift-tools-version: 6.2
@preconcurrency import PackageDescription
import CompilerPluginSupport

let package = Package(
    name: "SwiftSection",
    platforms: [.macOS(.v13)],
    products: [
        .executable(name: "swift-section", targets: ["swift-section"]),
        .executable(name: "swift-section-mcp", targets: ["swift-section-mcp"]),
        .library(name: "MachOSwiftSection", targets: ["MachOSwiftSection"]),
    ],
    dependencies: [
        .package(url: "https://github.com/p-x9/MachOKit.git", exact: "0.42.0"),
        .package(url: "https://github.com/swiftlang/swift-syntax.git", "509.1.0" ..< "602.0.0"),
        .package(url: "https://github.com/p-x9/AssociatedObject", from: "0.13.0"),
        .package(url: "https://github.com/p-x9/swift-fileio.git", from: "0.9.0"),
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.5.1"),
        .package(url: "https://github.com/onevcat/Rainbow", from: "4.0.0"),
        .package(url: "https://github.com/Mx-Iris/FrameworkToolbox", branch: "main"),
        .package(url: "https://github.com/apple/swift-collections", from: "1.2.0"),
        .package(url: "https://github.com/MxIris-Library-Forks/swift-memberwise-init-macro", from: "0.5.3-fork"),
        .package(url: "https://github.com/p-x9/MachOObjCSection", from: "0.5.0"),
        .package(url: "https://github.com/pointfreeco/swift-dependencies", from: "1.9.4"),
        .package(url: "https://github.com/apple/swift-async-algorithms", from: "1.0.4"),
        .package(url: "https://github.com/MxIris-Reverse-Engineering/DyldPrivate", branch: "main"),
    ],
    targets: [
        // MARK: - Core
        .target(name: "Semantic"),
        .target(name: "UtilitiesC"),
        .macro(
            name: "MachOMacros",
            dependencies: [
                .product(name: "SwiftSyntax", package: "swift-syntax"),
                .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
                .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
                .product(name: "SwiftSyntaxBuilder", package: "swift-syntax"),
            ]
        ),
        .target(
            name: "Utilities",
            dependencies: [
                "MachOMacros", "UtilitiesC",
                .product(name: "FoundationToolbox", package: "FrameworkToolbox"),
                .product(name: "AssociatedObject", package: "AssociatedObject"),
                .product(name: "MemberwiseInit", package: "swift-memberwise-init-macro"),
                .product(name: "OrderedCollections", package: "swift-collections"),
                .product(name: "Dependencies", package: "swift-dependencies"),
                .product(name: "AsyncAlgorithms", package: "swift-async-algorithms"),
            ]
        ),
        .target(name: "Demangling", dependencies: ["Utilities"]),
        .target(
            name: "MachOExtensions",
            dependencies: ["Utilities", .product(name: "MachOKit", package: "MachOKit")]
        ),
        .target(
            name: "MachOCaches",
            dependencies: ["MachOExtensions", "Utilities", .product(name: "MachOKit", package: "MachOKit")]
        ),
        .target(
            name: "MachOReading",
            dependencies: [
                "MachOExtensions", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
                .product(name: "FileIO", package: "swift-fileio"),
            ]
        ),
        .target(
            name: "MachOResolving",
            dependencies: ["MachOExtensions", "MachOReading", .product(name: "MachOKit", package: "MachOKit")]
        ),
        .target(
            name: "MachOSymbols",
            dependencies: [
                "MachOReading", "MachOResolving", "Utilities", "Demangling", "MachOCaches",
                .product(name: "MachOKit", package: "MachOKit"),
            ],
            swiftSettings: [.unsafeFlags(["-Xfrontend", "-enable-private-imports"])]
        ),
        .target(
            name: "MachOPointers",
            dependencies: [
                "MachOReading", "MachOResolving", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
            ]
        ),
        .target(
            name: "MachOSymbolPointers",
            dependencies: [
                "MachOReading", "MachOResolving", "MachOPointers", "MachOSymbols", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
            ]
        ),
        .target(
            name: "MachOFoundation",
            dependencies: [
                "MachOReading", "MachOExtensions", "MachOPointers", "MachOSymbols",
                "MachOResolving", "MachOSymbolPointers", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
            ]
        ),
        .target(
            name: "MachOSwiftSection",
            dependencies: [
                "MachOFoundation", "Demangling", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
                .product(name: "DyldPrivate", package: "DyldPrivate"),
            ]
        ),
        // MARK: - High-level
        .target(
            name: "SwiftDump",
            dependencies: [
                "MachOSwiftSection", "Semantic", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
                .product(name: "MachOObjCSection", package: "MachOObjCSection"),
            ]
        ),
        .target(
            name: "SwiftInterface",
            dependencies: [
                "MachOSwiftSection", "SwiftDump", "Semantic", "Utilities",
                .product(name: "MachOKit", package: "MachOKit"),
            ]
        ),
        // MARK: - MCP
        .target(
            name: "SwiftSectionMCPCore",
            dependencies: [
                "MachOFoundation", "MachOSwiftSection", "SwiftInterface",
                "Semantic", "MachOSymbols",
                .product(name: "MachOKit", package: "MachOKit"),
                .product(name: "Rainbow", package: "Rainbow"),
            ]
        ),
        .executableTarget(name: "swift-section-mcp", dependencies: ["SwiftSectionMCPCore"]),
        // MARK: - CLI
        .executableTarget(
            name: "swift-section",
            dependencies: [
                "SwiftDump", "SwiftInterface",
                .product(name: "Rainbow", package: "Rainbow"),
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
            ]
        ),
    ]
)
