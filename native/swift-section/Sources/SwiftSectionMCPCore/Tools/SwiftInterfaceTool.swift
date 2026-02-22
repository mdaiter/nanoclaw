import Foundation
import MachOKit
import MachOFoundation
import Semantic
import SwiftInterface

struct SwiftInterfaceTool: MCPTool {
    let name = "swiftInterface"
    let description = "Generate a Swift interface for the provided Mach-O binary or dyld shared cache image."

    var inputSchema: JSONValue {
        MachOToolSchema.inputSchema(
            additionalProperties: false,
            extraProperties: [
                "outputPath": .object([
                    "type": .string("string"),
                    "description": .string("Optional file path for saving the generated Swift interface.")
                ]),
                "showCImportedTypes": .object([
                    "type": .string("boolean"),
                    "description": .string("Include imported C types in the generated interface.")
                ]),
                "parseOpaqueReturnType": .object([
                    "type": .string("boolean"),
                    "description": .string("Attempt to resolve opaque return types (experimental).")
                ]),
                "emitOffsetComments": .object([
                    "type": .string("boolean"),
                    "description": .string("Emit field offset and PWT offset comments when possible.")
                ]),
                "colorScheme": .object([
                    "type": .string("string"),
                    "description": .string("Color scheme for syntax-highlighted output."),
                    "enum": .array(InterfaceColorScheme.allCases.map { .string($0.rawValue) })
                ])
            ]
        )
    }

    func call(arguments: JSONValue?, context: ToolContext) async throws -> ToolResponse {
        let request = try context.decodeArguments(SwiftInterfaceToolArguments.self, from: arguments)
        let loadOptions = MachOLoadOptions(from: request)
        let machOFile = try MachOLoader.load(options: loadOptions)

        context.logger.info("Loaded Mach-O image for \(request.binaryPath ?? request.cacheImageName ?? "unknown binary"). Preparing Swift interface.")

        let configuration = SwiftInterfaceBuilderConfiguration(
            indexConfiguration: .init(
                showCImportedTypes: request.showCImportedTypes ?? false
            ),
            printConfiguration: .init(
                printStrippedSymbolicItem: true,
                emitOffsetComments: request.emitOffsetComments ?? false
            )
        )

        let builder = try SwiftInterfaceBuilder(configuration: configuration, eventHandlers: [], in: machOFile)

        if request.parseOpaqueReturnType ?? false {
            builder.addExtraDataProvider(SwiftInterfaceBuilderOpaqueTypeProvider(machO: machOFile))
        }

        try await builder.prepare()
        let semanticOutput = try await builder.printRoot()
        let renderedOutput = semanticOutput.rendered(using: request.colorScheme ?? .none)

        if let outputPath = request.outputPath {
            let outputURL = URL(fileURLWithPath: outputPath)
            try renderedOutput.write(to: outputURL, atomically: true, encoding: .utf8)
            context.logger.info("Swift interface written to \(outputPath).")
            return ToolResponse(content: [
                .text("Swift interface saved to \(outputPath)."),
                .text(renderedOutput)
            ])
        } else {
            return .text(renderedOutput)
        }
    }
}
