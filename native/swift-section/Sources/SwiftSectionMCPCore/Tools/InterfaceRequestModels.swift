import Foundation
import MachOKit
import MachOFoundation
import Rainbow
import Semantic

package protocol MachOLoadOptionsProviding: Sendable {
    var binaryPath: String? { get }
    var dyldSharedCache: Bool? { get }
    var usesSystemDyldSharedCache: Bool? { get }
    var cacheImagePath: String? { get }
    var cacheImageName: String? { get }
    var architecture: InterfaceArchitecture? { get }
}

struct SwiftInterfaceToolArguments: Codable, MachOLoadOptionsProviding {
    var binaryPath: String?
    var dyldSharedCache: Bool?
    var usesSystemDyldSharedCache: Bool?
    var cacheImagePath: String?
    var cacheImageName: String?
    var architecture: InterfaceArchitecture?
    var outputPath: String?
    var showCImportedTypes: Bool?
    var parseOpaqueReturnType: Bool?
    var emitOffsetComments: Bool?
    var colorScheme: InterfaceColorScheme?
}

package struct MachOLoadOptions {
    let filePath: String?
    let cacheImagePath: String?
    let cacheImageName: String?
    let isDyldSharedCache: Bool
    let usesSystemDyldSharedCache: Bool
    let architecture: InterfaceArchitecture?

    init(from provider: MachOLoadOptionsProviding) {
        self.filePath = provider.binaryPath
        self.cacheImagePath = provider.cacheImagePath
        self.cacheImageName = provider.cacheImageName
        self.isDyldSharedCache = provider.dyldSharedCache ?? false
        self.usesSystemDyldSharedCache = provider.usesSystemDyldSharedCache ?? false
        self.architecture = provider.architecture
    }
}

package enum InterfaceArchitecture: String, Codable, CaseIterable {
    case x86_64
    case arm64
    case arm64e

    var cpuSubtype: CPUSubType {
        switch self {
        case .x86_64:
            return .x86(.x86_64_all)
        case .arm64:
            return .arm64(.arm64_all)
        case .arm64e:
            return .arm64(.arm64e)
        }
    }
}

enum InterfaceColorScheme: String, Codable, CaseIterable {
    case none
    case light
    case dark
}

enum SwiftInterfaceToolError: LocalizedError {
    case missingBinaryPath
    case ambiguousCacheImageSelection
    case missingCacheImageSelection
    case imageNotFound(identifier: String)
    case invalidArchitecture
    case unsupportedSystemDyldSharedCache

    var errorDescription: String? {
        switch self {
        case .missingBinaryPath:
            return "binaryPath is required unless usesSystemDyldSharedCache is true."
        case .ambiguousCacheImageSelection:
            return "Provide either cacheImageName or cacheImagePath, not both."
        case .missingCacheImageSelection:
            return "cacheImageName or cacheImagePath must be provided when targeting a dyld shared cache."
        case .imageNotFound(let identifier):
            return "Could not find image \(identifier) in the dyld shared cache."
        case .invalidArchitecture:
            return "The requested architecture could not be found in the fat binary."
        case .unsupportedSystemDyldSharedCache:
            return "The current system does not expose a host dyld shared cache."
        }
    }
}

package enum MachOLoader {
    static func load(options: MachOLoadOptions) throws -> MachOFile {
        if options.isDyldSharedCache || options.usesSystemDyldSharedCache {
            return try loadFromDyldCache(options: options)
        } else {
            return try loadMachOFile(options: options)
        }
    }

    private static func loadFromDyldCache(options: MachOLoadOptions) throws -> MachOFile {
        let dyldCache: DyldCache
        if options.usesSystemDyldSharedCache {
            guard let host = DyldCache.host else {
                throw SwiftInterfaceToolError.unsupportedSystemDyldSharedCache
            }
            dyldCache = host
        } else {
            let path = try required(options.filePath, error: SwiftInterfaceToolError.missingBinaryPath)
            let url = URL(fileURLWithPath: path)
            dyldCache = try DyldCache(url: url)
        }

        if options.cacheImageName != nil && options.cacheImagePath != nil {
            throw SwiftInterfaceToolError.ambiguousCacheImageSelection
        } else if let imageName = options.cacheImageName {
            return try required(
                dyldCache.machOFile(by: .name(imageName)),
                error: SwiftInterfaceToolError.imageNotFound(identifier: imageName)
            )
        } else if let imagePath = options.cacheImagePath {
            return try required(
                dyldCache.machOFile(by: .path(imagePath)),
                error: SwiftInterfaceToolError.imageNotFound(identifier: imagePath)
            )
        } else {
            throw SwiftInterfaceToolError.missingCacheImageSelection
        }
    }

    private static func loadMachOFile(options: MachOLoadOptions) throws -> MachOFile {
        let filePath = try required(options.filePath, error: SwiftInterfaceToolError.missingBinaryPath)
        let url = URL(fileURLWithPath: filePath)
        let file = try File.loadFromFile(url: url)
        switch file {
        case .machO(let machOFile):
            return machOFile
        case .fat(let fatFile):
            let machOFiles = try fatFile.machOFiles()
            let preferredSubtype = options.architecture?.cpuSubtype ?? CPU.current?.subtype
            let selected = preferredSubtype.flatMap { subtype in
                machOFiles.first { $0.header.cpu.subtype == subtype }
            } ?? machOFiles.first

            return try required(
                selected,
                error: SwiftInterfaceToolError.invalidArchitecture
            )
        }
    }
}

extension SemanticString {
    func rendered(using colorScheme: InterfaceColorScheme) -> String {
        switch colorScheme {
        case .none:
            return string
        case .light, .dark:
            return components.map { $0.string.withColor(for: $0.type, colorScheme: colorScheme) }.joined()
        }
    }
}

private extension String {
    func withColor(for type: SemanticType, colorScheme: InterfaceColorScheme) -> String {
        if let colorHex = colorScheme.colorHex(for: type) {
            return self.hex(colorHex, to: .bit24)
        } else if type == .error {
            return self.red
        } else {
            return self
        }
    }
}

private extension InterfaceColorScheme {
    func colorHex(for type: SemanticType) -> String? {
        switch self {
        case .none:
            return nil
        case .light:
            switch type {
            case .comment:
                return "#56606B"
            case .keyword:
                return "#C33381"
            case .type(_, .name):
                return "#2E0D6E"
            case .type(_, .declaration):
                return "#004975"
            case .function(.name),
                 .member(.name):
                return "#5C2699"
            case .function(.declaration),
                 .member(.declaration),
                 .variable:
                return "#0F68A0"
            case .numeric:
                return "#000BFF"
            default:
                return nil
            }
        case .dark:
            switch type {
            case .comment:
                return "#6C7987"
            case .keyword:
                return "#F2248C"
            case .type(_, .name):
                return "#D0A8FF"
            case .type(_, .declaration):
                return "#5DD8FF"
            case .function(.name),
                 .member(.name):
                return "#A167E6"
            case .function(.declaration),
                 .member(.declaration),
                 .variable:
                return "#41A1C0"
            case .numeric:
                return "#D0BF69"
            default:
                return nil
            }
        }
    }
}
