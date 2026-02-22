import Foundation
import MachOKit
import MachOFoundation
import MachOSwiftSection
import SwiftInterface

struct ListTypesToolArguments: Codable, MachOLoadOptionsProviding {
    var binaryPath: String?
    var dyldSharedCache: Bool?
    var usesSystemDyldSharedCache: Bool?
    var cacheImagePath: String?
    var cacheImageName: String?
    var architecture: InterfaceArchitecture?

    var query: String?
    var caseSensitive: Bool?
    var limit: Int?
    var kindFilter: [ListTypesKind]?
    var includeMangledName: Bool?
}

enum ListTypesKind: String, Codable, CaseIterable {
    case `enum`
    case `struct`
    case `class`

    var typeKind: TypeKind {
        switch self {
        case .enum:
            return .enum
        case .struct:
            return .struct
        case .class:
            return .class
        }
    }
}

struct ListTypesRecord: Codable {
    let name: String
    let shortName: String
    let module: String?
    let kind: String
    let mangledName: String?
}

struct ListTypesTool: MCPTool {
    let name = "listTypes"
    let description = "List Swift nominal types (structs, enums, and classes) discovered in the target binary."

    var inputSchema: JSONValue {
        MachOToolSchema.inputSchema(
            additionalProperties: false,
            extraProperties: [
                "query": .object([
                    "type": .string("string"),
                    "description": .string("Optional substring filter that matches against the fully qualified type name.")
                ]),
                "caseSensitive": .object([
                    "type": .string("boolean"),
                    "description": .string("Set to true to make the query filter case-sensitive.")
                ]),
                "limit": .object([
                    "type": .string("integer"),
                    "description": .string("Maximum number of types to return. Defaults to 100.")
                ]),
                "kindFilter": .object([
                    "type": .string("array"),
                    "description": .string("Optional list of type kinds to include (enum, struct, class)."),
                    "items": .object([
                        "type": .string("string"),
                        "enum": .array(ListTypesKind.allCases.map { .string($0.rawValue) })
                    ])
                ]),
                "includeMangledName": .object([
                    "type": .string("boolean"),
                    "description": .string("Set to false to omit mangled names from the response.")
                ])
            ]
        )
    }

    func call(arguments: JSONValue?, context: ToolContext) async throws -> ToolResponse {
        let request = try context.decodeArguments(ListTypesToolArguments.self, from: arguments)
        let machO = try MachOLoader.load(options: MachOLoadOptions(from: request))
        let descriptors = try machO.swift.typeContextDescriptors

        let rawQuery = request.query?.trimmingCharacters(in: .whitespacesAndNewlines)
        let caseSensitive = request.caseSensitive ?? false
        let normalizedQuery = caseSensitive ? rawQuery : rawQuery?.lowercased()
        let filterKinds = request.kindFilter.map { Set($0.map(\.typeKind)) }
        let limit = request.limit ?? 100
        var records: [ListTypesRecord] = []
        records.reserveCapacity(min(limit, descriptors.count))

        for descriptor in descriptors {
            if let filterKinds, !filterKinds.contains(descriptor.mcpTypeKind) {
                continue
            }

            let typeName = try descriptor.typeName(in: machO)
            let fullName = typeName.name

            if let normalizedQuery,
               !fullName.matches(normalizedQuery: normalizedQuery, originalQuery: rawQuery, caseSensitive: caseSensitive) {
                continue
            }

            let mangledName: String?
            if request.includeMangledName ?? true {
                mangledName = try? descriptor.namedContextDescriptor.mangledName(in: machO).symbolString
            } else {
                mangledName = nil
            }

            let module = fullName.split(separator: ".").first.map(String.init)

            let record = ListTypesRecord(
                name: fullName,
                shortName: typeName.currentName,
                module: module,
                kind: descriptor.mcpTypeKind.responseValue,
                mangledName: mangledName
            )
            records.append(record)

            if records.count >= limit {
                break
            }
        }

        let payload = try context.encoder.encode(records)
        guard let json = String(data: payload, encoding: .utf8) else {
            throw MCPToolError.encodingFailed
        }

        return ToolResponse(content: [
            .text("Found \(records.count) Swift types."),
            .text(json)
        ])
    }
}

private extension String {
    func matches(normalizedQuery: String, originalQuery: String?, caseSensitive: Bool) -> Bool {
        if caseSensitive {
            guard let originalQuery else { return true }
            return contains(originalQuery)
        } else {
            return lowercased().contains(normalizedQuery)
        }
    }
}

private extension TypeContextDescriptorWrapper {
    var mcpTypeKind: TypeKind {
        switch self {
        case .enum:
            return .enum
        case .struct:
            return .struct
        case .class:
            return .class
        }
    }
}

private extension TypeKind {
    var responseValue: String {
        switch self {
        case .enum:
            return "enum"
        case .struct:
            return "struct"
        case .class:
            return "class"
        }
    }
}
