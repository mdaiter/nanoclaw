import Foundation
import MachOKit
import MachOFoundation
import MachOSymbols
import Demangling

struct SearchSymbolsArguments: Codable, MachOLoadOptionsProviding {
    var binaryPath: String?
    var dyldSharedCache: Bool?
    var usesSystemDyldSharedCache: Bool?
    var cacheImagePath: String?
    var cacheImageName: String?
    var architecture: InterfaceArchitecture?

    var query: String?
    var caseSensitive: Bool?
    var limit: Int?
    var kindFilter: [SymbolSearchKind]?
    var includeMangledName: Bool?
}

enum SymbolSearchKind: String, Codable, CaseIterable {
    case function
    case method
    case accessor
    case variable
    case typeMetadata
    case type
    case other
}

struct SymbolSearchRecord: Codable {
    let name: String
    let mangledName: String?
    let kind: SymbolSearchKind
    let offset: Int
}

struct SearchSymbolsTool: MCPTool {
    let name = "searchSymbols"
    let description = "Search Swift symbols within the target binary or dyld shared cache image."

    var inputSchema: JSONValue {
        MachOToolSchema.inputSchema(
            additionalProperties: false,
            extraProperties: [
                "query": .object([
                    "type": .string("string"),
                    "description": .string("Substring to search for within demangled or mangled symbol names.")
                ]),
                "caseSensitive": .object([
                    "type": .string("boolean"),
                    "description": .string("Treat query as case-sensitive (defaults to false).")
                ]),
                "limit": .object([
                    "type": .string("integer"),
                    "description": .string("Maximum number of symbols to return. Defaults to 50.")
                ]),
                "kindFilter": .object([
                    "type": .string("array"),
                    "description": .string("Return only the specified symbol categories."),
                    "items": .object([
                        "type": .string("string"),
                        "enum": .array(SymbolSearchKind.allCases.map { .string($0.rawValue) })
                    ])
                ]),
                "includeMangledName": .object([
                    "type": .string("boolean"),
                    "description": .string("Set to false to omit mangled symbol names from the response.")
                ])
            ]
        )
    }

    func call(arguments: JSONValue?, context: ToolContext) async throws -> ToolResponse {
        let request = try context.decodeArguments(SearchSymbolsArguments.self, from: arguments)
        let machO = try MachOLoader.load(options: MachOLoadOptions(from: request))
        let symbols = machO.swiftSymbols

        let filterKinds = request.kindFilter.map { Set($0) }
        let caseSensitive = request.caseSensitive ?? false
        let normalizedQuery = normalize(query: request.query, caseSensitive: caseSensitive)
        let includeMangled = request.includeMangledName ?? true
        let limit = request.limit ?? 50

        var results: [SymbolSearchRecord] = []
        results.reserveCapacity(limit)

        for symbol in symbols {
            let demangledNode = try? symbol.demangledNode
            let printedName = demangledNode?.print(using: .interfaceTypeBuilderOnly) ?? symbol.name

            if let normalizedQuery,
               !matches(text: printedName, normalizedQuery: normalizedQuery, originalQuery: request.query, caseSensitive: caseSensitive) &&
               !matches(text: symbol.name, normalizedQuery: normalizedQuery, originalQuery: request.query, caseSensitive: caseSensitive) {
                continue
            }

            let kind = SymbolClassifier.classify(node: demangledNode)
            if let filterKinds, !filterKinds.contains(kind) {
                continue
            }

            let record = SymbolSearchRecord(
                name: printedName,
                mangledName: includeMangled ? symbol.name : nil,
                kind: kind,
                offset: symbol.offset
            )
            results.append(record)

            if results.count >= limit {
                break
            }
        }

        let payload = try context.encoder.encode(results)
        guard let json = String(data: payload, encoding: .utf8) else {
            throw MCPToolError.encodingFailed
        }

        return ToolResponse(content: [
            .text("Found \(results.count) Swift symbols."),
            .text(json)
        ])
    }

    private func normalize(query: String?, caseSensitive: Bool) -> String? {
        guard let query, !query.isEmpty else {
            return nil
        }
        return caseSensitive ? query : query.lowercased()
    }

    private func matches(text: String, normalizedQuery: String, originalQuery: String?, caseSensitive: Bool) -> Bool {
        if caseSensitive {
            guard let originalQuery else { return true }
            return text.contains(originalQuery)
        } else {
            return text.lowercased().contains(normalizedQuery)
        }
    }
}

private enum SymbolClassifier {
    static func classify(node: Node?) -> SymbolSearchKind {
        guard let node else { return .other }

        if node.contains(.methodDescriptor) || node.contains(.methodLookupFunction) {
            return .method
        } else if node.contains(.getter) || node.contains(.setter) || node.contains(.willSet) || node.contains(.didSet) {
            return .accessor
        } else if node.contains(.function) {
            return .function
        } else if node.contains(.variable) {
            return .variable
        } else if node.contains(.metatype) || node.contains(.typeMetadata) {
            return .typeMetadata
        } else if node.representsTypeForMCP {
            return .type
        } else {
            return .other
        }
    }
}

private extension Node {
    func contains(_ kind: Kind) -> Bool {
        first(of: kind) != nil
    }

    var representsTypeForMCP: Bool {
        if first(of: .type) != nil {
            return true
        }
        return contains(.enum) ||
            contains(.boundGenericEnum) ||
            contains(.structure) ||
            contains(.boundGenericStructure) ||
            contains(.class) ||
            contains(.boundGenericClass)
    }
}
