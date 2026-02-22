import Foundation

enum MachOToolSchema {
    static func baseProperties() -> [String: JSONValue] {
        [
            "binaryPath": .object([
                "type": .string("string"),
                "description": .string("Absolute path to the Mach-O binary when not using the system dyld shared cache.")
            ]),
            "dyldSharedCache": .object([
                "type": .string("boolean"),
                "description": .string("Set to true if binaryPath points to a dyld shared cache.")
            ]),
            "usesSystemDyldSharedCache": .object([
                "type": .string("boolean"),
                "description": .string("Load the Mach-O from the current system dyld shared cache.")
            ]),
            "cacheImageName": .object([
                "type": .string("string"),
                "description": .string("Name of the image inside the selected dyld shared cache.")
            ]),
            "cacheImagePath": .object([
                "type": .string("string"),
                "description": .string("Path of the image inside the selected dyld shared cache.")
            ]),
            "architecture": .object([
                "type": .string("string"),
                "description": .string("Architecture to select when binaryPath points to a fat binary."),
                "enum": .array(InterfaceArchitecture.allCases.map { .string($0.rawValue) })
            ])
        ]
    }

    static func inputSchema(additionalProperties: Bool = false, extraProperties: [String: JSONValue]) -> JSONValue {
        var properties = baseProperties()
        extraProperties.forEach { properties[$0.key] = $0.value }

        return .object([
            "type": .string("object"),
            "additionalProperties": .bool(additionalProperties),
            "properties": .object(properties)
        ])
    }
}

